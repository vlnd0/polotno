use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Playlist {
    pub items: Vec<Uuid>,
    pub interval_seconds: u32,
    pub muted: bool,
}

impl Default for Playlist {
    fn default() -> Self {
        Self {
            items: vec![],
            interval_seconds: 60,
            muted: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Surface {
    pub id: Uuid,
    pub name: String,
    /// Clockwise: top-left, top-right, bottom-right, bottom-left; normalized frame coordinates.
    pub corners: [Point; 4],
    pub playlist: Playlist,
    #[serde(default)]
    pub frame: Option<NeonFrame>,
    #[serde(default = "default_fit")]
    pub fit: String,
    #[serde(default)]
    pub calibration: bool,
    #[serde(default)]
    pub flip_horizontal: bool,
    #[serde(default)]
    pub flip_vertical: bool,
}
fn default_fit() -> String {
    "contain".into()
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NeonFrame {
    pub color: String,
    pub width: f32,
    pub glow: f32,
}

impl Surface {
    pub fn new(name: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            corners: [
                Point { x: 0.1, y: 0.1 },
                Point { x: 0.9, y: 0.1 },
                Point { x: 0.9, y: 0.9 },
                Point { x: 0.1, y: 0.9 },
            ],
            playlist: Playlist::default(),
            frame: None,
            fit: default_fit(),
            calibration: true,
            flip_horizontal: false,
            flip_vertical: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Scene {
    pub id: Uuid,
    pub name: String,
    /// Array order is the stacking order, back to front. No surface-count limit.
    pub surfaces: Vec<Surface>,
}

impl Scene {
    pub fn empty() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: "Новая сцена".into(),
            surfaces: vec![],
        }
    }
    pub fn duplicate_surface(&mut self, id: Uuid) -> Result<Uuid, String> {
        let index = self
            .surfaces
            .iter()
            .position(|s| s.id == id)
            .ok_or("Область не найдена")?;
        let mut copy = self.surfaces[index].clone();
        copy.id = Uuid::new_v4();
        copy.name = format!("{} — копия", copy.name);
        let new_id = copy.id;
        self.surfaces.insert(index + 1, copy);
        Ok(new_id)
    }
    pub fn validate(&self, assets: &[MediaAsset]) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("Укажите название сцены".into());
        }
        let assets: HashMap<_, _> = assets.iter().map(|a| (a.id, a.kind)).collect();
        let mut ids = HashSet::new();
        for surface in &self.surfaces {
            if surface.fit != "contain" && surface.fit != "cover" && surface.fit != "stretch" {
                return Err("Неверный режим пропорций".into());
            }
            if let Some(frame) = &surface.frame {
                if frame.color.len() != 7
                    || !frame.color.starts_with('#')
                    || !frame.color[1..].bytes().all(|c| c.is_ascii_hexdigit())
                    || !frame.width.is_finite()
                    || !(1.0..=24.0).contains(&frame.width)
                    || !frame.glow.is_finite()
                    || !(0.0..=80.0).contains(&frame.glow)
                {
                    return Err("Неверные параметры неоновой рамки".into());
                }
            }
            if !ids.insert(surface.id) {
                return Err("Идентификаторы областей должны быть уникальны".into());
            }
            if surface.name.trim().is_empty() {
                return Err("Укажите название области".into());
            }
            if surface.playlist.interval_seconds == 0 {
                return Err("Интервал должен быть больше нуля".into());
            }
            for point in &surface.corners {
                if !point.x.is_finite()
                    || !point.y.is_finite()
                    || !(0.0..=1.0).contains(&point.x)
                    || !(0.0..=1.0).contains(&point.y)
                {
                    return Err("Координаты должны находиться внутри кадра".into());
                }
            }
            // Reject crossed, reversed and degenerate quads before the projective transform.
            for i in 0..4 {
                let a = &surface.corners[i];
                let b = &surface.corners[(i + 1) % 4];
                let c = &surface.corners[(i + 2) % 4];
                let cross = (b.x - a.x) * (c.y - b.y) - (b.y - a.y) * (c.x - b.x);
                if cross <= 0.00001 {
                    return Err(
                        "Углы должны образовывать выпуклую область по часовой стрелке".into(),
                    );
                }
            }
            for item in &surface.playlist.items {
                match assets.get(item) {
                    Some(_) => {}
                    None => return Err("Файл плейлиста не найден".into()),
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MediaKind {
    Image,
    Video,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MediaAsset {
    pub id: Uuid,
    pub name: String,
    pub kind: MediaKind,
    pub mime: String,
    pub bytes: u64,
    #[serde(default)]
    pub attribution: Option<Attribution>,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    #[serde(default)]
    pub original_mime: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Attribution {
    pub title: String,
    pub artist: String,
    pub source_url: String,
    pub rights: String,
    pub provider_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlaybackState {
    pub scene: Scene,
    pub assets: Vec<MediaAsset>,
    pub revision: u64,
    pub paused: bool,
}
