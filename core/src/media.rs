use crate::{
    model::{MediaAsset, MediaKind},
    server::{bad, sniff, ApiError, Shared},
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use jni::{
    objects::{GlobalRef, JClass, JString, JValue},
    JNIEnv, JavaVM,
};
use serde_json::Value;
use std::{
    path::{Path as FsPath, PathBuf},
    sync::OnceLock,
    time::Duration,
};
use tokio::{io::AsyncWriteExt, process::Command};
use uuid::Uuid;
const MAX_UPLOAD: u64 = 512 * 1024 * 1024;
static JAVA: OnceLock<(JavaVM, GlobalRef)> = OnceLock::new();
pub fn init_java(env: &mut JNIEnv) -> Result<(), String> {
    if JAVA.get().is_none() {
        let class = env
            .find_class("app/polotno/NativeMedia")
            .map_err(|e| e.to_string())?;
        let global = env.new_global_ref(class).map_err(|e| e.to_string())?;
        let vm = env.get_java_vm().map_err(|e| e.to_string())?;
        let _ = JAVA.set((vm, global));
    }
    Ok(())
}
fn android_photo(source: &FsPath, target: &FsPath) -> Option<Value> {
    let (vm, class) = JAVA.get()?;
    let mut env = vm.attach_current_thread().ok()?;
    let a = env.new_string(source.to_str()?).ok()?;
    let b = env.new_string(target.to_str()?).ok()?;
    let class: &JClass = class.as_obj().into();
    let result = env
        .call_static_method(
            class,
            "decodeImage",
            "(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;",
            &[JValue::Object(&a), JValue::Object(&b)],
        )
        .ok()?
        .l()
        .ok()?;
    let text: String = env.get_string(&JString::from(result)).ok()?.into();
    serde_json::from_str(&text).ok()
}
fn binary(directory: &FsPath, name: &str) -> PathBuf {
    #[cfg(target_os = "android")]
    {
        directory.join(format!("libpolotno_{name}.so"))
    }
    #[cfg(not(target_os = "android"))]
    {
        directory.join(name)
    }
}
async fn probe(directory: &FsPath, path: &FsPath) -> Result<Value, String> {
    let output = tokio::time::timeout(
        Duration::from_secs(30),
        Command::new(binary(directory, "ffprobe"))
            .args([
                "-v",
                "error",
                "-show_streams",
                "-show_format",
                "-of",
                "json",
            ])
            .arg(path)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| "Проверка файла заняла слишком много времени".to_string())?
    .map_err(|_| "Конвертер недоступен".to_string())?;
    if !output.status.success() {
        return Err(
            "Не удалось прочитать файл. Проверьте, что он не повреждён и не защищён".into(),
        );
    }
    serde_json::from_slice(&output.stdout).map_err(|_| "Не удалось прочитать формат файла".into())
}
async fn prepare(
    directory: &FsPath,
    source: &FsPath,
    folder: &FsPath,
) -> Result<(PathBuf, MediaKind, u32, u32), String> {
    let png = folder.join("ready.png");
    let photo_source = source.to_path_buf();
    let photo_target = png.clone();
    if let Some(size) =
        tokio::task::spawn_blocking(move || android_photo(&photo_source, &photo_target))
            .await
            .ok()
            .flatten()
    {
        return Ok((
            png,
            MediaKind::Image,
            size["width"].as_u64().unwrap_or(0) as u32,
            size["height"].as_u64().unwrap_or(0) as u32,
        ));
    }
    let data = probe(directory, source).await?;
    let stream = data["streams"]
        .as_array()
        .and_then(|s| s.iter().find(|s| s["codec_type"] == "video"))
        .ok_or("В файле нет изображения или видео")?;
    let codec = stream["codec_name"].as_str().unwrap_or("");
    let format = data["format"]["format_name"].as_str().unwrap_or("");
    let still = format.contains("image")
        || [
            "png", "mjpeg", "tiff", "bmp", "webp", "qoi", "exr", "targa", "ppm", "pbm", "pgm",
            "jpeg2000",
        ]
        .contains(&codec);
    let ready = folder.join(if still { "ready.png" } else { "ready.mp4" });
    let mut command = Command::new(binary(directory, "ffmpeg"));
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-y",
            "-threads",
            "2",
            "-i",
        ])
        .arg(source)
        .args(["-map", "0:v:0"]);
    if still {
        command.args([
            "-frames:v",
            "1",
            "-vf",
            "scale='min(iw,2048)':'min(ih,2048)':force_original_aspect_ratio=decrease",
            "-c:v",
            "png",
        ]);
    } else if compatible_video(&data) {
        // Keep already compatible H.264 clips, including their lower frame rate.
        // Remux to MP4 so browser/projector playback does not depend on the input container.
        command.args(["-map", "0:a:0?", "-c", "copy", "-movflags", "+faststart"]);
    } else {
        command.args([
            "-vf",
            "scale='min(iw,1920)':'min(ih,1080)':force_original_aspect_ratio=decrease:force_divisible_by=2,fps=30",
            "-c:v",
            "libopenh264",
            "-b:v",
            "4M",
            "-pix_fmt",
            "yuv420p",
            "-map",
            "0:a?",
            "-c:a",
            "aac",
            "-b:a",
            "128k",
            "-movflags",
            "+faststart",
        ]);
    }
    let output = tokio::time::timeout(
        Duration::from_secs(1800),
        command.arg(&ready).kill_on_drop(true).output(),
    )
    .await
    .map_err(|_| "Обработка заняла слишком много времени".to_string())?
    .map_err(|_| "Конвертер недоступен".to_string())?;
    if !output.status.success() {
        return Err(
            "Не удалось обработать кодек этого файла. Оригинал сохранён; галерея не изменена"
                .into(),
        );
    }
    let processed = probe(directory, &ready).await?;
    let streams = processed["streams"]
        .as_array()
        .ok_or("Обработка не создала готовое медиа")?;
    let output = streams
        .iter()
        .find(|s| s["codec_type"] == "video")
        .ok_or("Конвертер не создал изображение или видео")?;
    Ok((
        ready,
        if still {
            MediaKind::Image
        } else {
            MediaKind::Video
        },
        output["width"].as_u64().unwrap_or(0) as u32,
        output["height"].as_u64().unwrap_or(0) as u32,
    ))
}

fn compatible_video(data: &serde_json::Value) -> bool {
    let Some(streams) = data["streams"].as_array() else {
        return false;
    };
    let Some(video) = streams.iter().find(|s| s["codec_type"] == "video") else {
        return false;
    };
    let width = video["width"].as_u64().unwrap_or(0);
    let height = video["height"].as_u64().unwrap_or(0);
    let rate = video["avg_frame_rate"].as_str().unwrap_or("");
    let fps = rate
        .split_once('/')
        .and_then(|(n, d)| Some(n.parse::<f64>().ok()? / d.parse::<f64>().ok()?))
        .unwrap_or(f64::NAN);
    let rotated = video["tags"]["rotate"].as_str().is_some_and(|r| r != "0")
        || video["side_data_list"].as_array().is_some_and(|list| {
            list.iter()
                .any(|s| s["rotation"].as_f64().is_some_and(|r| r != 0.0))
        });
    video["codec_name"] == "h264"
        && video["pix_fmt"] == "yuv420p"
        && width > 0
        && width <= 1920
        && height > 0
        && height <= 1080
        && fps.is_finite()
        && fps > 0.0
        && fps <= 30.0
        && !rotated
        && !matches!(
            video["color_transfer"].as_str(),
            Some("smpte2084" | "arib-std-b67")
        )
        && streams
            .iter()
            .filter(|s| s["codec_type"] == "audio")
            .all(|s| s["codec_name"] == "aac")
}

pub async fn upload(
    state: Shared,
    request: axum::extract::Request,
) -> Result<Json<MediaAsset>, ApiError> {
    let headers = request.headers();
    let name = headers
        .get("x-file-name")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| serde_json::from_str::<String>(v).ok())
        .unwrap_or("Файл".into());
    let mime = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_owned();
    if headers
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .is_some_and(|n| n > MAX_UPLOAD)
    {
        return Err(bad("Файл больше 512 МБ"));
    }
    let (root, tools, gate) = {
        let state = state.lock().unwrap();
        (
            state.root.clone(),
            state.media_tools.clone(),
            state.media_gate.clone(),
        )
    };
    let _permit = gate.acquire_owned().await.map_err(bad)?;
    let id = Uuid::new_v4();
    let folder = root.join("imports").join(id.to_string());
    tokio::fs::create_dir_all(&folder).await.map_err(bad)?;
    let source = folder.join("original");
    let mut file = tokio::fs::File::create(&source).await.map_err(bad)?;
    let mut body = request.into_body().into_data_stream();
    let mut total = 0u64;
    use futures_util::StreamExt;
    while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(bad)?;
        total += chunk.len() as u64;
        if total > MAX_UPLOAD {
            return Err(bad("Файл больше 512 МБ"));
        }
        file.write_all(&chunk).await.map_err(bad)?;
    }
    file.sync_all().await.map_err(bad)?;
    drop(file);
    let (ready, width, height) = if let Some(tools) = tools {
        let (path, _, width, height) = prepare(&tools, &source, &folder).await.map_err(bad)?;
        (path, width, height)
    } else {
        (source.clone(), 0, 0)
    };
    use std::io::Read;
    let mut header = [0u8; 32];
    let read = std::fs::File::open(&ready)
        .and_then(|mut f| f.read(&mut header))
        .map_err(bad)?;
    let (kind, ready_mime) = sniff(&header[..read]).map_err(bad)?;
    let bytes = std::fs::metadata(&ready).map_err(bad)?.len();
    let asset = MediaAsset {
        id,
        name,
        kind,
        mime: ready_mime.into(),
        bytes,
        attribution: None,
        width,
        height,
        original_mime: Some(mime),
    };
    let converted = root.join("media").join(id.to_string());
    let temporary = converted.with_extension("part");
    let original = root.join("originals").join(id.to_string());
    std::fs::create_dir_all(root.join("originals")).map_err(bad)?;
    let mut state = state.lock().unwrap();
    let result = (|| -> Result<(), String> {
        std::fs::copy(&ready, &temporary).map_err(|e| e.to_string())?;
        std::fs::File::open(&temporary)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
        std::fs::rename(&temporary, &converted).map_err(|e| e.to_string())?;
        std::fs::rename(&source, &original).map_err(|e| e.to_string())?;
        for directory in [root.join("media"), root.join("originals")] {
            std::fs::File::open(directory)
                .and_then(|f| f.sync_all())
                .map_err(|e| e.to_string())?;
        }
        state.store.add_asset(&asset)?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temporary);
        let _ = std::fs::remove_file(&converted);
        if original.exists() {
            let _ = std::fs::rename(&original, &source);
        }
        return Err(bad(error));
    }
    state.playback.assets.push(asset.clone());
    state.playback.revision += 1;
    let _ = std::fs::remove_dir_all(folder);
    Ok(Json(asset))
}
pub async fn original(
    State(state): State<Shared>,
    Path(id): Path<Uuid>,
    request: axum::extract::Request,
) -> Response {
    let path = {
        let state = state.lock().unwrap();
        if !state.playback.assets.iter().any(|a| a.id == id) {
            return StatusCode::NOT_FOUND.into_response();
        }
        state.root.join("originals").join(id.to_string())
    };
    let mut response = tower_http::services::ServeFile::new_with_mime(
        path,
        &"application/octet-stream".parse().unwrap(),
    )
    .try_call(request)
    .await
    .unwrap()
    .into_response();
    response
        .headers_mut()
        .insert("content-disposition", "attachment".parse().unwrap());
    response
}

pub async fn poster(
    State(state): State<Shared>,
    Path(id): Path<Uuid>,
    request: axum::extract::Request,
) -> Response {
    let (root, tools, gate) = {
        let state = state.lock().unwrap();
        if !state.playback.assets.iter().any(|a| a.id == id) {
            return StatusCode::NOT_FOUND.into_response();
        }
        (
            state.root.clone(),
            state.media_tools.clone(),
            state.media_gate.clone(),
        )
    };
    let folder = root.join("previews");
    let path = folder.join(format!("{id}.jpg"));
    if !path.exists() {
        let Some(tools) = tools else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let Ok(_permit) = gate.acquire_owned().await else {
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        };
        if !path.exists() {
            if tokio::fs::create_dir_all(&folder).await.is_err() {
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
            let temporary = folder.join(format!("{id}.part.jpg"));
            let result = Command::new(binary(&tools, "ffmpeg"))
                .args([
                    "-hide_banner",
                    "-loglevel",
                    "error",
                    "-nostdin",
                    "-y",
                    "-threads",
                    "1",
                    "-i",
                ])
                .arg(root.join("media").join(id.to_string()))
                .args([
                    "-frames:v",
                    "1",
                    "-vf",
                    "scale=480:480:force_original_aspect_ratio=decrease",
                    "-q:v",
                    "3",
                ])
                .arg(&temporary)
                .kill_on_drop(true)
                .output();
            let ok = tokio::time::timeout(Duration::from_secs(30), result)
                .await
                .ok()
                .and_then(Result::ok)
                .is_some_and(|output| output.status.success());
            if !ok {
                let _ = tokio::fs::remove_file(temporary).await;
                return StatusCode::UNPROCESSABLE_ENTITY.into_response();
            }
            if tokio::fs::rename(temporary, &path).await.is_err() {
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
    }
    let mut response =
        tower_http::services::ServeFile::new_with_mime(path, &"image/jpeg".parse().unwrap())
            .try_call(request)
            .await
            .unwrap()
            .into_response();
    response.headers_mut().insert(
        "cache-control",
        "private, max-age=86400, immutable".parse().unwrap(),
    );
    response
        .headers_mut()
        .insert("vary", "Cookie".parse().unwrap());
    response
}

#[cfg(test)]
mod tests {
    use super::compatible_video;
    use serde_json::json;
    #[test]
    fn keeps_lightweight_sdr_but_normalizes_high_fps_hdr_rotation_and_other_codecs() {
        let safe = json!({"streams":[{"codec_type":"video","codec_name":"h264","pix_fmt":"yuv420p","width":854,"height":480,"avg_frame_rate":"24/1"},{"codec_type":"audio","codec_name":"aac"}]});
        assert!(compatible_video(&safe));
        for (key, value) in [
            ("avg_frame_rate", json!("60/1")),
            ("avg_frame_rate", json!("0/0")),
            ("codec_name", json!("hevc")),
            ("height", json!(1920)),
            ("color_transfer", json!("smpte2084")),
            ("color_transfer", json!("arib-std-b67")),
            ("pix_fmt", json!("yuv420p10le")),
            ("side_data_list", json!([{"rotation":90}])),
        ] {
            let mut unsafe_video = safe.clone();
            unsafe_video["streams"][0][key] = value;
            assert!(!compatible_video(&unsafe_video), "{key}");
        }
        let mut audio = safe;
        audio["streams"][1]["codec_name"] = json!("pcm_s16le");
        assert!(!compatible_video(&audio));
    }
}
