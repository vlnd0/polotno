use crate::{
    model::*,
    pairing::{self, AttemptGate, BrowserSession},
    store::Store,
};
use axum::{
    extract::{DefaultBodyLimit, Path as ApiPath, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use uuid::Uuid;

async fn editor(State(state): State<Shared>) -> Response {
    let root = state.lock().unwrap().root.join("editor");
    match tokio::fs::read(root.join("index.html")).await {
        Ok(bytes) => ([("content-type", "text/html; charset=utf-8")], bytes).into_response(),
        Err(_) => Html("<html lang='ru'><meta charset='utf-8'><p>Редактор не собран. Выполните npm ci и npm run build в web/.</p></html>").into_response(),
    }
}
async fn editor_asset(
    State(state): State<Shared>,
    ApiPath(name): ApiPath<String>,
    request: axum::extract::Request,
) -> Response {
    if name.contains('/') || name.contains('\\') || name == ".." {
        return StatusCode::NOT_FOUND.into_response();
    }
    let path = state.lock().unwrap().root.join("editor/assets").join(&name);
    let mime = if name.ends_with(".js") {
        "text/javascript"
    } else if name.ends_with(".css") {
        "text/css"
    } else {
        return StatusCode::NOT_FOUND.into_response();
    };
    tower_http::services::ServeFile::new_with_mime(path, &mime.parse().unwrap())
        .try_call(request)
        .await
        .unwrap()
        .into_response()
}
async fn media(
    State(state): State<Shared>,
    ApiPath(id): ApiPath<Uuid>,
    request: axum::extract::Request,
) -> Response {
    let asset = state
        .lock()
        .unwrap()
        .playback
        .assets
        .iter()
        .find(|a| a.id == id)
        .cloned();
    let Some(asset) = asset else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let path = state
        .lock()
        .unwrap()
        .root
        .join("media")
        .join(id.to_string());
    let mut response =
        tower_http::services::ServeFile::new_with_mime(path, &asset.mime.parse().unwrap())
            .try_call(request)
            .await
            .unwrap()
            .into_response();
    if response.status().is_success() {
        response.headers_mut().insert(
            "cache-control",
            "private, max-age=86400, immutable".parse().unwrap(),
        );
        response
            .headers_mut()
            .insert("vary", "Cookie".parse().unwrap());
    }
    response
}

pub type Shared = Arc<Mutex<AppState>>;
pub struct AppState {
    pub store: Store,
    pub playback: PlaybackState,
    pub root: PathBuf,
    pub pairing_code: Option<String>,
    pub sessions: HashSet<String>,
    pub browser_sessions: Vec<BrowserSession>,
    pub pairing_attempts: AttemptGate,
    pub connected_controllers: HashSet<String>,
    pub controller_epoch: u64,
    pub media_tools: Option<PathBuf>,
    pub media_gate: Arc<tokio::sync::Semaphore>,
}

impl AppState {
    pub fn open(root: &Path) -> Result<Shared, String> {
        fs::create_dir_all(root.join("media")).map_err(|e| e.to_string())?;
        // Incomplete uploads and files never committed to SQLite are not gallery data.
        let store = Store::open(&root.join("polotno.sqlite"))?;
        let assets = store.assets()?;
        let known: HashSet<_> = assets.iter().map(|a| a.id.to_string()).collect();
        for entry in fs::read_dir(root.join("media")).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if !known.contains(&name) && (name.ends_with(".part") || Uuid::parse_str(&name).is_ok())
            {
                fs::remove_file(entry.path()).map_err(|e| e.to_string())?;
            }
        }
        let scene = store.active()?;
        scene.validate(&assets)?;
        let browser_sessions = pairing::load(root);
        let sessions = browser_sessions
            .iter()
            .map(|session| session.token.clone())
            .collect();
        Ok(Arc::new(Mutex::new(Self {
            store,
            playback: PlaybackState {
                scene,
                assets,
                revision: 1,
                paused: false,
            },
            root: root.into(),
            pairing_code: Some(pairing::installation_code(root)?),
            sessions,
            browser_sessions,
            pairing_attempts: AttemptGate::default(),
            connected_controllers: HashSet::new(),
            controller_epoch: 0,
            media_tools: None,
            media_gate: Arc::new(tokio::sync::Semaphore::new(1)),
        })))
    }
    pub fn apply(&mut self, scene: Scene, save: bool) -> Result<(), String> {
        scene.validate(&self.playback.assets)?;
        if save {
            self.store.save_and_select(&scene)?;
        }
        self.playback.scene = scene;
        self.playback.revision += 1;
        Ok(())
    }
    pub fn command(&mut self, command: &str) -> Result<(), String> {
        match command {
            "pause" => self.playback.paused = !self.playback.paused,
            "next" | "previous" => {
                let scenes = self.store.scenes()?;
                let index = scenes
                    .iter()
                    .position(|s| s.id == self.playback.scene.id)
                    .unwrap_or(0);
                let next = if command == "next" {
                    (index + 1) % scenes.len()
                } else {
                    (index + scenes.len() - 1) % scenes.len()
                };
                self.apply(scenes[next].clone(), true)?;
                return Ok(());
            }
            _ => return Err("Неизвестная команда".into()),
        }
        self.playback.revision += 1;
        Ok(())
    }
    pub fn rotate_pairing(&mut self) -> String {
        // Showing Menu or pairing a second browser must not replace the installation's code.
        self.pairing_code
            .clone()
            .expect("installation pairing code")
    }
}

pub(crate) type ApiError = (StatusCode, Json<serde_json::Value>);
fn error(status: StatusCode, message: impl ToString) -> ApiError {
    (
        status,
        Json(serde_json::json!({"error": message.to_string()})),
    )
}
pub(crate) fn bad(message: impl ToString) -> ApiError {
    error(StatusCode::BAD_REQUEST, message)
}

pub fn router(state: Shared) -> Router {
    let private = Router::new()
        .route("/api/state", get(playback))
        .route("/api/scenes", get(scenes))
        .route("/api/preview", put(preview))
        .route("/api/save", put(save))
        .route("/api/upload", post(upload))
        .route("/api/original/{id}", get(crate::media::original))
        .route("/api/control", post(control))
        .route("/api/media/{id}", get(media))
        .route("/api/media/{id}/poster", get(crate::media::poster))
        .route("/api/catalog", get(crate::catalog::browse))
        .route("/api/catalog/import", post(crate::catalog::import))
        .route(
            "/api/wallpapers/{id}/{variant}",
            get(crate::wallpapers::serve),
        )
        .route_layer(middleware::from_fn_with_state(state.clone(), authorize));
    Router::new()
        .route("/", get(editor))
        .route("/assets/{name}", get(editor_asset))
        .route(
            "/health",
            get(|| async {
                Json(
                    serde_json::json!({"product": "polotno", "version": env!("CARGO_PKG_VERSION")}),
                )
            }),
        )
        .route("/api/pair", post(pair))
        .merge(private)
        .layer(DefaultBodyLimit::max(512 * 1024 * 1024))
        .layer(middleware::from_fn(same_origin))
        .with_state(state)
}

// No CORS. Refuse cross-origin browser writes, including pairing and cookies.
async fn same_origin(request: axum::extract::Request, next: Next) -> Response {
    if let Some(origin) = request.headers().get("origin") {
        let expected = request
            .headers()
            .get("host")
            .and_then(|h| h.to_str().ok())
            .map(|h| format!("http://{h}"));
        if origin.to_str().ok() != expected.as_deref() {
            return error(
                StatusCode::FORBIDDEN,
                "Запрос должен исходить с адреса проектора",
            )
            .into_response();
        }
    }
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .entry("cache-control")
        .or_insert("no-store".parse().unwrap());
    response
        .headers_mut()
        .insert("x-content-type-options", "nosniff".parse().unwrap());
    response
        .headers_mut()
        .insert("referrer-policy", "no-referrer".parse().unwrap());
    response
}

async fn authorize(
    State(state): State<Shared>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let token = request
        .headers()
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.split(';')
                .find_map(|p| p.trim().strip_prefix("polotno_session="))
        })
        .unwrap_or("");
    let allowed = {
        let mut state = state.lock().unwrap();
        let allowed = state.sessions.contains(token)
            && state
                .browser_sessions
                .iter()
                .find(|session| session.token == token)
                .is_none_or(|session| session.expires_at > pairing::now());
        if allowed && state.connected_controllers.insert(token.to_string()) {
            state.controller_epoch += 1;
            state.playback.revision += 1;
        }
        allowed
    };
    if !allowed {
        return error(StatusCode::UNAUTHORIZED, "Откройте QR-код на проекторе").into_response();
    }
    next.run(request).await
}

#[derive(Deserialize)]
struct PairRequest {
    code: String,
}
async fn pair(
    State(state): State<Shared>,
    Json(request): Json<PairRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let mut state = state.lock().unwrap();
    let code = request.code.trim();
    let matches = state
        .pairing_code
        .as_deref()
        .is_some_and(|qr| qr == code || pairing::display_code(qr) == code);
    let numeric = code.len() == 6 && code.bytes().all(|c| c.is_ascii_digit());
    if (!matches || numeric) && !state.pairing_attempts.allow() {
        return Err(error(
            StatusCode::TOO_MANY_REQUESTS,
            "Слишком много попыток. Подождите минуту",
        ));
    }
    if !matches {
        return Err(error(
            StatusCode::UNAUTHORIZED,
            "QR-код устарел. Покажите новый на проекторе",
        ));
    }
    if state.sessions.len() >= 32 {
        return Err(error(
            StatusCode::TOO_MANY_REQUESTS,
            "Перезапустите галерею для новых подключений",
        ));
    }
    let token = Uuid::new_v4().to_string();
    let mut sessions: Vec<_> = state
        .browser_sessions
        .iter()
        .filter(|s| s.expires_at > pairing::now())
        .cloned()
        .collect();
    sessions.push(BrowserSession {
        token: token.clone(),
        expires_at: pairing::now() + pairing::SESSION_SECONDS,
    });
    pairing::persist(&state.root, &sessions).map_err(|_| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Не удалось сохранить подключение. Попробуйте снова",
        )
    })?;
    state.browser_sessions = sessions;
    state.sessions.insert(token.clone());
    let mut headers = HeaderMap::new();
    headers.insert(
        "set-cookie",
        format!(
            "polotno_session={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age={}",
            pairing::SESSION_SECONDS
        )
        .parse()
        .unwrap(),
    );
    Ok((headers, Json(serde_json::json!({"paired": true}))))
}
async fn playback(State(state): State<Shared>) -> Json<PlaybackState> {
    Json(state.lock().unwrap().playback.clone())
}
async fn scenes(State(state): State<Shared>) -> Result<Json<Vec<Scene>>, ApiError> {
    Ok(Json(state.lock().unwrap().store.scenes().map_err(bad)?))
}
async fn preview(
    State(state): State<Shared>,
    Json(scene): Json<Scene>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut state = state.lock().unwrap();
    state.apply(scene, false).map_err(bad)?;
    Ok(Json(
        serde_json::json!({"revision":state.playback.revision}),
    ))
}
async fn save(
    State(state): State<Shared>,
    Json(scene): Json<Scene>,
) -> Result<Json<PlaybackState>, ApiError> {
    let mut state = state.lock().unwrap();
    state.apply(scene, true).map_err(bad)?;
    Ok(Json(state.playback.clone()))
}
#[derive(Deserialize)]
struct Control {
    command: String,
}
async fn control(
    State(state): State<Shared>,
    Json(request): Json<Control>,
) -> Result<Json<PlaybackState>, ApiError> {
    let mut state = state.lock().unwrap();
    state.command(&request.command).map_err(bad)?;
    Ok(Json(state.playback.clone()))
}

pub fn sniff(bytes: &[u8]) -> Result<(MediaKind, &'static str), String> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Ok((MediaKind::Image, "image/png"))
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Ok((MediaKind::Image, "image/jpeg"))
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Ok((MediaKind::Image, "image/webp"))
    } else if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
        Ok((MediaKind::Video, "video/mp4"))
    } else {
        Err("Поддерживаются JPEG, PNG, WebP и MP4".into())
    }
}

pub fn commit_upload(state: &mut AppState, bytes: &[u8], name: &str) -> Result<MediaAsset, String> {
    commit_upload_attributed(state, bytes, name, None)
}
pub fn commit_upload_attributed(
    state: &mut AppState,
    bytes: &[u8],
    name: &str,
    attribution: Option<Attribution>,
) -> Result<MediaAsset, String> {
    let (kind, mime) = sniff(bytes)?;
    let asset = MediaAsset {
        id: Uuid::new_v4(),
        name: name.chars().take(200).collect(),
        kind,
        mime: mime.into(),
        bytes: bytes.len() as u64,
        attribution,
        width: 0,
        height: 0,
        original_mime: None,
    };
    let final_path = state.root.join("media").join(asset.id.to_string());
    let temporary = final_path.with_extension("part");
    let result = (|| {
        let mut file = File::create(&temporary).map_err(|e| e.to_string())?;
        file.write_all(bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        fs::rename(&temporary, &final_path).map_err(|e| e.to_string())?;
        File::open(state.root.join("media"))
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
        state.store.add_asset(&asset)?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary);
        let _ = fs::remove_file(&final_path);
        return Err(error);
    }
    state.playback.assets.push(asset.clone());
    state.playback.revision += 1;
    Ok(asset)
}
async fn upload(
    State(state): State<Shared>,
    request: axum::extract::Request,
) -> Result<Json<MediaAsset>, ApiError> {
    crate::media::upload(state, request).await
}

#[derive(Serialize)]
pub struct ServerInfo {
    pub port: u16,
    pub pairing_code: String,
    pub display_code: String,
    pub has_paired_clients: bool,
}
