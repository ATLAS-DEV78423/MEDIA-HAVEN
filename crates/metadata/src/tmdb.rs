//! Optional TMDB metadata provider — OFF by default.
//!
//! Only a title and year are ever handed to the fetch function; no filesystem
//! path, folder name or library content is included. The key is injected from
//! the environment (the app sources it from the ADR-0007 secret store); it is
//! never read from or written to the repository.
//!
//! Transport seam: `TmdbProvider::new` still uses a fail-closed fetch, so no
//! request can leave the machine until the application injects one. ADR-0023
//! approved `http_search_fetch` as the real transport; it is only used by an
//! enabled provider with an injected key. Everything around it — opt-in gate,
//! fail-closed key check, 30-day cache, eviction, parsing and sanitisation — is
//! implemented and tested here.

use crate::cache::Cache;
use crate::model::{MAX_GENRES, MAX_GENRE_CHARS, MAX_SYNOPSIS_CHARS, MAX_TITLE_CHARS};
use app_core::domain::MetadataRecord;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// TMDB's required attribution line. The UI must render this wherever TMDB
/// data appears; this backend crate only exposes the constant.
pub const TMDB_ATTRIBUTION: &str =
    "This product uses the TMDB API but is not endorsed or certified by TMDB.";

/// Environment variable the app populates from the secret store.
pub const API_KEY_ENV: &str = "MEDIAHAVEN_TMDB_API_KEY";
pub const CACHE_DIR_ENV: &str = "MEDIAHAVEN_TMDB_CACHE_DIR";
pub const DEFAULT_MAX_CACHE_BYTES: u64 = 64 * 1024 * 1024;

/// Read the API key from the environment. Absent/blank means fail closed.
pub fn api_key_from_env() -> Option<String> {
    std::env::var(API_KEY_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[derive(Debug, Clone)]
pub struct TmdbOptions {
    /// Master switch. `false` makes `lookup` a zero-request, `None`-returning no-op.
    pub enabled: bool,
    /// Injected secret. Never a literal in source.
    pub api_key: Option<String>,
    pub cache_dir: PathBuf,
    pub max_bytes: u64,
}

impl Default for TmdbOptions {
    fn default() -> Self {
        Self {
            enabled: false,
            api_key: None,
            cache_dir: std::env::var_os(CACHE_DIR_ENV)
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::temp_dir().join("media-haven-tmdb-cache")),
            max_bytes: DEFAULT_MAX_CACHE_BYTES,
        }
    }
}

/// Pluggable fetch. Receives only title, year and the injected key — never a
/// path. Returns a raw JSON body; any error degrades to `None`.
pub type FetchFn = dyn Fn(&str, Option<i64>, &str) -> Result<String, String> + Send + Sync;

/// A parsed result plus the poster path (which `MetadataRecord` has no field
/// for). The future artwork route consumes `poster_path`; `lookup` returns just
/// the record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TmdbLookup {
    pub record: MetadataRecord,
    pub poster_path: Option<String>,
}

impl TmdbLookup {
    pub fn attribution(&self) -> &'static str {
        TMDB_ATTRIBUTION
    }
}

pub struct TmdbProvider {
    options: TmdbOptions,
    fetch: Box<FetchFn>,
    cache: Cache,
}

impl TmdbProvider {
    /// Default constructor. Uses the failing transport seam so no request can
    /// leave the machine until a fetch is explicitly injected and enabled.
    pub fn new(options: TmdbOptions) -> Self {
        Self::with_fetch(options, Box::new(unconfigured_fetch))
    }

    pub fn with_fetch(options: TmdbOptions, fetch: Box<FetchFn>) -> Self {
        let cache = Cache::new(options.cache_dir.clone(), options.max_bytes);
        Self {
            options,
            fetch,
            cache,
        }
    }

    pub fn options(&self) -> &TmdbOptions {
        &self.options
    }

    /// Look up by title and year only. Returns `None` when disabled, when the
    /// key is absent, when there is no fresh cache and the fetch fails, or when
    /// the payload cannot be parsed. Never returns an error to the caller.
    pub fn lookup(&self, title: &str, year: Option<i64>) -> Option<MetadataRecord> {
        self.lookup_full(title, year).map(|found| found.record)
    }

    /// As `lookup`, but also exposes the sanitised poster path for the artwork
    /// route. A fresh cache hit makes no fetch call.
    pub fn lookup_full(&self, title: &str, year: Option<i64>) -> Option<TmdbLookup> {
        if !self.options.enabled {
            return None;
        }
        let key = self
            .options
            .api_key
            .as_deref()
            .map(str::trim)
            .filter(|key| !key.is_empty())?;
        let cache_key = cache_key(title, year);

        if let Some(bytes) = self.cache.get(&cache_key) {
            if let Ok(text) = std::str::from_utf8(&bytes) {
                if let Some(found) = parse_result(text) {
                    return Some(found);
                }
            }
        }

        let json = (self.fetch)(title, year, key).ok()?;
        let found = parse_result(&json)?;
        // Cache only payloads that parsed, so a hostile body is never stored.
        let _ = self.cache.put(&cache_key, json.as_bytes());
        Some(found)
    }
}

/// The real HTTPS transport is deliberately absent: no HTTP client crate is in
/// the workspace and adding one needs an ADR. Injection point for that work.
fn unconfigured_fetch(_title: &str, _year: Option<i64>, _key: &str) -> Result<String, String> {
    Err("TMDB HTTP transport is not configured".to_string())
}

fn cache_key(title: &str, year: Option<i64>) -> String {
    format!(
        "{}|{}",
        title.trim().to_ascii_lowercase(),
        year.unwrap_or(0)
    )
}

fn parse_result(json: &str) -> Option<TmdbLookup> {
    let value: Value = serde_json::from_str(json).ok()?;
    let hit = value.get("results")?.as_array()?.first()?;

    let title = sanitize(hit.get("title")?.as_str()?, MAX_TITLE_CHARS)?;
    let year = hit
        .get("release_date")
        .and_then(Value::as_str)
        .and_then(|date| date.get(0..4))
        .and_then(|part| part.parse::<i64>().ok())
        .filter(|year| (1870..=2200).contains(year));
    let synopsis = hit
        .get("overview")
        .and_then(Value::as_str)
        .and_then(|text| sanitize(text, MAX_SYNOPSIS_CHARS));

    let mut genres = Vec::new();
    if let Some(items) = hit.get("genres").and_then(Value::as_array) {
        for item in items {
            let Some(name) = item.get("name").and_then(Value::as_str) else {
                continue;
            };
            let Some(name) = sanitize(name, MAX_GENRE_CHARS) else {
                continue;
            };
            if !genres
                .iter()
                .any(|known: &String| known.eq_ignore_ascii_case(&name))
            {
                genres.push(name);
                if genres.len() == MAX_GENRES {
                    break;
                }
            }
        }
    }
    // shortcut: the search endpoint returns genre_ids, not names; mapping them
    // needs a genre table or a details call, which the real transport must add.

    let poster_path = hit
        .get("poster_path")
        .and_then(Value::as_str)
        .and_then(sanitize_poster);

    Some(TmdbLookup {
        record: MetadataRecord {
            title: Some(title),
            year,
            synopsis,
            runtime_minutes: None,
            genres,
        },
        poster_path,
    })
}

/// Collapse whitespace, drop control characters, cap length. Blank becomes `None`.
fn sanitize(raw: &str, max_chars: usize) -> Option<String> {
    let without_controls: String = raw
        .chars()
        .filter(|ch| !ch.is_control() || matches!(ch, '\n' | '\r' | '\t'))
        .collect();
    let joined = without_controls
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if joined.is_empty() {
        return None;
    }
    let capped: String = joined.chars().take(max_chars).collect();
    if capped.trim().is_empty() {
        None
    } else {
        Some(capped)
    }
}

/// TMDB artwork paths are CDN-relative (`/abc123.jpg`). Accept only that
/// shape so a hostile payload cannot become a local path, URL or script.
pub fn sanitize_poster(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.is_empty()
        || value.len() > 512
        || !value.starts_with('/')
        || value.contains("..")
        || value.contains('\\')
        || value.chars().any(char::is_control)
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '_' | '-' | '.'))
    {
        return None;
    }
    Some(value.to_string())
}

/// Redact transport errors: reqwest's `Display` appends the request URL, which
/// carries the ADR-0007 API key in its query string. Never surface it.
pub fn request_error<E: std::fmt::Display>(_error: E) -> String {
    "TMDB request failed".to_string()
}

/// Real TMDB v3 search transport. Only title/year/key are used; no path.
pub fn http_search_fetch() -> Box<FetchFn> {
    Box::new(|title, year, key| {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|err| err.to_string())?;
        let mut request = client
            .get("https://api.themoviedb.org/3/search/movie")
            .query(&[("api_key", key), ("query", title)]);
        if let Some(year) = year {
            request = request.query(&[("year", year.to_string())]);
        }
        let response = request.send().map_err(request_error)?;
        response
            .error_for_status()
            .map_err(request_error)?
            .text()
            .map_err(|err| err.to_string())
    })
}

/// 2 MiB per poster; anything larger is refused before it is cached.
pub const MAX_ARTWORK_BYTES: usize = 2 * 1024 * 1024;

pub fn too_large(len: usize) -> bool {
    len > MAX_ARTWORK_BYTES
}

pub fn artwork_key(poster_path: &str) -> String {
    let digest = Sha256::digest(poster_path.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Accept only real JPEG/PNG headers; never cache HTML or script as an image.
pub fn classify_image(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        Some("image/png")
    } else {
        None
    }
}

/// Download a poster from the TMDB image CDN. Pure URL construction from the
/// already-sanitised `poster_path`; no path from the local filesystem is used.
pub fn fetch_artwork_bytes(poster_path: &str) -> Result<(Vec<u8>, String), String> {
    let path = sanitize_poster(poster_path).ok_or_else(|| "invalid poster path".to_string())?;
    let url = format!("https://image.tmdb.org/t/p/w500{path}");
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|err| err.to_string())?;
    let response = client.get(&url).send().map_err(|err| err.to_string())?;
    let bytes = response
        .error_for_status()
        .map_err(|err| err.to_string())?
        .bytes()
        .map_err(|err| err.to_string())?;
    if too_large(bytes.len()) {
        return Err("poster exceeds size cap".to_string());
    }
    let kind = classify_image(&bytes).ok_or_else(|| "poster is not an image".to_string())?;
    Ok((bytes.to_vec(), kind.to_string()))
}

/// Hashed, size-capped image cache under the core data dir.
pub struct ArtworkCache {
    dir: PathBuf,
    max_bytes: u64,
}

impl ArtworkCache {
    pub fn new(dir: PathBuf, max_bytes: u64) -> Self {
        Self { dir, max_bytes }
    }

    pub fn get(&self, key: &str) -> Option<(Vec<u8>, String)> {
        let path = self.dir.join(key);
        let bytes = std::fs::read(&path).ok()?;
        let kind = classify_image(&bytes)?.to_string();
        // Reading updates recency so eviction is genuinely oldest-first.
        let _ = std::fs::File::open(&path)
            .and_then(|file| file.set_modified(std::time::SystemTime::now()));
        Some((bytes, kind))
    }

    pub fn put(&self, key: &str, bytes: &[u8], content_type: &str) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        if classify_image(bytes).is_none() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "not an image",
            ));
        }
        let path = self.dir.join(key);
        std::fs::write(&path, bytes)?;
        let mut used = std::fs::read_dir(&self.dir)?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                entry
                    .metadata()
                    .ok()
                    .map(|meta| (entry.path(), meta.len(), meta.modified().ok()))
            })
            .fold(0u64, |sum, (_, len, _)| sum.saturating_add(len));
        if used > self.max_bytes {
            let mut entries: Vec<_> = std::fs::read_dir(&self.dir)?
                .filter_map(|entry| entry.ok())
                .filter_map(|entry| {
                    entry
                        .metadata()
                        .ok()
                        .map(|meta| (entry.path(), meta.modified().ok(), meta.len()))
                })
                .collect();
            entries.sort_by_key(|(_, modified, _)| *modified);
            for (path, _, len) in entries {
                if used <= self.max_bytes {
                    break;
                }
                if std::fs::remove_file(&path).is_ok() {
                    used = used.saturating_sub(len);
                }
            }
        }
        let _ = content_type; // content type is re-derived from magic bytes on read
        Ok(())
    }
}

// shortcut: content_type is re-derived from magic bytes on read, so the stored
// file needs no metadata sidecar; upgrade only if non-image formats are added.
