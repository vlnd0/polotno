use crate::{
    model::{Attribution, MediaAsset},
    server::{bad, commit_upload_attributed, ApiError, Shared},
};
use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::{sync::OnceLock, time::Duration};

const FIELDS: &str = "id,title,artist_display,date_display,image_id,is_public_domain";
const PICKS: &str = "27992,28560,20684,11723,81558,16571,80607,14620";
static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
fn client() -> &'static reqwest::Client {
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(25))
            .user_agent("Polotno/0.1 personal projector gallery")
            .redirect(reqwest::redirect::Policy::limited(3))
            .build()
            .expect("HTTP client")
    })
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CatalogItem {
    pub id: u64,
    pub provider: String,
    pub title: String,
    pub artist: String,
    pub date: String,
    pub preview_url: String,
    pub source_url: String,
    pub rights: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub video_url: Option<String>,
}
#[derive(Deserialize)]
struct Artwork {
    id: u64,
    title: String,
    #[serde(default)]
    artist_display: String,
    #[serde(default)]
    date_display: String,
    image_id: Option<String>,
    #[serde(default)]
    is_public_domain: bool,
}
#[derive(Deserialize)]
struct ListResponse {
    data: Vec<Artwork>,
    pagination: Option<Pagination>,
}
#[derive(Deserialize)]
struct Pagination {
    total_pages: u32,
}
#[derive(Deserialize)]
struct DetailResponse {
    data: Artwork,
}
#[derive(Deserialize)]
pub struct BrowseQuery {
    pub q: Option<String>,
    pub page: Option<u32>,
    pub provider: Option<String>,
    pub kind: Option<String>,
}
#[derive(Serialize)]
pub struct CatalogPage {
    pub items: Vec<CatalogItem>,
    pub page: u32,
    pub has_more: bool,
}
fn item(art: &Artwork) -> Option<CatalogItem> {
    if !art.is_public_domain {
        return None;
    }
    let image_id = art.image_id.as_ref()?;
    // Validate the provider's identifier before using it as a URL component.
    if uuid::Uuid::parse_str(image_id).is_err() {
        return None;
    }
    Some(CatalogItem {
        id: art.id,
        provider: "aic".into(),
        title: art.title.clone(),
        artist: art.artist_display.clone(),
        date: art.date_display.clone(),
        preview_url: format!("https://www.artic.edu/iiif/2/{image_id}/full/400,/0/default.jpg"),
        source_url: format!("https://www.artic.edu/artworks/{}", art.id),
        rights: "CC0 · общественное достояние".into(),
        kind: "image".into(),
        category: "Музей".into(),
        video_url: None,
    })
}
pub async fn browse(Query(query): Query<BrowseQuery>) -> Result<Json<CatalogPage>, ApiError> {
    if query.provider.as_deref() == Some("polotno") {
        return Ok(Json(
            crate::wallpapers::browse(query.kind.as_deref()).map_err(bad)?,
        ));
    }
    if query.provider.as_deref().unwrap_or("met") == "met" {
        return browse_met(query).await;
    }
    if query.provider.as_deref() != Some("aic") {
        return Err(bad("Неизвестная коллекция"));
    }
    let page = query.page.unwrap_or(1).max(1);
    let search = query.q.as_deref().unwrap_or("").trim();
    let request = if search.is_empty() {
        client()
            .get("https://api.artic.edu/api/v1/artworks")
            .query(&[("ids", PICKS), ("fields", FIELDS)])
    } else {
        client()
            .get("https://api.artic.edu/api/v1/artworks/search")
            .query(&[
                ("q", search),
                ("fields", FIELDS),
                ("query[term][is_public_domain]", "true"),
                ("limit", "24"),
                ("page", &page.to_string()),
            ])
    };
    let response: ListResponse = request
        .send()
        .await
        .map_err(|_| bad("Музей недоступен. Попробуйте ещё раз"))?
        .error_for_status()
        .map_err(|_| bad("Музей временно недоступен"))?
        .json()
        .await
        .map_err(|_| bad("Не удалось прочитать каталог"))?;
    Ok(Json(CatalogPage {
        items: response.data.iter().filter_map(item).collect(),
        page,
        has_more: !search.is_empty() && response.pagination.is_some_and(|p| page < p.total_pages),
    }))
}
#[derive(Deserialize)]
pub struct ImportRequest {
    pub id: u64,
    pub provider: Option<String>,
}
pub async fn import(
    State(state): State<Shared>,
    Json(request): Json<ImportRequest>,
) -> Result<Json<MediaAsset>, ApiError> {
    if request.provider.as_deref() == Some("polotno") {
        return crate::wallpapers::import(state, request.id).await;
    }
    let provider = request.provider.as_deref().unwrap_or("met");
    if provider != "met" && provider != "aic" {
        return Err(bad("Неизвестная коллекция"));
    }
    let provider_id = format!("{provider}:{}", request.id);
    if let Some(asset) = state
        .lock()
        .unwrap()
        .playback
        .assets
        .iter()
        .find(|a| {
            a.attribution
                .as_ref()
                .is_some_and(|info| info.provider_id == provider_id)
        })
        .cloned()
    {
        return Ok(Json(asset));
    }
    // Re-check rights with the official provider at import time; clients cannot supply URLs or rights.
    let (info, url) = if provider == "met" {
        let art = met_detail(request.id).await?;
        let info =
            met_item(&art).ok_or_else(|| bad("У этой работы нет доступного изображения CC0"))?;
        let url = reqwest::Url::parse(&art.primary_image).map_err(bad)?;
        if url.scheme() != "https" || url.host_str() != Some("images.metmuseum.org") {
            return Err(bad("Неподдерживаемый адрес изображения музея"));
        }
        (info, art.primary_image)
    } else {
        let response: DetailResponse = client()
            .get(format!(
                "https://api.artic.edu/api/v1/artworks/{}",
                request.id
            ))
            .query(&[("fields", FIELDS)])
            .send()
            .await
            .map_err(|_| bad("Не удалось связаться с музеем"))?
            .error_for_status()
            .map_err(|_| bad("Картина недоступна"))?
            .json()
            .await
            .map_err(|_| bad("Не удалось прочитать данные картины"))?;
        let info = item(&response.data)
            .ok_or_else(|| bad("У этой работы нет доступного изображения CC0"))?;
        let url = format!(
            "https://www.artic.edu/iiif/2/{}/full/843,/0/default.jpg",
            response.data.image_id.unwrap()
        );
        (info, url)
    };
    let mut download = client()
        .get(url)
        .send()
        .await
        .map_err(|_| bad("Не удалось скачать картину"))?
        .error_for_status()
        .map_err(|_| bad("Изображение недоступно"))?;
    const MAX_IMAGE: usize = 16 * 1024 * 1024;
    if download
        .content_length()
        .is_some_and(|length| length > MAX_IMAGE as u64)
    {
        return Err(bad("Изображение слишком большое"));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = download
        .chunk()
        .await
        .map_err(|_| bad("Загрузка прервалась. Попробуйте ещё раз"))?
    {
        if bytes.len() + chunk.len() > MAX_IMAGE {
            return Err(bad("Изображение слишком большое"));
        }
        bytes.extend_from_slice(&chunk);
    }
    let attribution = Attribution {
        title: info.title.clone(),
        artist: info.artist,
        source_url: info.source_url,
        rights: info.rights,
        provider_id,
    };
    let mut state = state.lock().unwrap();
    // A second simultaneous selection reuses the first committed download.
    if let Some(asset) = state
        .playback
        .assets
        .iter()
        .find(|a| {
            a.attribution
                .as_ref()
                .is_some_and(|info| info.provider_id == attribution.provider_id)
        })
        .cloned()
    {
        return Ok(Json(asset));
    }
    Ok(Json(
        commit_upload_attributed(&mut state, &bytes, &info.title, Some(attribution))
            .map_err(bad)?,
    ))
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MetArtwork {
    #[serde(rename = "objectID")]
    object_id: u64,
    title: String,
    #[serde(default)]
    artist_display_name: String,
    #[serde(default)]
    object_date: String,
    #[serde(default)]
    is_public_domain: bool,
    #[serde(default)]
    primary_image: String,
    #[serde(default)]
    primary_image_small: String,
}
fn met_item(art: &MetArtwork) -> Option<CatalogItem> {
    if !art.is_public_domain || art.primary_image_small.is_empty() {
        return None;
    }
    Some(CatalogItem {
        id: art.object_id,
        provider: "met".into(),
        title: art.title.clone(),
        artist: art.artist_display_name.clone(),
        date: art.object_date.clone(),
        preview_url: art.primary_image_small.clone(),
        source_url: format!(
            "https://www.metmuseum.org/art/collection/search/{}",
            art.object_id
        ),
        rights: "CC0 · общественное достояние".into(),
        kind: "image".into(),
        category: "Музей".into(),
        video_url: None,
    })
}
async fn met_detail(id: u64) -> Result<MetArtwork, ApiError> {
    client()
        .get(format!(
            "https://collectionapi.metmuseum.org/public/collection/v1/objects/{id}"
        ))
        .send()
        .await
        .map_err(|_| bad("Музей недоступен. Попробуйте ещё раз"))?
        .error_for_status()
        .map_err(|_| bad("Картина недоступна"))?
        .json()
        .await
        .map_err(|_| bad("Не удалось прочитать данные картины"))
}
#[derive(Deserialize)]
struct MetSearch {
    total: u64,
    #[serde(rename = "objectIDs")]
    object_ids: Option<Vec<u64>>,
}
async fn browse_met(query: BrowseQuery) -> Result<Json<CatalogPage>, ApiError> {
    let search = query.q.as_deref().unwrap_or("").trim();
    let page = query.page.unwrap_or(1).max(1);
    if search.is_empty() {
        // Published Open Access metadata gives immediate choices; rights are re-checked on import.
        let picks: Vec<MetArtwork> =
            serde_json::from_str(include_str!("../catalog/met-picks.json")).map_err(bad)?;
        return Ok(Json(CatalogPage {
            items: picks.iter().filter_map(met_item).collect(),
            page: 1,
            has_more: false,
        }));
    }
    let translated = match search.to_lowercase().as_str() {
        "моне" => "Monet",
        "ван гог" => "Van Gogh",
        "вермеер" => "Vermeer",
        "дега" => "Degas",
        "сезанн" => "Cezanne",
        "тернер" => "Turner",
        _ => search,
    };
    let offset = u64::from(page - 1) * 24;
    // The old v1 search was retired on 2026-10-01. Always use paginated v1.1.
    let result: MetSearch = client()
        .get("https://collectionapi.metmuseum.org/public/collection/v1.1/search")
        .query(&[
            ("q", translated),
            ("hasImages", "true"),
            ("limit", "24"),
            ("offset", &offset.to_string()),
        ])
        .send()
        .await
        .map_err(|_| bad("Поиск музея недоступен"))?
        .error_for_status()
        .map_err(|_| bad("Не удалось выполнить поиск"))?
        .json()
        .await
        .map_err(|_| bad("Не удалось прочитать результаты"))?;
    let ids = result.object_ids.unwrap_or_default();
    let mut jobs = tokio::task::JoinSet::new();
    // Bounded batches respect the provider's 80 requests/second ceiling.
    let mut items = Vec::new();
    for batch in ids.chunks(4) {
        for &id in batch {
            jobs.spawn(async move { met_detail(id).await.ok().and_then(|art| met_item(&art)) });
        }
        while let Some(result) = jobs.join_next().await {
            if let Ok(Some(item)) = result {
                items.push(item);
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    items.sort_by_key(|item| {
        ids.iter()
            .position(|id| *id == item.id)
            .unwrap_or(usize::MAX)
    });
    Ok(Json(CatalogPage {
        items,
        page,
        has_more: offset + 24 < result.total,
    }))
}
