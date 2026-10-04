use crate::model::{MediaAsset, Scene};
use rusqlite::{params, Connection};
use std::path::Path;

pub struct Store {
    connection: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self, String> {
        let connection = Connection::open(path).map_err(|e| e.to_string())?;
        connection
            .execute_batch(
                "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
            CREATE TABLE IF NOT EXISTS scenes (id TEXT PRIMARY KEY, data TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS media (id TEXT PRIMARY KEY, data TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
            )
            .map_err(|e| e.to_string())?;
        let store = Self { connection };
        if store.scenes()?.is_empty() {
            store.save_and_select(&Scene::empty())?;
        }
        Ok(store)
    }
    pub fn scenes(&self) -> Result<Vec<Scene>, String> {
        self.read_json("SELECT data FROM scenes ORDER BY rowid")
    }
    pub fn assets(&self) -> Result<Vec<MediaAsset>, String> {
        self.read_json("SELECT data FROM media ORDER BY rowid")
    }
    fn read_json<T: serde::de::DeserializeOwned>(&self, sql: &str) -> Result<Vec<T>, String> {
        let mut query = self.connection.prepare(sql).map_err(|e| e.to_string())?;
        let rows = query
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.map(|row| {
            serde_json::from_str(&row.map_err(|e| e.to_string())?).map_err(|e| e.to_string())
        })
        .collect()
    }
    pub fn active(&self) -> Result<Scene, String> {
        let id: String = self
            .connection
            .query_row(
                "SELECT value FROM settings WHERE key='active_scene'",
                [],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        self.scenes()?
            .into_iter()
            .find(|s| s.id.to_string() == id)
            .ok_or("Сохранённая сцена не найдена".into())
    }
    pub fn save_and_select(&self, scene: &Scene) -> Result<(), String> {
        let tx = self
            .connection
            .unchecked_transaction()
            .map_err(|e| e.to_string())?;
        tx.execute(
            "INSERT INTO scenes VALUES (?1,?2) ON CONFLICT(id) DO UPDATE SET data=excluded.data",
            params![
                scene.id.to_string(),
                serde_json::to_string(scene).map_err(|e| e.to_string())?
            ],
        )
        .map_err(|e| e.to_string())?;
        tx.execute("INSERT INTO settings VALUES ('active_scene',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [scene.id.to_string()]).map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())
    }
    pub fn add_asset(&self, asset: &MediaAsset) -> Result<(), String> {
        self.connection
            .execute(
                "INSERT INTO media VALUES (?1,?2)",
                params![
                    asset.id.to_string(),
                    serde_json::to_string(asset).map_err(|e| e.to_string())?
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn update_asset(&self, asset: &MediaAsset) -> Result<(), String> {
        self.connection
            .execute(
                "UPDATE media SET data=?2 WHERE id=?1",
                params![
                    asset.id.to_string(),
                    serde_json::to_string(asset).map_err(|e| e.to_string())?
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
