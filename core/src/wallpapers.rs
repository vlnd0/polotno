use crate::{
    catalog::{CatalogItem, CatalogPage},
    model::{Attribution, MediaAsset},
    server::{bad, commit_upload_attributed, ApiError, Shared},
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;

#[derive(Clone, Deserialize)]
struct Wallpaper {
    id: u64,
    title: String,
    description: String,
    category: String,
    kind: String,
    file: String,
    poster: String,
    duration_seconds: u32,
}
fn manifest() -> Result<Vec<Wallpaper>, String> {
    serde_json::from_str(include_str!("../../web/public/wallpapers/manifest.json"))
        .map_err(|e| e.to_string())
}
fn find(id: u64) -> Result<Wallpaper, ApiError> {
    manifest()
        .map_err(bad)?
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| bad("Обои не найдены"))
}
pub fn browse(kind: Option<&str>) -> Result<CatalogPage, String> {
    let items = manifest()?
        .into_iter()
        .filter(|item| kind.is_none_or(|kind| item.kind == kind))
        .map(|item| CatalogItem {
            id: item.id,
            provider: "polotno".into(),
            title: item.title,
            artist: item.description,
            date: if item.kind == "video" {
                format!("{} с · плавный цикл", item.duration_seconds)
            } else {
                "".into()
            },
            preview_url: format!("/api/wallpapers/{}/preview", item.id),
            source_url: "https://github.com/vlnd0/polotno".into(),
            rights: "Оригинальная графика Полотна · MIT".into(),
            video_url: if item.kind == "video" {
                Some(format!("/api/wallpapers/{}/video", item.id))
            } else {
                None
            },
            kind: item.kind,
            category: item.category,
        })
        .collect();
    Ok(CatalogPage {
        items,
        page: 1,
        has_more: false,
    })
}
pub async fn import(state: Shared, id: u64) -> Result<Json<MediaAsset>, ApiError> {
    let item = find(id)?;
    let provider_id = format!("polotno:{id}");
    let path = {
        let state = state.lock().unwrap();
        if let Some(asset) = state.playback.assets.iter().find(|a| {
            a.attribution
                .as_ref()
                .is_some_and(|a| a.provider_id == provider_id)
        }) {
            return Ok(Json(asset.clone()));
        }
        state.root.join("editor/wallpapers").join(&item.file)
    };
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|_| bad("Не удалось прочитать встроенные обои"))?;
    let info = Attribution {
        title: item.title.clone(),
        artist: "Полотно".into(),
        source_url: "https://github.com/vlnd0/polotno".into(),
        rights: "Оригинальная графика · MIT".into(),
        provider_id,
    };
    let mut state = state.lock().unwrap();
    if let Some(asset) = state.playback.assets.iter().find(|a| {
        a.attribution
            .as_ref()
            .is_some_and(|a| a.provider_id == info.provider_id)
    }) {
        return Ok(Json(asset.clone()));
    }
    Ok(Json(
        commit_upload_attributed(&mut state, &bytes, &item.title, Some(info)).map_err(bad)?,
    ))
}
pub async fn serve(
    State(state): State<Shared>,
    Path((id, variant)): Path<(u64, String)>,
    request: axum::extract::Request,
) -> Response {
    let Ok(item) = find(id) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let (file, mime) = match variant.as_str() {
        "preview" => (item.poster, "image/jpeg"),
        "video" if item.kind == "video" => (item.file, "video/mp4"),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    let path = state
        .lock()
        .unwrap()
        .root
        .join("editor/wallpapers")
        .join(file);
    let mut response = tower_http::services::ServeFile::new_with_mime(path, &mime.parse().unwrap())
        .try_call(request)
        .await
        .unwrap()
        .into_response();
    if response.status().is_success() {
        response
            .headers_mut()
            .insert("cache-control", "private, max-age=86400".parse().unwrap());
        response
            .headers_mut()
            .insert("vary", "Cookie".parse().unwrap());
    }
    response
}
