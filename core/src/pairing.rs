use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::Path,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

pub const SESSION_SECONDS: u64 = 30 * 24 * 60 * 60;
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub fn display_code(qr: &str) -> String {
    format!(
        "{:06}",
        Uuid::parse_str(qr).expect("generated QR UUID").as_u128() % 1_000_000
    )
}

/// A private installation secret keeps the QR and human code stable across restarts.
pub fn installation_code(root: &Path) -> Result<String, String> {
    let path = root.join("pairing-secret");
    if path.exists() {
        let code = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        Uuid::parse_str(&code).map_err(|_| "Повреждён ключ подключения".to_string())?;
        return Ok(code);
    }
    let code = Uuid::new_v4().to_string();
    let temporary = root.join("pairing-secret.part");
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary).map_err(|e| e.to_string())?;
    file.write_all(code.as_bytes()).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    fs::rename(temporary, path).map_err(|e| e.to_string())?;
    File::open(root)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
    Ok(code)
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BrowserSession {
    pub token: String,
    pub expires_at: u64,
}
pub fn load(root: &Path) -> Vec<BrowserSession> {
    fs::read(root.join("browser-sessions.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Vec<BrowserSession>>(&bytes).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|s| s.expires_at > now() && Uuid::parse_str(&s.token).is_ok())
        .collect()
}
pub fn persist(root: &Path, sessions: &[BrowserSession]) -> Result<(), String> {
    let temporary = root.join("browser-sessions.part");
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary).map_err(|e| e.to_string())?;
    file.write_all(&serde_json::to_vec(sessions).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    fs::rename(temporary, root.join("browser-sessions.json")).map_err(|e| e.to_string())?;
    File::open(root)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())
}

#[derive(Default)]
pub struct AttemptGate {
    attempts: u32,
    window: Option<Instant>,
}
impl AttemptGate {
    pub fn allow(&mut self) -> bool {
        if self
            .window
            .is_none_or(|start| start.elapsed() >= Duration::from_secs(60))
        {
            self.attempts = 0;
            self.window = Some(Instant::now());
        }
        if self.attempts >= 5 {
            return false;
        }
        self.attempts += 1;
        true
    }
}
