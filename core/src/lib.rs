pub mod catalog;
pub mod media;
pub mod model;
pub mod pairing;
pub mod server;
pub mod store;
pub mod wallpapers;

use jni::{
    objects::{JClass, JString},
    sys::{jint, jlong, jstring},
    JNIEnv,
};
use server::{AppState, ServerInfo, Shared};
use std::{path::Path, sync::Mutex, time::Duration};
use tokio::{runtime::Runtime, sync::oneshot};

pub struct Server {
    runtime: Option<Runtime>,
    shutdown: Option<oneshot::Sender<()>>,
    pub state: Shared,
    pub port: u16,
}
impl Server {
    pub fn start(root: &Path, port: u16) -> Result<Self, String> {
        let state = AppState::open(root)?;
        let runtime = Runtime::new().map_err(|e| e.to_string())?;
        let listener = match runtime.block_on(tokio::net::TcpListener::bind(("0.0.0.0", port))) {
            Ok(listener) => listener,
            Err(error) if port != 0 && error.kind() == std::io::ErrorKind::AddrInUse => runtime
                .block_on(tokio::net::TcpListener::bind(("0.0.0.0", 0)))
                .map_err(|e| e.to_string())?,
            Err(error) => return Err(error.to_string()),
        };
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let (shutdown, receiver) = oneshot::channel();
        let router = server::router(state.clone());
        runtime.spawn(async move {
            let _ = axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    let _ = receiver.await;
                })
                .await;
        });
        Ok(Self {
            runtime: Some(runtime),
            shutdown: Some(shutdown),
            state,
            port,
        })
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_timeout(Duration::from_secs(2));
        }
    }
}
static SERVER: Mutex<Option<Server>> = Mutex::new(None);

fn to_java(env: &mut JNIEnv, value: Result<String, String>) -> jstring {
    match value {
        Ok(value) => env
            .new_string(value)
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut()),
        Err(_) => {
            let _ = env.throw_new(
                "java/lang/IllegalStateException",
                "Не удалось открыть локальную галерею",
            );
            std::ptr::null_mut()
        }
    }
}
#[no_mangle]
pub extern "system" fn Java_app_polotno_NativeCore_start(
    mut env: JNIEnv,
    _: JClass,
    path: JString,
    port: jint,
) -> jstring {
    let result = (|| {
        let path: String = env.get_string(&path).map_err(|e| e.to_string())?.into();
        let mut slot = SERVER.lock().unwrap();
        if slot.is_none() {
            *slot = Some(Server::start(
                Path::new(&path),
                u16::try_from(port).map_err(|e| e.to_string())?,
            )?);
        }
        let server = slot.as_ref().unwrap();
        crate::media::init_java(&mut env)?;
        let mut state = server.state.lock().unwrap();
        let code = state.rotate_pairing();
        serde_json::to_string(&ServerInfo {
            port: server.port,
            display_code: crate::pairing::display_code(&code),
            pairing_code: code,
            has_paired_clients: !state.browser_sessions.is_empty(),
        })
        .map_err(|e| e.to_string())
    })();
    to_java(&mut env, result)
}
#[no_mangle]
pub extern "system" fn Java_app_polotno_NativeCore_snapshot(mut env: JNIEnv, _: JClass) -> jstring {
    let result = (|| {
        let slot = SERVER.lock().unwrap();
        let server = slot.as_ref().ok_or("Сервер остановлен")?;
        let state = server.state.lock().unwrap();
        let mut value = serde_json::to_value(&state.playback).map_err(|e| e.to_string())?;
        value["controller_epoch"] = serde_json::json!(state.controller_epoch);
        serde_json::to_string(&value).map_err(|e| e.to_string())
    })();
    to_java(&mut env, result)
}
#[no_mangle]
pub extern "system" fn Java_app_polotno_NativeCore_revision(_: JNIEnv, _: JClass) -> jlong {
    let slot = SERVER.lock().unwrap();
    let Some(server) = slot.as_ref() else {
        return -1;
    };
    let revision = server.state.lock().unwrap().playback.revision as jlong;
    revision
}
#[no_mangle]
pub extern "system" fn Java_app_polotno_NativeCore_command(
    mut env: JNIEnv,
    _: JClass,
    command: JString,
) {
    let result = (|| {
        let command: String = env.get_string(&command).map_err(|e| e.to_string())?.into();
        let slot = SERVER.lock().unwrap();
        let server = slot.as_ref().ok_or("Сервер остановлен")?;
        let result = server.state.lock().unwrap().command(&command);
        result
    })();
    if result.is_err() {
        let _ = env.throw_new(
            "java/lang/IllegalStateException",
            "Не удалось выполнить команду",
        );
    }
}
#[no_mangle]
pub extern "system" fn Java_app_polotno_NativeCore_stop(_: JNIEnv, _: JClass) {
    SERVER.lock().unwrap().take();
}
#[no_mangle]
pub extern "system" fn Java_app_polotno_NativeCore_configureMedia(
    mut env: JNIEnv,
    _: JClass,
    directory: JString,
) {
    if let Ok(path) = env.get_string(&directory) {
        let path: String = path.into();
        if let Some(server) = SERVER.lock().unwrap().as_ref() {
            server.state.lock().unwrap().media_tools = Some(std::path::PathBuf::from(path));
        }
    }
}
