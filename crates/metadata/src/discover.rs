//! Display-only TMDB discovery. Only the operator's key, a kind and a page
//! number are sent; no library path or content is included.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration;

use crate::cache::Cache;
use crate::model::{MAX_SYNOPSIS_CHARS, MAX_TITLE_CHARS};
use crate::tmdb::{api_key_from_env, sanitize_poster, DEFAULT_MAX_CACHE_BYTES};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CatalogKind {
    Movie,
    Tv,
    Anime,
}

// shortcut: no Eq derive — vote_average is f64, which is only PartialEq.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    pub id: String,
    pub kind: CatalogKind,
    pub title: String,
    pub year: Option<i64>,
    pub synopsis: Option<String>,
    pub poster_path: Option<String>,
    pub vote_average: Option<f64>,
}

pub type DiscoverFetchFn = dyn Fn(CatalogKind, u32, &str) -> Result<String, String> + Send + Sync;

/// Relative endpoint template. `{page}` is substituted by `http_discover_fetch`;
/// the page argument is kept in the signature for call-site symmetry.
pub fn discover_url(kind: CatalogKind, _page: u32) -> &'static str {
    match kind {
        CatalogKind::Movie => "/3/discover/movie?sort_by=popularity.desc&page={page}",
        CatalogKind::Tv => "/3/discover/tv?sort_by=popularity.desc&page={page}",
        CatalogKind::Anime => "/3/discover/tv?with_genres=16&with_original_language=ja&sort_by=popularity.desc&page={page}",
    }
}

pub struct DiscoverOptions {
    pub enabled: bool,
    pub api_key: Option<String>,
    pub cache_dir: PathBuf,
}

pub struct DiscoverProvider {
    options: DiscoverOptions,
    fetch: Box<DiscoverFetchFn>,
    cache: Cache,
}

impl DiscoverProvider {
    pub fn new() -> Self {
        let api_key = api_key_from_env();
        Self::with_options(DiscoverOptions {
            enabled: api_key.is_some(),
            api_key,
            cache_dir: std::env::temp_dir().join("media-haven-discover-cache"),
        })
    }

    pub fn with_options(options: DiscoverOptions) -> Self {
        Self::with_fetch(options, Box::new(http_discover_fetch))
    }

    pub fn with_fetch(options: DiscoverOptions, fetch: Box<DiscoverFetchFn>) -> Self {
        let cache = Cache::new(options.cache_dir.clone(), DEFAULT_MAX_CACHE_BYTES);
        Self {
            options,
            fetch,
            cache,
        }
    }

    pub fn enabled(&self) -> bool {
        self.options.enabled
    }

    pub fn discover(&self, kind: CatalogKind, page: u32) -> Vec<CatalogEntry> {
        if !self.options.enabled {
            return Vec::new();
        }
        let Some(key) = self
            .options
            .api_key
            .as_deref()
            .filter(|key| !key.trim().is_empty())
        else {
            return Vec::new();
        };
        let cache_key = format!("discover|{}|{}", kind as u8, page);
        let json = self
            .cache
            .get(&cache_key)
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .or_else(|| (self.fetch)(kind, page, key).ok())
            .unwrap_or_default();
        let entries = parse_page(kind, &json);
        if !entries.is_empty() && self.cache.get(&cache_key).is_none() {
            let _ = self.cache.put(&cache_key, json.as_bytes());
        }
        entries
    }
}

impl Default for DiscoverProvider {
    fn default() -> Self {
        Self::new()
    }
}

pub fn http_discover_fetch(kind: CatalogKind, page: u32, key: &str) -> Result<String, String> {
    let template = discover_url(kind, page).replace("{page}", &page.to_string());
    let url = format!("https://api.themoviedb.org{template}&api_key={key}");
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|_| "client unavailable".to_string())?;
    client
        .get(url)
        .send()
        .map_err(|_| "discover request failed".to_string())?
        .error_for_status()
        .map_err(|_| "discover request failed".to_string())?
        .text()
        .map_err(|_| "discover response unreadable".to_string())
}

fn clean(raw: &str, max: usize) -> Option<String> {
    let text: String = raw.chars().filter(|c| !c.is_control()).collect();
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        None
    } else {
        Some(text.chars().take(max).collect())
    }
}

fn parse_page(kind: CatalogKind, json: &str) -> Vec<CatalogEntry> {
    let value: Value = match serde_json::from_str(json) {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };
    let results = match value.get("results").and_then(Value::as_array) {
        Some(results) => results,
        None => return Vec::new(),
    };
    results
        .iter()
        .filter_map(|hit| {
            let title_field = if matches!(kind, CatalogKind::Movie) {
                "title"
            } else {
                "name"
            };
            let title = clean(hit.get(title_field)?.as_str()?, MAX_TITLE_CHARS)?;
            let year = hit
                .get("release_date")
                .or_else(|| hit.get("first_air_date"))
                .and_then(Value::as_str)
                .and_then(|date| date.get(0..4))
                .and_then(|part| part.parse::<i64>().ok())
                .filter(|year| (1870..=2200).contains(year));
            Some(CatalogEntry {
                id: hit.get("id")?.as_i64()?.to_string(),
                kind,
                title,
                year,
                synopsis: hit
                    .get("overview")
                    .and_then(Value::as_str)
                    .and_then(|text| clean(text, MAX_SYNOPSIS_CHARS)),
                // shortcut: poster_path is re-checked with the TMDB sanitiser, so
                // a hostile payload cannot become a path or URL; upgrade only if
                // another artwork source is added.
                poster_path: hit
                    .get("poster_path")
                    .and_then(Value::as_str)
                    .and_then(sanitize_poster),
                vote_average: hit
                    .get("vote_average")
                    .and_then(Value::as_f64)
                    .filter(|vote| (0.0..=10.0).contains(vote)),
            })
        })
        .take(20)
        .collect()
}
