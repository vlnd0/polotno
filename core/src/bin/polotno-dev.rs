use polotno_core::Server;
use std::{io, path::PathBuf};

fn main() -> Result<(), String> {
    let root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or("runtime".into());
    let editor = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../web/dist");
    if editor.exists() {
        std::fs::create_dir_all(root.join("editor/assets")).map_err(|e| e.to_string())?;
        std::fs::copy(editor.join("index.html"), root.join("editor/index.html"))
            .map_err(|e| e.to_string())?;
        for entry in std::fs::read_dir(editor.join("assets")).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            std::fs::copy(
                entry.path(),
                root.join("editor/assets").join(entry.file_name()),
            )
            .map_err(|e| e.to_string())?;
        }
        std::fs::create_dir_all(root.join("editor/wallpapers")).map_err(|e| e.to_string())?;
        for entry in std::fs::read_dir(editor.join("wallpapers")).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            std::fs::copy(
                entry.path(),
                root.join("editor/wallpapers").join(entry.file_name()),
            )
            .map_err(|e| e.to_string())?;
        }
    }
    let server = Server::start(&root, 8787)?;
    let code = server.state.lock().unwrap().pairing_code.clone().unwrap();
    println!(
        "Полотно: http://localhost:{}/#pair={}\nEnter — остановить сервер",
        server.port, code
    );
    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    Ok(())
}
