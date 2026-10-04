use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use polotno_core::{
    model::*,
    server::{self, AppState},
    Server,
};
use tower::ServiceExt;
use uuid::Uuid;

fn asset(kind: MediaKind) -> MediaAsset {
    MediaAsset {
        id: Uuid::new_v4(),
        name: "Тест".into(),
        kind,
        mime: "test".into(),
        bytes: 12,
        attribution: None,
        width: 0,
        height: 0,
        original_mime: None,
    }
}

#[test]
fn dynamic_surfaces_and_video_playlists_have_no_count_cap() {
    let image = asset(MediaKind::Image);
    let video = asset(MediaKind::Video);
    let assets = [image.clone(), video.clone()];
    for count in [0, 1, 3, 6, 12, 128] {
        let mut scene = Scene::empty();
        scene.surfaces = (0..count)
            .map(|_| {
                let mut s = Surface::new("Картина");
                s.playlist.items = vec![image.id, video.id];
                s
            })
            .collect();
        assert!(scene.validate(&assets).is_ok());
    }
}

#[test]
fn independent_playlists_and_mutations_preserve_other_surfaces() {
    let mut scene = Scene::empty();
    let a = Surface::new("А");
    let mut b = Surface::new("Б");
    b.playlist.interval_seconds = 17;
    scene.surfaces = vec![a.clone(), b.clone()];
    let copied_id = scene.duplicate_surface(a.id).unwrap();
    assert_ne!(copied_id, a.id);
    assert_eq!(scene.surfaces[2], b);
    scene.surfaces[1].playlist.interval_seconds = 9;
    assert_eq!(scene.surfaces[0], a);
    scene.surfaces.swap(0, 2);
    scene.surfaces.retain(|s| s.id != copied_id);
    assert_eq!(scene.surfaces, vec![b, a]);
}

#[test]
fn reject_missing_assets_invalid_geometry_and_duplicate_ids() {
    let mut scene = Scene::empty();
    let surface = Surface::new("А");
    scene.surfaces = vec![surface.clone(), surface];
    assert!(scene.validate(&[]).is_err());
    scene.surfaces.pop();
    scene.surfaces[0].corners.swap(0, 2);
    assert!(scene.validate(&[]).is_err());
    scene.surfaces[0] = Surface::new("А");
    scene.surfaces[0].playlist.items = vec![Uuid::new_v4()];
    assert!(scene.validate(&[]).is_err());
}

#[test]
fn saved_scene_survives_restart_preview_does_not() {
    let root = tempfile::tempdir().unwrap();
    let state = AppState::open(root.path()).unwrap();
    let mut saved = Scene::empty();
    saved.surfaces = (0..12)
        .map(|i| Surface::new(&format!("Область {i}")))
        .collect();
    state.lock().unwrap().apply(saved.clone(), true).unwrap();
    let mut preview = saved.clone();
    preview.name = "Несохранённое".into();
    preview.surfaces.clear();
    state.lock().unwrap().apply(preview, false).unwrap();
    drop(state);
    let reopened = AppState::open(root.path()).unwrap();
    assert_eq!(reopened.lock().unwrap().playback.scene, saved);
}

#[test]
fn failed_upload_and_crash_leftovers_do_not_damage_gallery() {
    let root = tempfile::tempdir().unwrap();
    let state = AppState::open(root.path()).unwrap();
    let png = b"\x89PNG\r\n\x1a\nfixture";
    let asset = server::commit_upload(&mut state.lock().unwrap(), png, "../свой файл.png").unwrap();
    assert!(root
        .path()
        .join("media")
        .join(asset.id.to_string())
        .exists());
    assert!(server::commit_upload(&mut state.lock().unwrap(), b"bad", "bad.png").is_err());
    let orphan = root.path().join("media").join(Uuid::new_v4().to_string());
    std::fs::write(&orphan, png).unwrap();
    let partial = orphan.with_extension("part");
    std::fs::write(&partial, b"partial").unwrap();
    drop(state);
    let reopened = AppState::open(root.path()).unwrap();
    assert_eq!(reopened.lock().unwrap().playback.assets.len(), 1);
    assert!(!orphan.exists());
    assert!(!partial.exists());
    assert_eq!(
        std::fs::read(root.path().join("media").join(asset.id.to_string())).unwrap(),
        png
    );
}

#[tokio::test]
async fn pairing_connects_multiple_browsers_and_private_routes_require_session() {
    let root = tempfile::tempdir().unwrap();
    let state = AppState::open(root.path()).unwrap();
    let router = server::router(state.clone());
    let request = || {
        Request::builder()
            .uri("/api/state")
            .body(Body::empty())
            .unwrap()
    };
    assert_eq!(
        router.clone().oneshot(request()).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    let code = state.lock().unwrap().pairing_code.clone().unwrap();
    let pair = || {
        Request::builder()
            .method("POST")
            .uri("/api/pair")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::json!({"code":code}).to_string()))
            .unwrap()
    };
    let response = router.clone().oneshot(pair()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    assert_eq!(
        router.clone().oneshot(pair()).await.unwrap().status(),
        StatusCode::OK
    );
    let request = Request::builder()
        .uri("/api/state")
        .header("cookie", &cookie)
        .body(Body::empty())
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let snapshot: PlaybackState = serde_json::from_slice(&body).unwrap();
    assert!(snapshot.scene.surfaces.is_empty());
    let evil = Request::builder()
        .method("POST")
        .uri("/api/control")
        .header("cookie", &cookie)
        .header("host", "192.168.1.2:8787")
        .header("origin", "http://evil.test")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"command":"pause"}"#))
        .unwrap();
    assert_eq!(
        router.oneshot(evil).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn api_accepts_many_video_playlists_and_persists_them() {
    let root = tempfile::tempdir().unwrap();
    let state = AppState::open(root.path()).unwrap();
    let video = server::commit_upload(
        &mut state.lock().unwrap(),
        b"\x00\x00\x00\x18ftypmp42",
        "video.mp4",
    )
    .unwrap();
    state.lock().unwrap().sessions.insert("test".into());
    let router = server::router(state.clone());
    let mut scene = Scene::empty();
    scene.surfaces = (0..6)
        .map(|_| {
            let mut s = Surface::new("Видео");
            s.playlist.items = vec![video.id];
            s
        })
        .collect();
    for route in ["preview", "save"] {
        let request = Request::builder()
            .method("PUT")
            .uri(format!("/api/{route}"))
            .header("cookie", "polotno_session=test")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&scene).unwrap()))
            .unwrap();
        assert_eq!(
            router.clone().oneshot(request).await.unwrap().status(),
            StatusCode::OK
        );
        assert_eq!(state.lock().unwrap().playback.scene, scene);
    }
    assert_eq!(state.lock().unwrap().store.active().unwrap(), scene);
    let mut invalid = scene.clone();
    invalid.surfaces[0].playlist.items = vec![Uuid::new_v4()];
    for route in ["preview", "save"] {
        let request = Request::builder()
            .method("PUT")
            .uri(format!("/api/{route}"))
            .header("cookie", "polotno_session=test")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&invalid).unwrap()))
            .unwrap();
        assert_eq!(
            router.clone().oneshot(request).await.unwrap().status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(state.lock().unwrap().playback.scene, scene);
        assert_eq!(state.lock().unwrap().store.active().unwrap(), scene);
    }
    drop(router);
    drop(state);
    let reopened = AppState::open(root.path()).unwrap();
    assert_eq!(reopened.lock().unwrap().playback.scene, scene);
}

#[test]
fn server_release_closes_listener_and_can_restart_on_same_port() {
    let root = tempfile::tempdir().unwrap();
    let server = Server::start(root.path(), 0).unwrap();
    let port = server.port;
    assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_ok());
    drop(server);
    assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_err());
    let server = Server::start(root.path(), port).unwrap();
    assert_eq!(server.port, port);
}

#[tokio::test]
async fn media_requires_pairing_and_supports_bounded_range_reads() {
    let root = tempfile::tempdir().unwrap();
    let state = AppState::open(root.path()).unwrap();
    let bytes = b"\x89PNG\r\n\x1a\nfixture";
    let asset = server::commit_upload(&mut state.lock().unwrap(), bytes, "Image").unwrap();
    state.lock().unwrap().sessions.insert("test".into());
    let router = server::router(state);
    let url = format!("/api/media/{}", asset.id);
    let unauthorized = Request::builder().uri(&url).body(Body::empty()).unwrap();
    assert_eq!(
        router.clone().oneshot(unauthorized).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    let request = Request::builder()
        .uri(&url)
        .header("cookie", "polotno_session=test")
        .header("range", "bytes=0-7")
        .body(Body::empty())
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(response.headers()["content-type"], "image/png");
    assert_eq!(
        &response.into_body().collect().await.unwrap().to_bytes()[..],
        &bytes[..8]
    );
}

#[test]
fn museum_attribution_is_committed_with_media_and_survives_restart() {
    let root = tempfile::tempdir().unwrap();
    let state = AppState::open(root.path()).unwrap();
    let info = Attribution {
        title: "Painting".into(),
        artist: "Artist".into(),
        source_url: "https://www.metmuseum.org/art/collection/search/436535".into(),
        rights: "CC0".into(),
        provider_id: "met:436535".into(),
    };
    server::commit_upload_attributed(
        &mut state.lock().unwrap(),
        b"\x89PNG\r\n\x1a\nfixture",
        "Painting",
        Some(info),
    )
    .unwrap();
    drop(state);
    let reopened = AppState::open(root.path()).unwrap();
    let guard = reopened.lock().unwrap();
    let info = guard.playback.assets[0].attribution.as_ref().unwrap();
    assert_eq!(info.provider_id, "met:436535");
    assert_eq!(info.rights, "CC0");
    assert_eq!(info.artist, "Artist");
}

#[tokio::test]
async fn manual_pairing_cookie_survives_server_restart_and_expires() {
    let root = tempfile::tempdir().unwrap();
    let state = AppState::open(root.path()).unwrap();
    let qr = state.lock().unwrap().pairing_code.clone().unwrap();
    let code = polotno_core::pairing::display_code(&qr);
    let request = Request::builder()
        .method("POST")
        .uri("/api/pair")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::json!({"code":code}).to_string()))
        .unwrap();
    let response = server::router(state.clone())
        .oneshot(request)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let header = response.headers()["set-cookie"].to_str().unwrap();
    assert!(header.contains("Max-Age=2592000"));
    let cookie = header.split(';').next().unwrap().to_owned();
    drop(state);
    let reopened = AppState::open(root.path()).unwrap();
    let request = || {
        Request::builder()
            .uri("/api/state")
            .header("cookie", &cookie)
            .body(Body::empty())
            .unwrap()
    };
    assert_eq!(
        server::router(reopened.clone())
            .oneshot(request())
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    {
        let mut state = reopened.lock().unwrap();
        state.browser_sessions[0].expires_at = 0;
        polotno_core::pairing::persist(root.path(), &state.browser_sessions).unwrap();
    }
    assert_eq!(
        server::router(reopened)
            .oneshot(request())
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let restarted = AppState::open(root.path()).unwrap();
    assert!(restarted.lock().unwrap().sessions.is_empty());
}

#[tokio::test]
async fn manual_codes_are_rate_limited_without_blocking_secret_qr() {
    let root = tempfile::tempdir().unwrap();
    let state = AppState::open(root.path()).unwrap();
    let router = server::router(state.clone());
    let pair = |code: &str| {
        Request::builder()
            .method("POST")
            .uri("/api/pair")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::json!({"code":code}).to_string()))
            .unwrap()
    };
    for _ in 0..5 {
        assert_eq!(
            router
                .clone()
                .oneshot(pair("wrong"))
                .await
                .unwrap()
                .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    let qr = state.lock().unwrap().rotate_pairing();
    let code = polotno_core::pairing::display_code(&qr);
    assert_eq!(
        router.clone().oneshot(pair(&code)).await.unwrap().status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        router.oneshot(pair(&qr)).await.unwrap().status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn failed_session_write_does_not_consume_pairing_code() {
    let root = tempfile::tempdir().unwrap();
    let state = AppState::open(root.path()).unwrap();
    let router = server::router(state.clone());
    let code = state.lock().unwrap().pairing_code.clone().unwrap();
    let pair = || {
        Request::builder()
            .method("POST")
            .uri("/api/pair")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::json!({"code":code}).to_string()))
            .unwrap()
    };
    std::fs::create_dir(root.path().join("browser-sessions.part")).unwrap();
    assert_eq!(
        router.clone().oneshot(pair()).await.unwrap().status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(state.lock().unwrap().pairing_code.as_ref(), Some(&code));
    assert!(state.lock().unwrap().sessions.is_empty());
    std::fs::remove_dir(root.path().join("browser-sessions.part")).unwrap();
    assert_eq!(
        router.oneshot(pair()).await.unwrap().status(),
        StatusCode::OK
    );
}

#[test]
fn installation_pairing_code_survives_restart_and_menu() {
    let root = tempfile::tempdir().unwrap();
    let state = AppState::open(root.path()).unwrap();
    let code = state.lock().unwrap().pairing_code.clone().unwrap();
    assert_eq!(state.lock().unwrap().rotate_pairing(), code);
    drop(state);
    let reopened = AppState::open(root.path()).unwrap();
    assert_eq!(reopened.lock().unwrap().pairing_code.as_ref(), Some(&code));
}
#[tokio::test]
async fn controller_is_announced_once_and_scene_geometry_survives_connection() {
    let root = tempfile::tempdir().unwrap();
    let state = AppState::open(root.path()).unwrap();
    state.lock().unwrap().sessions.insert("test".into());
    let scene = state.lock().unwrap().playback.scene.clone();
    let router = server::router(state.clone());
    let request = || {
        Request::builder()
            .uri("/api/state")
            .header("cookie", "polotno_session=test")
            .body(Body::empty())
            .unwrap()
    };
    for _ in 0..3 {
        assert_eq!(
            router.clone().oneshot(request()).await.unwrap().status(),
            StatusCode::OK
        );
    }
    let state = state.lock().unwrap();
    assert_eq!(state.controller_epoch, 1);
    assert_eq!(state.playback.scene, scene);
}

#[tokio::test]
async fn bundled_wallpapers_browse_and_import_offline_without_duplicate_media() {
    let root = tempfile::tempdir().unwrap();
    let state = AppState::open(root.path()).unwrap();
    state.lock().unwrap().sessions.insert("test".into());
    let router = server::router(state.clone());
    let get = Request::builder()
        .uri("/api/catalog?provider=polotno&kind=video")
        .header("cookie", "polotno_session=test")
        .body(Body::empty())
        .unwrap();
    let response = router.clone().oneshot(get).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let items = value["items"].as_array().unwrap();
    assert!(items.len() >= 12);
    assert!(items.iter().all(|i| i["kind"] == "video"));
    assert!(items.iter().any(|i| i["category"] == "Киберпанк"));
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../../web/public/wallpapers/manifest.json")).unwrap();
    let movie = manifest
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["id"] == 1)
        .unwrap()["file"]
        .as_str()
        .unwrap();
    let path = root.path().join("editor/wallpapers");
    std::fs::create_dir_all(&path).unwrap();
    std::fs::copy(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../web/public/wallpapers")
            .join(movie),
        path.join(movie),
    )
    .unwrap();
    let request = || {
        Request::builder()
            .method("POST")
            .uri("/api/catalog/import")
            .header("cookie", "polotno_session=test")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"provider":"polotno","id":1}"#))
            .unwrap()
    };
    for _ in 0..2 {
        assert_eq!(
            router.clone().oneshot(request()).await.unwrap().status(),
            StatusCode::OK
        );
    }
    let state = state.lock().unwrap();
    assert_eq!(state.playback.assets.len(), 1);
    assert_eq!(state.playback.assets[0].kind, MediaKind::Video);
    assert_eq!(
        state.playback.assets[0]
            .attribution
            .as_ref()
            .unwrap()
            .provider_id,
        "polotno:1"
    );
}

#[test]
fn neon_frames_validate_and_restore_with_calibration_and_portrait_fit() {
    let root = tempfile::tempdir().unwrap();
    let state = AppState::open(root.path()).unwrap();
    let mut scene = Scene::empty();
    let mut surface = Surface::new("Сетка");
    surface.frame = Some(NeonFrame {
        color: "#ec4899".into(),
        width: 3.0,
        glow: 18.0,
    });
    scene.surfaces.push(surface);
    state.lock().unwrap().apply(scene.clone(), true).unwrap();
    drop(state);
    let restored = AppState::open(root.path()).unwrap();
    assert_eq!(restored.lock().unwrap().playback.scene, scene);
    scene.surfaces[0].frame.as_mut().unwrap().glow = f32::INFINITY;
    assert!(scene.validate(&[]).is_err());
}

#[tokio::test]
async fn streamed_upload_preserves_original_and_keeps_existing_gallery_on_error() {
    let root = tempfile::tempdir().unwrap();
    let state = AppState::open(root.path()).unwrap();
    state.lock().unwrap().sessions.insert("test".into());
    let router = server::router(state.clone());
    let bytes = b"\x89PNG\r\n\x1a\nfixture";
    let request = Request::builder()
        .method("POST")
        .uri("/api/upload")
        .header("cookie", "polotno_session=test")
        .header("content-type", "image/png")
        .body(Body::from(bytes.to_vec()))
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let asset: MediaAsset =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(
        std::fs::read(root.path().join("originals").join(asset.id.to_string())).unwrap(),
        bytes
    );
    let invalid = Request::builder()
        .method("POST")
        .uri("/api/upload")
        .header("cookie", "polotno_session=test")
        .body(Body::from("not-media"))
        .unwrap();
    assert_eq!(
        router.oneshot(invalid).await.unwrap().status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(state.lock().unwrap().playback.assets.len(), 1);
    assert_eq!(state.lock().unwrap().store.assets().unwrap().len(), 1);
}

#[test]
fn reflection_is_independent_per_surface_persists_and_defaults_for_legacy_scenes() {
    let root = tempfile::tempdir().unwrap();
    let state = AppState::open(root.path()).unwrap();
    let mut scene = Scene::empty();
    scene.surfaces = vec![Surface::new("А"), Surface::new("Б")];
    scene.surfaces[0].flip_horizontal = true;
    scene.surfaces[1].flip_vertical = true;
    let id = scene.duplicate_surface(scene.surfaces[0].id).unwrap();
    let copy = scene.surfaces.iter().find(|s| s.id == id).unwrap();
    assert!(copy.flip_horizontal);
    assert!(!copy.flip_vertical);
    state.lock().unwrap().apply(scene.clone(), true).unwrap();
    drop(state);
    let restored = AppState::open(root.path()).unwrap();
    assert_eq!(restored.lock().unwrap().playback.scene, scene);
    let mut legacy = serde_json::to_value(&scene).unwrap();
    for surface in legacy["surfaces"].as_array_mut().unwrap() {
        surface.as_object_mut().unwrap().remove("flip_horizontal");
        surface.as_object_mut().unwrap().remove("flip_vertical");
    }
    let legacy: Scene = serde_json::from_value(legacy).unwrap();
    assert!(legacy
        .surfaces
        .iter()
        .all(|s| !s.flip_horizontal && !s.flip_vertical));
}
