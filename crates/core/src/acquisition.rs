//! Host-internal acquisition routes (ADR-0018).
//!
//! The qBittorrent WebUI is reached only from `core`, which is the sole
//! container on the `acquire` network. These routes carry the Windows host's
//! `download_*` commands to that client. They are deliberately NOT renderer
//! commands: they live in this module under `/host/` instead of `routes.rs`,
//! so the 24-command POST parity surface (renderer commands only) is unchanged.
//! They still pass the signed auth middleware exactly like `/readyz`.

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use app_core::domain::{
    AppError, AppErrorCode, BrowseQuery, BrowseSort, CleanupEntry, DownloadState,
    MediaAvailability, MediaDetail,
};
use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use media_acquisition::{
    inspect_with_runner, missing_episodes, next_two_in_season, reconcile, AcquireError,
    CleanupCandidate, CleanupDecision, CleanupError, CleanupState, CleanupWorker,
    DockerProbeRunner, EpisodeRef, InspectionState, ManagedFile, PrefetchScheduler, ProbeRunner,
    QbClient, TorrentApi, DEFAULT_BUFFER_LIMIT_BYTES, REPLAY_GRACE, SAVE_PATH,
};
use media_persistence::sqlite::{
    connect_path, CleanupRepository, ManagedFileRow, SqliteLibraryRepository,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::routes::{ApiError, ApiJson};
use crate::state::AppState;

/// Distinct prefix: never mixed into the renderer's command surface.
pub const HOST_PREFIX: &str = "/host";
const ADD_PATH: &str = "/host/acquisition_add";
const STATUS_PATH: &str = "/host/acquisition_status";
const REMOVE_PATH: &str = "/host/acquisition_remove";
const BUFFER_STATUS_PATH: &str = "/host/acquisition_buffer_status";
const CLEANUP_CONFIRM_PATH: &str = "/host/acquisition_cleanup_confirm";
const CLEANUP_PASS_PATH: &str = "/host/acquisition_cleanup_pass";
const INSPECT_PATH: &str = "/host/acquisition_inspect";
const SOURCE_SET_PATH: &str = "/host/acquisition_source_set";
const IMDB_LOOKUP_PATH: &str = "/host/imdb_lookup";
const SEED_PATH: &str = "/host/indexers_seed";
const SEARCH_PATH: &str = "/host/source_search";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AddArgs {
    source: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RemoveArgs {
    id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SourceSetArgs {
    media_id: String,
    source: String,
}

/// Host-internal inspection trigger for a completed download. `containerPath`
/// is a host-internal container path (never a renderer DTO field); it is
/// constrained to `/downloads/...` before any command is built.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InspectArgs {
    id: String,
    container_path: String,
}

/// Host-internal completion signal (NOT a renderer DTO). Carries the B5 inputs
/// so the backend can confirm completion with the shared policy, never trusting
/// a renderer's claim of "watched".
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CompletionSignalArgs {
    media_id: String,
    watched_seconds: f64,
    duration_seconds: f64,
    position_seconds: f64,
    ended_normally: bool,
    user_stopped: bool,
}

/// Routes merged into the protected router in `server.rs`.
pub fn host_router() -> Router<AppState> {
    Router::new()
        .route(ADD_PATH, post(add))
        .route(STATUS_PATH, post(status))
        .route(REMOVE_PATH, post(remove))
        .route(BUFFER_STATUS_PATH, post(buffer_status))
        .route(CLEANUP_CONFIRM_PATH, post(confirm_completion))
        .route(CLEANUP_PASS_PATH, post(cleanup_pass))
        .route(INSPECT_PATH, post(inspect))
        .route(SOURCE_SET_PATH, post(source_set))
        .route(IMDB_LOOKUP_PATH, post(imdb_lookup))
        .route(SEED_PATH, post(indexers_seed))
        .route(SEARCH_PATH, post(source_search))
}

/// Attach an operator-supplied source to a library item (R-01 / ADR-0021).
/// Host-internal: the renderer reaches it through a named command only. The
/// source is validated again here (trust boundary) and never returned to a DTO.
async fn source_set(
    State(state): State<AppState>,
    ApiJson(args): ApiJson<SourceSetArgs>,
) -> Result<Json<Value>, ApiError> {
    if args.media_id.trim().is_empty() || args.source.trim().is_empty() {
        return Err(cleanup_invalid("mediaId and source are required."));
    }
    if args.source.len() > 4096 {
        return Err(cleanup_invalid("source is too long."));
    }
    state
        .service
        .set_item_source(&args.media_id, &args.source)
        .await
        .map_err(ApiError)?;
    Ok(Json(Value::Null))
}

/// Display-only IMDb/OMDb fields returned to the renderer, keyed by media id.
/// No dataset path, API key or token is ever part of this shape.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImdbDisplay {
    #[serde(default)]
    pub imdb_rating: Option<f64>,
    #[serde(default)]
    pub vote_count: Option<i64>,
    #[serde(default)]
    pub age_rating: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImdbLookupArgs {
    media_id: String,
}

/// Case/punctuation-insensitive key for matching library titles to the
/// operator's offline `title.basics.tsv` listings.
fn normalize_title(raw: &str) -> String {
    raw.chars()
        .map(|ch| {
            if ch.is_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Resolve a media id to its title/year from the library, then match those
/// against the opt-in offline IMDb datasets. Every failure — disabled opt-in,
/// unknown id, missing files, no match — degrades to an all-`None` display.
async fn lookup_display(state: &AppState, media_id: &str) -> ImdbDisplay {
    if media_id.trim().is_empty() || !state.imdb.options().enabled {
        return ImdbDisplay::default();
    }
    let (title, year) = match state.service.detail(media_id).await {
        Ok(MediaDetail::Movie(detail)) => (detail.title, detail.year),
        Ok(MediaDetail::Series(detail)) => (detail.title, detail.year),
        Err(_) => return ImdbDisplay::default(),
    };
    let Some(records) = state.imdb.records() else {
        return ImdbDisplay::default();
    };
    let needle = normalize_title(&title);
    let matched = records.into_iter().find(|record| {
        normalize_title(&record.title) == needle
            && year
                .map(|wanted| Some(wanted) == record.year)
                .unwrap_or(true)
    });
    matched
        .map(|record| ImdbDisplay {
            imdb_rating: record.rating,
            vote_count: record.votes,
            age_rating: record.age_rating,
        })
        .unwrap_or_default()
}

async fn imdb_lookup(
    State(state): State<AppState>,
    ApiJson(args): ApiJson<ImdbLookupArgs>,
) -> Result<Json<Value>, ApiError> {
    encode(lookup_display(&state, &args.media_id).await)
}

// ---------------------------------------------------------------------------
// Seeded sources (Task 2). One host-internal route installs the bundled
// Prowlarr-native definitions in the operator's own Prowlarr instance. The
// address and key come only from this process's environment; neither enters a
// DTO, a log line or an error string. No new renderer command is added.
// ---------------------------------------------------------------------------

const PROWLARR_URL_ENV: &str = "MEDIAHAVEN_PROWLARR_URL";
const PROWLARR_KEY_ENV: &str = "MEDIAHAVEN_PROWLARR_API_KEY";

/// Only these have a native `definitionName` in Prowlarr. The two custom
/// Miauflix definitions are reported as skipped and are never posted.
const SEED_IDS: [&str; 3] = ["yts", "nyaasi", "thepiratebay"];
const CUSTOM_IDS: [&str; 2] = ["eztv-api", "animetosho"];

const SKIP_CUSTOM: &str = "not in Prowlarr";
const SKIP_UNAVAILABLE: &str = "not available in Prowlarr";
const SKIP_PRESENT: &str = "already present";
const SKIP_FAILED: &str = "Prowlarr rejected the definition";
const TRANSPORT_ERROR: &str = "Prowlarr could not be reached.";
const RESPONSE_ERROR: &str = "Prowlarr returned an unexpected response.";

/// Counts and fixed reasons only. Never carries a URL, key, path or raw
/// Prowlarr response text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SeedResult {
    pub added: Vec<String>,
    pub skipped: Vec<String>,
}

/// One Prowlarr call. Credentials are supplied per request by `seed_indexers`;
/// the seam stores neither, so a test fake cannot leak them into a DTO.
#[derive(Debug, Clone)]
struct ProwlarrRequest {
    base_url: String,
    api_key: String,
    path: String,
    /// Extra query string appended after `apikey` (begins with `&`, or empty).
    /// Seeded source calls leave it empty; Torznab search supplies `t` and `q`.
    query: String,
    body: Option<Value>,
}

/// Mirrors the metadata crate's `FetchFn` seam: a boxed closure returning raw
/// response text or a fixed error. Tests inject a fake; no live HTTP.
type ProwlarrFetchFn = dyn Fn(&ProwlarrRequest) -> Result<String, String> + Send + Sync;

struct ProwlarrHttp {
    fetch: Box<ProwlarrFetchFn>,
}

impl ProwlarrHttp {
    fn live() -> Self {
        Self {
            fetch: Box::new(|request| {
                let client = reqwest::blocking::Client::builder()
                    .timeout(Duration::from_secs(2))
                    .build()
                    .map_err(|_| TRANSPORT_ERROR.to_string())?;
                let url = format!(
                    "{}{}?apikey={}{}",
                    request.base_url.trim_end_matches('/'),
                    request.path,
                    request.api_key,
                    request.query
                );
                let outgoing = match &request.body {
                    Some(body) => client
                        .post(&url)
                        .header(reqwest::header::CONTENT_TYPE, "application/json")
                        .body(serde_json::to_vec(body).map_err(|_| RESPONSE_ERROR.to_string())?),
                    None => client.get(&url),
                };
                let response = outgoing.send().map_err(|_| TRANSPORT_ERROR.to_string())?;
                if !response.status().is_success() {
                    return Err(RESPONSE_ERROR.to_string());
                }
                response.text().map_err(|_| RESPONSE_ERROR.to_string())
            }),
        }
    }

    fn call(
        &self,
        base_url: &str,
        api_key: &str,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value, String> {
        let text = (self.fetch)(&ProwlarrRequest {
            base_url: base_url.to_string(),
            api_key: api_key.to_string(),
            path: path.to_string(),
            query: String::new(),
            body,
        })?;
        serde_json::from_str(&text).map_err(|_| RESPONSE_ERROR.to_string())
    }
}

/// Read the operator inputs from this process only and seed through the live
/// transport. Called from a `spawn_blocking` task by the host route.
pub fn seed_indexers() -> SeedResult {
    let url = std::env::var(PROWLARR_URL_ENV).unwrap_or_default();
    let key = std::env::var(PROWLARR_KEY_ENV).unwrap_or_default();
    seed_indexers_with(&ProwlarrHttp::live(), &url, &key)
}

/// The whole seeding flow behind an injectable HTTP seam. Every failure
/// degrades to a fixed reason; no transport text or credential is returned.
fn seed_indexers_with(http: &ProwlarrHttp, base_url: &str, api_key: &str) -> SeedResult {
    let mut result = SeedResult {
        added: Vec::new(),
        skipped: Vec::new(),
    };
    if base_url.trim().is_empty() || api_key.trim().is_empty() {
        return result;
    }
    for id in CUSTOM_IDS {
        result.skipped.push(format!("{id}: {SKIP_CUSTOM}"));
    }

    let Ok(schema) = http.call(base_url, api_key, "/api/v1/indexer/schema", None) else {
        return result;
    };
    let Some(schema) = schema.as_array() else {
        return result;
    };
    let Ok(indexers) = http.call(base_url, api_key, "/api/v1/indexer", None) else {
        return result;
    };
    let existing: HashSet<String> = indexers
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|row| row["name"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    for id in SEED_IDS {
        let Some(entry) = schema
            .iter()
            .find(|item| item["definitionName"].as_str() == Some(id))
        else {
            result.skipped.push(format!("{id}: {SKIP_UNAVAILABLE}"));
            continue;
        };
        let Some(name) = entry["name"].as_str() else {
            result.skipped.push(format!("{id}: {SKIP_UNAVAILABLE}"));
            continue;
        };
        if existing.contains(name) {
            result.skipped.push(format!("{id}: {SKIP_PRESENT}"));
            continue;
        }
        // Prowlarr returns 400 when `fields` is missing or empty: the schema's
        // own field array must be forwarded verbatim.
        let Some(fields) = entry["fields"].as_array() else {
            result.skipped.push(format!("{id}: {SKIP_UNAVAILABLE}"));
            continue;
        };
        let Some(base) = entry["indexerUrls"]
            .as_array()
            .and_then(|urls| urls.first())
            .and_then(Value::as_str)
        else {
            result.skipped.push(format!("{id}: {SKIP_UNAVAILABLE}"));
            continue;
        };
        let body = serde_json::json!({
            "name": name,
            "implementation": entry["implementation"],
            "configContract": entry["configContract"],
            "protocol": entry["protocol"],
            "appProfileId": 1,
            "enable": true,
            "priority": 1,
            "tags": [],
            "fields": fields,
            "settings": { "definitionFile": id, "baseUrl": base },
        });
        match http.call(base_url, api_key, "/api/v1/indexer", Some(body)) {
            Ok(_) => result.added.push(id.to_string()),
            Err(_) => result.skipped.push(format!("{id}: {SKIP_FAILED}")),
        }
    }
    result
}

/// Install the bundled sources in the operator's Prowlarr. The blocking client
/// runs off the tokio workers; the response is counts and fixed reasons only.
async fn indexers_seed() -> Result<Json<Value>, ApiError> {
    let result = tokio::task::spawn_blocking(seed_indexers)
        .await
        .map_err(|_| {
            ApiError(AppError::new(
                AppErrorCode::Internal,
                "The seeding request failed.",
                true,
            ))
        })?;
    encode(result)
}

// ---------------------------------------------------------------------------
// Source search (Task 3). Reuses the SAME ProwlarrHttp seam as seeding: no
// second client, no credential in a DTO. Results are candidates only; sending
// one still goes through download_add and its executable/probe gates.
// ---------------------------------------------------------------------------

/// Inputs from the Details page. The address and key are never part of this DTO.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SourceSearchArgs {
    title: String,
    year: Option<i32>,
    season: Option<u32>,
    episode: Option<u32>,
}

/// A path-free candidate. `magnet` is the source the operator may choose; `id`
/// is a synthetic `indexer:item` key, never a URL or a filesystem path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceCandidate {
    pub id: String,
    pub title: String,
    pub size: u64,
    pub seeders: u32,
    pub magnet: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceSearchResult {
    pub results: Vec<SourceCandidate>,
}

const MAX_SEARCH_RESULTS: usize = 10;
const MAX_SEARCH_TITLE_LEN: usize = 200;

/// Minimal form encoding for a single Torznab `q` value; no dependency.
fn form_encode(raw: &str) -> String {
    let mut out = String::new();
    for byte in raw.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Inner text of the first `<tag>…</tag>` in a fragment. Handles the namespaced
/// `magnet:link` tag as naturally as plain `title`/`size`/`link`.
fn xml_value<'a>(block: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let from = block.find(&open)? + open.len();
    let to = block[from..].find(&close)?;
    Some(block[from..from + to].trim())
}

/// One attribute value from an XML open tag; both quote styles are accepted.
fn xml_attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    for quote in ['"', '\''] {
        let needle = format!("{name}={quote}");
        if let Some(from) = tag.find(&needle).map(|offset| offset + needle.len()) {
            if let Some(to) = tag[from..].find(quote) {
                return Some(&tag[from..from + to]);
            }
        }
    }
    None
}

/// Value of `<torznab:attr name="…" value="…"/>`; attribute order is not assumed.
fn torznab_attr<'a>(block: &'a str, name: &str) -> Option<&'a str> {
    let mut from = 0usize;
    while let Some(relative) = block[from..].find("<torznab:attr") {
        let start = from + relative;
        let end = block[start..]
            .find("/>")
            .map(|offset| start + offset + 2)
            .or_else(|| block[start..].find('>').map(|offset| start + offset + 1))
            .unwrap_or(block.len());
        let attr = &block[start..end];
        if xml_attr(attr, "name") == Some(name) {
            return xml_attr(attr, "value");
        }
        from = end;
    }
    None
}

/// Seeders from `<torznab:attr name="seeders" value="…"/>`; missing or malformed
/// values degrade to zero (sort falls back).
fn xml_seeders(block: &str) -> u32 {
    torznab_attr(block, "seeders")
        .and_then(|value| value.parse::<f64>().ok())
        .map(|value| value.max(0.0) as u32)
        .unwrap_or(0)
}

/// Build a `btih` magnet from an exact 40-character ASCII hex infohash. Only a
/// hash can take this path: no HTTP URL or bare path can pass the hex check.
fn magnet_from_infohash(hash: &str) -> Option<String> {
    if hash.len() != 40 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!("magnet:?xt=urn:btih:{}", hash.to_ascii_lowercase()))
}

fn numbered(tokens: &[&str], prefix: &str, wanted: u32) -> bool {
    let target = wanted.to_string();
    let padded = format!("{wanted:02}");
    tokens.iter().any(|token| {
        token.strip_prefix(prefix).is_some_and(|rest| {
            let digits: String = rest.chars().take_while(|ch| ch.is_ascii_digit()).collect();
            digits == target || digits == padded
        })
    })
}

fn labelled(tokens: &[&str], label: &str, wanted: u32) -> bool {
    tokens
        .windows(2)
        .any(|pair| pair[0] == label && pair[1].parse::<u32>().ok() == Some(wanted))
}

/// Year/season/episode guards applied to the normalized item title only when
/// the caller supplied them (movies supply a year; series supply S/E).
fn matches_numbers(
    norm: &str,
    year: Option<i32>,
    season: Option<u32>,
    episode: Option<u32>,
) -> bool {
    let tokens: Vec<&str> = norm.split_whitespace().collect();
    if let Some(year) = year {
        let wanted = year.to_string();
        if season.is_none() && episode.is_none() {
            // A movie title must carry the requested year.
            if !tokens.iter().any(|token| *token == wanted) {
                return false;
            }
        } else {
            // TV releases often omit the year. If the title does carry a
            // plausible year, it must still agree. `1080p`/`2160p` are
            // resolutions, not years, so they are excluded by the range.
            let plausible_year = tokens.iter().any(|token| {
                token.len() == 4
                    && token.chars().all(|ch| ch.is_ascii_digit())
                    && token
                        .parse::<u32>()
                        .is_ok_and(|value| (1900..=2099).contains(&value))
            });
            if plausible_year && !tokens.iter().any(|token| *token == wanted) {
                return false;
            }
        }
    }
    if let Some(season) = season {
        let ok = numbered(&tokens, "s", season)
            || labelled(&tokens, "season", season)
            || tokens.iter().any(|token| {
                token
                    .split_once('x')
                    .and_then(|(left, _)| left.parse::<u32>().ok())
                    == Some(season)
            });
        if !ok {
            return false;
        }
    }
    if let Some(episode) = episode {
        let trailing_number = |token: &&str, split: char| {
            token.split_once(split).and_then(|(_, right)| {
                let digits: String = right.chars().take_while(|ch| ch.is_ascii_digit()).collect();
                digits.parse::<u32>().ok()
            }) == Some(episode)
        };
        let ok = numbered(&tokens, "e", episode)
            || labelled(&tokens, "episode", episode)
            || labelled(&tokens, "ep", episode)
            || tokens.iter().any(|token| trailing_number(token, 'e'))
            || tokens.iter().any(|token| trailing_number(token, 'x'));
        if !ok {
            return false;
        }
    }
    true
}

/// Whole-word phrase match on normalized titles. Release titles often prepend
/// articles (`The Arrival …`) or suffix tags, so a prefix check drops real
/// candidates; a token-aligned phrase accepts those without matching unrelated
/// titles that merely contain the words in the wrong order.
fn contains_phrase(norm: &str, wanted: &str) -> bool {
    if wanted.is_empty() {
        return false;
    }
    let haystack: Vec<&str> = norm.split_whitespace().collect();
    let needle: Vec<&str> = wanted.split_whitespace().collect();
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }
    haystack
        .windows(needle.len())
        .any(|window| window == needle.as_slice())
}

fn parse_item(
    block: &str,
    indexer_id: u64,
    index: usize,
    args: &SourceSearchArgs,
) -> Option<SourceCandidate> {
    let title = xml_value(block, "title")?;
    // A Torznab `<link>` is usually an HTTP details page, which the download
    // boundary rejects. Reuse that exact validator here so a candidate can
    // never be offered with a source that `download_add` would refuse. Many
    // feeds carry no usable link at all but do expose a 40-hex `infohash`;
    // synthesise a btih magnet from it, then validate that too through the
    // same boundary. An unusable raw link never becomes a candidate.
    let magnet = xml_value(block, "magnet:link")
        .or_else(|| xml_value(block, "link"))
        .and_then(|raw| media_acquisition::validate_source(raw).ok())
        .or_else(|| {
            torznab_attr(block, "infohash")
                .and_then(magnet_from_infohash)
                .and_then(|magnet| media_acquisition::validate_source(&magnet).ok())
        })?;
    let wanted = normalize_title(args.title.trim());
    let norm = normalize_title(title);
    if !contains_phrase(&norm, &wanted) {
        return None;
    }
    if !matches_numbers(&norm, args.year, args.season, args.episode) {
        return None;
    }
    Some(SourceCandidate {
        id: format!("{indexer_id}-{index}"),
        title: title.to_string(),
        size: xml_value(block, "size")
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0),
        seeders: xml_seeders(block),
        magnet,
    })
}

/// The whole search behind the seeding seam. Every transport failure degrades
/// to fewer candidates; no address, key or raw response text is ever returned.
fn source_search_with(
    http: &ProwlarrHttp,
    base_url: &str,
    api_key: &str,
    args: &SourceSearchArgs,
) -> SourceSearchResult {
    let empty = SourceSearchResult {
        results: Vec::new(),
    };
    let title = args.title.trim();
    if title.is_empty() || title.len() > MAX_SEARCH_TITLE_LEN {
        return empty;
    }
    if base_url.trim().is_empty() || api_key.trim().is_empty() {
        return empty;
    }

    let Ok(indexers) = http.call(base_url, api_key, "/api/v1/indexer", None) else {
        return empty;
    };
    let Some(rows) = indexers.as_array() else {
        return empty;
    };
    let query = format!("&t=search&q={}", form_encode(title));
    let mut results = Vec::new();
    for row in rows {
        // Prowlarr supplies the numeric id; formatting it as u64 keeps the
        // request path free of renderer-controlled text.
        let Some(indexer_id) = row["id"].as_u64() else {
            continue;
        };
        let enabled = row["enable"]
            .as_bool()
            .or_else(|| row["enabled"].as_bool())
            .unwrap_or(false);
        if !enabled {
            continue;
        }
        let Ok(xml) = (http.fetch)(&ProwlarrRequest {
            base_url: base_url.to_string(),
            api_key: api_key.to_string(),
            path: format!("/api/v1/indexer/{indexer_id}/newznab"),
            query: query.clone(),
            body: None,
        }) else {
            continue; // one indexer's failure must not fail the whole search
        };
        for (index, raw_item) in xml.split("<item>").skip(1).enumerate() {
            let block = raw_item.split("</item>").next().unwrap_or(raw_item);
            if let Some(candidate) = parse_item(block, indexer_id, index, args) {
                results.push(candidate);
            }
        }
    }
    results.sort_by(|a, b| b.seeders.cmp(&a.seeders).then_with(|| a.id.cmp(&b.id)));
    results.truncate(MAX_SEARCH_RESULTS);
    SourceSearchResult { results }
}

/// Host route: search the operator's enabled Prowlarr indexers. The blocking
/// client runs off the tokio workers; only path-free candidates are returned.
async fn source_search(ApiJson(args): ApiJson<SourceSearchArgs>) -> Result<Json<Value>, ApiError> {
    let result = tokio::task::spawn_blocking(move || {
        let url = std::env::var(PROWLARR_URL_ENV).unwrap_or_default();
        let key = std::env::var(PROWLARR_KEY_ENV).unwrap_or_default();
        source_search_with(&ProwlarrHttp::live(), &url, &key, &args)
    })
    .await
    .map_err(|_| {
        ApiError(AppError::new(
            AppErrorCode::Internal,
            "The source search failed.",
            true,
        ))
    })?;
    encode(result)
}

fn map_error(error: AcquireError) -> ApiError {
    let code = match error {
        AcquireError::NotFound => AppErrorCode::NotFound,
        AcquireError::Backend => AppErrorCode::BackendUnreachable,
        _ => AppErrorCode::InvalidRequest,
    };
    ApiError(AppError::new(
        code,
        "The acquisition request was rejected.",
        matches!(code, AppErrorCode::BackendUnreachable),
    ))
}

fn encode(item: impl serde::Serialize) -> Result<Json<Value>, ApiError> {
    serde_json::to_value(item).map(Json).map_err(|_| {
        ApiError(AppError::new(
            AppErrorCode::Internal,
            "Encoding failed.",
            true,
        ))
    })
}

async fn add(
    State(state): State<AppState>,
    ApiJson(args): ApiJson<AddArgs>,
) -> Result<Json<Value>, ApiError> {
    // The backend repeats validation at its own trust boundary: the host is a
    // client here, and no path or credential is accepted from the renderer.
    let item = state.acquisition.acquire(&args.source).map_err(map_error)?;
    // Persist the acquisition through the existing Task 1 port. Without this
    // the orchestrator keeps the item only in memory: no production INSERT
    // reaches torrent_tasks/download_items/file_mappings, cleanup never sees
    // app_managed=true, and a real acquisition can never be reconciled.
    // All paths built here are host-internal container paths; no path enters a
    // renderer DTO (the returned DownloadItem is path-free).
    let files = state
        .acquisition
        .files(&item.torrent_hash)
        .unwrap_or_default();
    let paths: Vec<String> = if files.is_empty() {
        vec![format!("{SAVE_PATH}/{}", item.torrent_hash)]
    } else {
        files
            .iter()
            .map(|name| format!("{}/{}", SAVE_PATH, name.trim_start_matches('/')))
            .collect()
    };
    for path in &paths {
        state
            .service
            .download_add(&item, path, &args.source, SAVE_PATH)
            .await?;
    }
    // Bridge the orchestrator's automatic inspection result (computed in memory
    // on acquire) into SQLite so the playability gate can read it. Persist by
    // download id (= torrent hash); the gate looks up by media_id, which stays
    // NULL until the per-episode source mapping (spec R-01 gate) is supplied.
    // Writing it now means no re-scan is needed when that mapping lands.
    if let Some(inspection) = state.acquisition.inspection_state(&item.id) {
        let detail = (inspection != InspectionState::Clean)
            .then(|| format!("probe: {}", inspection.as_str()));
        verify_repo()
            .await?
            .set_verification_state(&item.id, inspection.as_str(), detail.as_deref())
            .await?;
    }
    encode(item)
}

async fn status(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    encode(state.acquisition.status())
}

async fn remove(
    State(state): State<AppState>,
    ApiJson(args): ApiJson<RemoveArgs>,
) -> Result<Json<Value>, ApiError> {
    state.acquisition.remove(&args.id).map_err(map_error)?;
    Ok(Json(Value::Null))
}

// ---------------------------------------------------------------------------
// Downloaded-media inspection gate (security hardening, Task 1). Stays under
// /host/ and outside the 24-command renderer surface. The probe result is
// stored on `download_items`; playback/detail refuse anything not `clean`.
// ---------------------------------------------------------------------------

/// Lazy repository for verification writes, mirroring `CLEANUP_REPO`: `core`
/// opens one pool per process rather than one per request.
static VERIFY_REPO: OnceLock<SqliteLibraryRepository> = OnceLock::new();

async fn verify_repo() -> Result<&'static SqliteLibraryRepository, AppError> {
    if let Some(repo) = VERIFY_REPO.get() {
        return Ok(repo);
    }
    let path = crate::server::default_data_dir().join("library.db");
    let repo = SqliteLibraryRepository::new(connect_path(&path).await.map_err(|_| {
        AppError::new(
            AppErrorCode::BackendUnreachable,
            "The verification store is unavailable.",
            true,
        )
    })?);
    let _ = VERIFY_REPO.set(repo);
    VERIFY_REPO.get().ok_or_else(|| {
        AppError::new(
            AppErrorCode::BackendUnreachable,
            "The verification store is unavailable.",
            true,
        )
    })
}

/// Run the probe gate and persist its result. The runner is injected so tests
/// never touch Docker; `spawn_blocking` keeps the bounded CLI wait off a tokio
/// worker. The detail string is a fixed label, never a filesystem path.
pub async fn inspect_and_record_with<R: ProbeRunner + Send + 'static>(
    repo: &SqliteLibraryRepository,
    runner: R,
    id: &str,
    container_path: &str,
) -> Result<InspectionState, AppError> {
    let path = container_path.to_string();
    let state = tokio::task::spawn_blocking(move || inspect_with_runner(&runner, &path))
        .await
        .map_err(|_| {
            AppError::new(
                AppErrorCode::BackendUnreachable,
                "The inspection task did not complete.",
                true,
            )
        })?;
    let detail = (state != InspectionState::Clean).then(|| format!("probe: {}", state.as_str()));
    repo.set_verification_state(id, state.as_str(), detail.as_deref())
        .await?;
    Ok(state)
}

/// Called by the host when a download is complete, before the item may play.
async fn inspect(
    State(_state): State<AppState>,
    ApiJson(args): ApiJson<InspectArgs>,
) -> Result<Json<Value>, ApiError> {
    if args.id.trim().is_empty() {
        return Err(cleanup_invalid("id is required."));
    }
    let repo = verify_repo().await?;
    let result =
        inspect_and_record_with(repo, DockerProbeRunner, &args.id, &args.container_path).await?;
    Ok(Json(serde_json::json!({ "state": result.as_str() })))
}

// ---------------------------------------------------------------------------
// Completion -> coordinated cleanup (Task 6, ADR-0019). All routes stay under
// /host/ and outside the 24-command renderer surface. No path or credential is
// ever returned to the renderer.
// ---------------------------------------------------------------------------

fn cleanup_invalid(message: &str) -> ApiError {
    ApiError(AppError::new(AppErrorCode::InvalidRequest, message, false))
}

/// Lazily open the cleanup store once. `playback_checkpoint` fires every few
/// seconds, so re-opening a pool (and re-running migrations) per call would be
/// wasteful. The library repository behind the read ports is left untouched.
static CLEANUP_REPO: OnceLock<CleanupRepository> = OnceLock::new();

async fn cleanup_repo() -> Result<&'static CleanupRepository, AppError> {
    if let Some(repo) = CLEANUP_REPO.get() {
        return Ok(repo);
    }
    let path = crate::server::default_data_dir().join("library.db");
    let repo = CleanupRepository::connect_path(&path).await.map_err(|_| {
        AppError::new(
            AppErrorCode::BackendUnreachable,
            "The cleanup store is unavailable.",
            true,
        )
    })?;
    // A racing checkpoint may have won; either repo points at the same DB.
    let _ = CLEANUP_REPO.set(repo);
    CLEANUP_REPO.get().ok_or_else(|| {
        AppError::new(
            AppErrorCode::BackendUnreachable,
            "The cleanup store is unavailable.",
            true,
        )
    })
}

/// The live playback session, owned by `core` because every playback_start /
/// playback_stop route passes through here. This is the authoritative source
/// for "actively playing" — a `download_items.state = playing` field that
/// nothing writes is not a signal (review finding 2).
static LIVE_PLAYBACK: OnceLock<Mutex<Option<String>>> = OnceLock::new();

fn live_playback() -> &'static Mutex<Option<String>> {
    LIVE_PLAYBACK.get_or_init(|| Mutex::new(None))
}

pub fn on_playback_started(media_id: &str) {
    if let Ok(mut live) = live_playback().lock() {
        *live = Some(media_id.to_string());
    }
}

pub fn on_playback_stopped() {
    if let Ok(mut live) = live_playback().lock() {
        *live = None;
    }
}

/// True only for the media id in the current live playback session.
pub fn is_playing_live(media_id: &str) -> bool {
    live_playback()
        .lock()
        .map(|live| live.as_deref() == Some(media_id))
        .unwrap_or(false)
}

/// A scheduled, callable pass (review finding 3). Started on the first
/// confirmed completion; `std`/`tokio` only, no new timer dependency. Each tick
/// only reconciles rows whose 24 h grace has elapsed.
static CLEANUP_SCHEDULER: OnceLock<()> = OnceLock::new();
const CLEANUP_PASS_INTERVAL: Duration = Duration::from_secs(60 * 60);

fn ensure_cleanup_scheduler(state: &AppState) {
    if CLEANUP_SCHEDULER.set(()).is_err() {
        return;
    }
    let state = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(CLEANUP_PASS_INTERVAL).await;
            if let Err(error) = run_cleanup_pass(&state).await {
                tracing::warn!(%error, "scheduled cleanup pass failed");
            }
        }
    });
}

/// Confirm (or refuse) completion with the shared B5 policy. Only a genuine
/// normal EOF watch writes a pending cleanup entry, with a 24 h replay grace.
async fn confirm_completion(
    State(state): State<AppState>,
    ApiJson(args): ApiJson<CompletionSignalArgs>,
) -> Result<Json<Value>, ApiError> {
    if args.media_id.trim().is_empty() {
        return Err(cleanup_invalid("mediaId is required."));
    }
    for value in [
        args.watched_seconds,
        args.duration_seconds,
        args.position_seconds,
    ] {
        if !value.is_finite() || value < 0.0 {
            return Err(cleanup_invalid(
                "playback times must be finite and non-negative.",
            ));
        }
    }

    let confirmed = media_acquisition::completion_confirmed_by_b5(
        args.watched_seconds,
        args.duration_seconds,
        args.position_seconds,
        args.ended_normally,
        args.user_stopped,
    );
    if confirmed {
        let grace = chrono::Duration::from_std(REPLAY_GRACE).unwrap_or_default();
        let eligible_at = (chrono::Utc::now() + grace).to_rfc3339();
        cleanup_repo()
            .await?
            .confirm(&args.media_id, &eligible_at)
            .await?;
        // First confirmed completion starts the scheduled pass; the pass only
        // acts once the 24 h grace has expired (never inline in this request).
        ensure_cleanup_scheduler(&state);
    }
    Ok(Json(serde_json::json!({ "confirmed": confirmed })))
}

/// Called from `playback_checkpoint`, but only AFTER a confirmed completion is
/// recorded. It evaluates eligibility; it never deletes inline. Errors are
/// swallowed so cleanup can never fail a playback request.
pub async fn on_playback_checkpoint(media_id: &str) {
    if let Err(error) = evaluate_after_completion(media_id).await {
        tracing::warn!(%error, "cleanup evaluation skipped; playback unaffected");
    }
}

async fn evaluate_after_completion(media_id: &str) -> Result<(), AppError> {
    let repo = cleanup_repo().await?;
    let Some(entry) = repo.state(media_id).await? else {
        return Ok(()); // no confirmation signal: seeking/close/queue-removal
    };
    if entry.state != "pending" {
        return Ok(()); // only confirmed-completion rows are ever evaluated
    }
    let candidate = build_candidate(repo, media_id, &entry).await?;
    if media_acquisition::is_eligible(&candidate) == CleanupDecision::Eligible {
        // Actual deletion is the scheduled pass's job, never this request.
        tracing::info!(media_id, "cleanup eligible; deferred to scheduled pass");
    }
    Ok(())
}

/// Build the pure eligibility input from host-internal records. A corrupt
/// timestamp fails closed (treated as inside the grace window).
async fn build_candidate(
    repo: &CleanupRepository,
    media_id: &str,
    entry: &CleanupEntry,
) -> Result<CleanupCandidate, AppError> {
    let grace = REPLAY_GRACE.as_secs() as i64;
    let completed_at = chrono::DateTime::parse_from_rfc3339(&entry.eligible_at)
        .map(|instant| instant.timestamp().saturating_sub(grace))
        .unwrap_or(i64::MAX); // corrupt timestamp => never eligible
    Ok(CleanupCandidate {
        completion_confirmed: entry.state == "pending",
        app_managed: !repo.mappings_for_media(media_id).await?.is_empty(),
        completed_at,
        now: chrono::Utc::now().timestamp(),
        grace_seconds: grace,
        // Authoritative live session, not an inert download_items field.
        actively_playing: is_playing_live(media_id),
        prefetch_target: repo.is_prefetch_target(media_id).await?,
    })
}

fn to_managed(row: &ManagedFileRow) -> ManagedFile {
    ManagedFile {
        path: row.path.clone(),
        torrent_hash: row.torrent_hash.clone(),
        media_id: row.media_id.clone(),
        episode_no: row.episode_no,
    }
}

fn state_name(state: CleanupState) -> &'static str {
    match state {
        CleanupState::Removed => "removed",
        CleanupState::Failed => "failed",
    }
}

/// The scheduled pass: reconcile every past-due pending entry. This is a
/// separate host operation (not inline in any playback request) so deletion is
/// always a deliberate, auditable step.
pub async fn run_cleanup_pass(state: &AppState) -> Result<usize, AppError> {
    let repo = cleanup_repo().await?;
    if state.service.cleanup_eligible().await?.is_empty() {
        return Ok(0);
    }
    // No WebUI credentials outside the container: degrade honestly, delete none.
    let api = match QbClient::from_env() {
        Ok(client) => client,
        Err(error) => {
            tracing::warn!(%error, "cleanup pass skipped: qB client unavailable");
            return Ok(0);
        }
    };
    // `core` has only :ro /downloads. The worker deletes through qB (rw bind);
    // `exists` only verifies what qB did. No local filesystem mutation.
    run_cleanup_pass_with(state, repo, api, |path| std::path::Path::new(path).exists()).await
}

/// The reconciliation loop, with the qB API and existence probe injected so an
/// integration test can drive the PRODUCTION path (DB state in, real candidate
/// building, real `reconcile`, real state persistence) with a fake qB. The
/// eligibility/`needed` decisions come from live DB state, never a test flag.
/// Generic bounds (not `dyn`) keep the returned future `Send` for the scheduler
/// when instantiated with the production qB client and path probe.
async fn run_cleanup_pass_with<A: TorrentApi, E: Fn(&str) -> bool>(
    state: &AppState,
    repo: &CleanupRepository,
    api: A,
    exists: E,
) -> Result<usize, AppError> {
    let pending = state.service.cleanup_eligible().await?;
    let worker = CleanupWorker::new(api);
    let mut processed = 0usize;

    for entry in pending {
        let candidate = build_candidate(repo, &entry.media_id, &entry).await?;
        match media_acquisition::is_eligible(&candidate) {
            CleanupDecision::Eligible => {}
            // A confirmed entry with no app-managed mapping can never be
            // cleaned; fall through so the mapping check below records an
            // honest `failed` outcome instead of staying pending forever.
            CleanupDecision::NotEligible(media_acquisition::NotEligibleReason::NotAppManaged) => {}
            CleanupDecision::NotEligible(_) => continue,
        }
        let mappings = repo.mappings_for_media(&entry.media_id).await?;
        let Some(target_row) = mappings.first() else {
            // Reachable for a NotAppManaged decision and as a TOCTOU guard:
            // record an honest failure, delete nothing, never leave pending.
            let refused: Result<media_acquisition::CleanupOutcome, CleanupError> =
                Err(CleanupError::NotAppManaged);
            let (terminal, outcome) = reconcile(&refused);
            repo.set_state(&entry.media_id, state_name(terminal), Some(&outcome))
                .await?;
            processed += 1;
            continue;
        };

        let target = to_managed(target_row);
        let torrent_files: Vec<ManagedFile> = repo
            .mappings_for_torrent(&target.torrent_hash)
            .await?
            .iter()
            .map(to_managed)
            .collect();

        // A SIBLING episode still needs its file when its download is not
        // `watched`. The target itself is excluded: its state flips to Watched
        // only AFTER a successful cleanup, so counting it here would make the
        // target always "still needed" and allow a false "removed 0 file(s)"
        // success (AC-10). NULL media_id rows fail closed in the worker.
        let mut needed: HashSet<String> = HashSet::new();
        for row in &torrent_files {
            if let Some(id) = &row.media_id {
                if id == &entry.media_id {
                    continue;
                }
                if repo.download_state_for_media(id).await? != Some(DownloadState::Watched) {
                    needed.insert(id.clone());
                }
            }
        }
        let still_needed = |file: &ManagedFile| {
            // NULL media_id is always kept; the worker also enforces this
            // intrinsically so no caller can accidentally delete such a file.
            file.media_id
                .as_ref()
                .map(|id| needed.contains(id))
                .unwrap_or(true)
        };

        // No local deleter: deletion is qB's job through its rw bind. `exists`
        // is only a post-condition probe. Zeros can no longer map to Removed.
        let result = worker.execute(&target, &torrent_files, &still_needed, &exists);
        let (terminal, outcome) = reconcile(&result);
        repo.set_state(&entry.media_id, state_name(terminal), Some(&outcome))
            .await?;
        if terminal == CleanupState::Removed {
            if let Some(id) = repo.download_id_for_media(&entry.media_id).await? {
                repo.set_download_state(&id, DownloadState::Watched).await?;
            }
        }
        processed += 1;
    }
    Ok(processed)
}

/// Host-internal trigger for the scheduled pass (a future timer can call the
/// same function). Deletion happens here, never in a playback request.
async fn cleanup_pass(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let processed = run_cleanup_pass(&state).await?;
    Ok(Json(serde_json::json!({ "processed": processed })))
}

// ---------------------------------------------------------------------------
// Rolling two-episode buffer (Task 5). Renderer-neutral: these are host routes
// and a call from the playback route, never a new renderer command.
// ---------------------------------------------------------------------------

/// Host-internal Downloads status: whether the buffer is paused at the G-8 cap
/// and the message to surface. Stays under `/host/`, out of the 24-command
/// renderer surface. The same notice is also on the renderer `DownloadsScreen`
/// DTO; this route carries no path or credential.
async fn buffer_status(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let notice = state.service.downloads().await?.buffer_notice;
    Ok(Json(serde_json::json!({
        "paused": notice.is_some(),
        "message": notice,
    })))
}

/// Retained bytes for app-managed downloads and the configured cap. The
/// Downloads screen currently reports zero used bytes; the decision logic is
/// covered by unit tests with explicit values.
async fn storage_usage(state: &AppState) -> Result<(u64, u64), AppError> {
    let used = state.service.downloads().await?.storage_used_bytes.max(0) as u64;
    let limit = state
        .service
        .settings_get()
        .await?
        .storage
        .limit_bytes
        .filter(|value| *value > 0)
        .map(|value| value as u64)
        .unwrap_or(DEFAULT_BUFFER_LIMIT_BYTES);
    Ok((used, limit))
}

struct Ordering {
    /// One ordered episode list per series. Grouping prevents episodes of two
    /// shows with the same season/episode numbers from mixing in the buffer.
    series: Vec<Vec<EpisodeRef>>,
    available: Vec<String>,
}

/// Episode ordering from `media_items` via the existing read ports, plus the
/// ids that already have a file or an active download (spec §9 steps 1–2).
/// `shortcut: scans up to 200 series; upgrade with a dedicated query if a
/// library exceeds that bound`.
async fn ordering(state: &AppState) -> Result<Ordering, AppError> {
    let query = BrowseQuery {
        sort: BrowseSort::Title,
        genre: None,
        limit: 200,
        offset: 0,
    };
    let mut series = Vec::new();
    let mut available = Vec::new();
    for show in state.service.shows(&query).await? {
        let mut refs = Vec::new();
        for season in &show.seasons {
            for episode in state.service.episodes(&show.id, season.number).await? {
                if matches!(episode.availability, MediaAvailability::Available) {
                    available.push(episode.id.clone());
                }
                refs.push(EpisodeRef {
                    media_id: episode.id,
                    season: episode.season_number,
                    episode: episode.episode_number,
                });
            }
        }
        refs.sort_by_key(|ep| (ep.season, ep.episode));
        if !refs.is_empty() {
            series.push(refs);
        }
    }
    available.extend(
        state
            .acquisition
            .status()
            .into_iter()
            .filter(|item| {
                matches!(
                    item.state,
                    DownloadState::Ready
                        | DownloadState::Downloading
                        | DownloadState::Playing
                        | DownloadState::Watched
                )
            })
            .filter_map(|item| item.media_id),
    );
    Ok(Ordering { series, available })
}

/// Read the operator-supplied source from the library store (ADR-0021). The
/// app bundles no trackers/indexers/catalogues (ADR-0018), so an absent source
/// stays `None` — never an invented one. A store error is degraded to `None`
/// so prefetch can never fail playback.
async fn source_for(state: &AppState, media_id: &str) -> Option<String> {
    state
        .service
        .source_for_media(media_id)
        .await
        .ok()
        .flatten()
}

/// Called from `playback_start` AFTER its policy checks. Never returns a
/// failure to the caller: prefetch must not fail playback (plan risk A-6).
pub async fn trigger_prefetch(state: &AppState, media_id: &str) {
    if let Err(error) = run_prefetch(state, media_id).await {
        tracing::warn!(%error, "prefetch skipped; playback continues");
    }
}

async fn run_prefetch(state: &AppState, media_id: &str) -> Result<(), AppError> {
    let ordering = ordering(state).await?;
    let Some(refs) = ordering
        .series
        .iter()
        .find(|group| group.iter().any(|ep| ep.media_id == media_id))
    else {
        return Ok(()); // a movie or unknown id has no episode ordering
    };

    let (used, limit) = storage_usage(state).await?;
    let next = next_two_in_season(refs, media_id);
    let missing = missing_episodes(&next, &ordering.available);
    // Snapshot the sources before leaving the async context: the scheduler
    // closure runs on a blocking thread and cannot await the store.
    let mut sources = HashMap::new();
    for id in &missing {
        if let Some(source) = source_for(state, id).await {
            sources.insert(id.clone(), source);
        }
    }
    if sources.is_empty() {
        tracing::info!(
            "prefetch: no lawful source mapped for upcoming episodes; buffer not filled"
        );
        return Ok(());
    }

    // The qB client is synchronous with 10 s socket timeouts. `acquire` inside
    // the scheduler would otherwise block a tokio worker; move the whole pure
    // scheduling + enqueue step onto the blocking pool (std/tokio only).
    let acquisition = state.acquisition.clone();
    let scheduler = PrefetchScheduler::new(move |id: &str| {
        let source = sources
            .get(id)
            .cloned()
            .ok_or(AcquireError::InvalidSource)?;
        acquisition.acquire(&source)
    })
    .with_limit(limit);
    let refs = refs.clone();
    let available = ordering.available.clone();
    let current = media_id.to_string();
    let queued = tokio::task::spawn_blocking(move || {
        scheduler.on_playback_start(&refs, &current, &available, used)
    })
    .await
    .map_err(|_| {
        AppError::new(
            AppErrorCode::BackendUnreachable,
            "prefetch task did not complete",
            true,
        )
    })?;
    tracing::info!(count = queued.len(), "prefetch enqueued episodes");
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    #[test]
    fn host_routes_are_outside_the_renderer_command_namespace() {
        for path in [
            ADD_PATH,
            STATUS_PATH,
            REMOVE_PATH,
            BUFFER_STATUS_PATH,
            CLEANUP_CONFIRM_PATH,
            CLEANUP_PASS_PATH,
            INSPECT_PATH,
            SOURCE_SET_PATH,
            IMDB_LOOKUP_PATH,
            SEED_PATH,
            SEARCH_PATH,
        ] {
            assert!(path.starts_with(HOST_PREFIX), "host-only path: {path}");
        }
    }

    /// Offline Torznab fixture: one title-prefix match with the right year and
    /// one prefix match with the wrong year (which must be discarded).
    #[test]
    fn source_search_filters_by_title_and_year_and_keeps_the_magnet() {
        const KEY: &str = "sentinel-prowlarr-key";
        const URL: &str = "http://prowlarr.invalid:9696";
        let http = ProwlarrHttp {
            fetch: Box::new(|request| match request.path.as_str() {
                "/api/v1/indexer" => Ok(serde_json::json!([
                    {"id": 7, "name": "YTS", "enable": true},
                    {"id": 8, "name": "Disabled", "enable": false}
                ])
                .to_string()),
                "/api/v1/indexer/7/newznab" => Ok(concat!(
                    r#"<?xml version="1.0"?><rss><channel>"#,
                    r#"<item><title>The.Right.Movie.1999.1080p</title>"#,
                    r#"<magnet:link>magnet:?xt=urn:btih:WRONG</magnet:link>"#,
                    r#"<size>100</size><torznab:attr name="seeders" value="99"/></item>"#,
                    r#"<item><title>The.Right.Movie.2024.1080p</title>"#,
                    r#"<magnet:link>magnet:?xt=urn:btih:RIGHT</magnet:link>"#,
                    r#"<size>1234</size><torznab:attr name="seeders" value="12"/></item>"#,
                    r#"</channel></rss>"#
                )
                .to_string()),
                other => Err(format!("unexpected path: {other}")),
            }),
        };
        let args = SourceSearchArgs {
            title: "The Right Movie".to_string(),
            year: Some(2024),
            season: None,
            episode: None,
        };

        let result = source_search_with(&http, URL, KEY, &args);

        assert_eq!(result.results.len(), 1, "wrong-year item is discarded");
        let candidate = &result.results[0];
        assert_eq!(candidate.title, "The.Right.Movie.2024.1080p");
        assert_eq!(candidate.magnet, "magnet:?xt=urn:btih:RIGHT");
        assert_eq!(candidate.seeders, 12);
        assert_eq!(candidate.size, 1234);
        let wire = serde_json::to_string(&result).expect("serialize results");
        assert!(!wire.contains(KEY), "the api key must never be returned");
        assert!(!wire.contains("http://"), "no address reaches the renderer");
    }

    #[test]
    fn source_search_accepts_only_magnet_or_bare_torrent_sources() {
        let http = ProwlarrHttp {
            fetch: Box::new(|request| match request.path.as_str() {
                "/api/v1/indexer" => Ok(serde_json::json!([{"id": 1, "enable": true}]).to_string()),
                "/api/v1/indexer/1/newznab" => Ok(concat!(
                    r#"<rss><channel>"#,
                    r#"<item><title>Show.2024.magnet</title>"#,
                    r#"<magnet:link>magnet:?xt=urn:btih:ABC</magnet:link></item>"#,
                    r#"<item><title>Show.2024.torrent</title>"#,
                    r#"<link>show.2024.torrent</link></item>"#,
                    r#"<item><title>Show.2024.http</title>"#,
                    r#"<link>https://indexer.invalid/details/1</link></item>"#,
                    r#"<item><title>Show.2024.path</title>"#,
                    r#"<link>C:\secrets\show.torrent</link></item>"#,
                    r#"</channel></rss>"#
                )
                .to_string()),
                other => Err(format!("unexpected path: {other}")),
            }),
        };
        let args = SourceSearchArgs {
            title: "Show".to_string(),
            year: Some(2024),
            season: None,
            episode: None,
        };

        let result = source_search_with(&http, "http://prowlarr.invalid", "sentinel", &args);

        assert_eq!(
            result.results.len(),
            2,
            "HTTP link and host path are unusable"
        );
        assert_eq!(result.results[0].magnet, "magnet:?xt=urn:btih:ABC");
        assert_eq!(result.results[1].magnet, "show.2024.torrent");
        let wire = serde_json::to_string(&result).expect("serialize results");
        assert!(
            !wire.contains("https://"),
            "no details URL reaches the renderer"
        );
        assert!(!wire.contains("secrets"), "no path reaches the renderer");
    }

    /// Live feeds often carry only an HTTP details link plus a Torznab
    /// infohash. A valid 40-hex hash must yield a btih magnet; a shorter one
    /// and the HTTP link must never become candidates.
    #[test]
    fn source_search_recovers_btih_magnet_from_infohash_when_link_is_http() {
        let http = ProwlarrHttp {
            fetch: Box::new(|request| {
                match request.path.as_str() {
                "/api/v1/indexer" => Ok(serde_json::json!([{"id": 1, "enable": true}]).to_string()),
                "/api/v1/indexer/1/newznab" => Ok(concat!(
                    r#"<rss><channel>"#,
                    r#"<item><title>Show.2024.hash</title>"#,
                    r#"<link>https://indexer.invalid/api/download/1</link>"#,
                    r#"<torznab:attr name="infohash" value="480B6B7D49D536F7881D66695C31F4B340372289"/>"#,
                    r#"<torznab:attr name="seeders" value="5"/></item>"#,
                    r#"<item><title>Show.2024.shorthash</title>"#,
                    r#"<link>https://indexer.invalid/api/download/2</link>"#,
                    r#"<torznab:attr name="infohash" value="480B6B7D49D536F7881D66695C31F4B34037228"/>"#,
                    r#"<torznab:attr name="seeders" value="99"/></item>"#,
                    r#"<item><title>Show.2024.magnet</title>"#,
                    r#"<magnet:link>magnet:?xt=urn:btih:ABC</magnet:link>"#,
                    r#"<torznab:attr name="seeders" value="1"/></item>"#,
                    r#"</channel></rss>"#
                )
                .to_string()),
                other => Err(format!("unexpected path: {other}")),
            }
            }),
        };
        let args = SourceSearchArgs {
            title: "Show".to_string(),
            year: Some(2024),
            season: None,
            episode: None,
        };

        let result = source_search_with(&http, "http://prowlarr.invalid", "sentinel", &args);

        assert_eq!(
            result.results.len(),
            2,
            "the HTTP-only item with a 39-char hash must be dropped"
        );
        let hashed = result
            .results
            .iter()
            .find(|candidate| candidate.title == "Show.2024.hash")
            .expect("infohash item kept");
        assert!(
            hashed.magnet.starts_with("magnet:?xt=urn:btih:"),
            "infohash must become a btih magnet: {}",
            hashed.magnet
        );
        assert!(result
            .results
            .iter()
            .all(|candidate| candidate.title != "Show.2024.shorthash"));
        assert!(result
            .results
            .iter()
            .any(|candidate| candidate.magnet == "magnet:?xt=urn:btih:ABC"));
        let wire = serde_json::to_string(&result).expect("serialize results");
        assert!(
            !wire.contains("https://"),
            "no details URL reaches the renderer"
        );
        assert!(
            !wire.contains("sentinel"),
            "no credential reaches the renderer"
        );
    }

    /// Live release titles prefix an article and suffix quality tags. A wanted
    /// library title must match as a word-aligned phrase anywhere in the title.
    #[test]
    fn source_search_matches_a_title_phrase_not_only_a_prefix() {
        let http = ProwlarrHttp {
            fetch: Box::new(|request| {
                match request.path.as_str() {
                "/api/v1/indexer" => Ok(serde_json::json!([{"id": 1, "enable": true}]).to_string()),
                "/api/v1/indexer/1/newznab" => Ok(concat!(
                    r#"<rss><channel>"#,
                    r#"<item><title>The Arrival (2023) 720p WEBRip x264 -YTS</title>"#,
                    r#"<link>https://indexer.invalid/api/download/1</link>"#,
                    r#"<torznab:attr name="infohash" value="480B6B7D49D536F7881D66695C31F4B340372289"/>"#,
                    r#"</item>"#,
                    r#"</channel></rss>"#
                )
                .to_string()),
                other => Err(format!("unexpected path: {other}")),
            }
            }),
        };
        let args = SourceSearchArgs {
            title: "Arrival".to_string(),
            year: Some(2023),
            season: None,
            episode: None,
        };

        let result = source_search_with(&http, "http://prowlarr.invalid", "sentinel", &args);

        assert_eq!(result.results.len(), 1, "article-prefixed title must match");
        assert_eq!(
            result.results[0].title,
            "The Arrival (2023) 720p WEBRip x264 -YTS"
        );
        assert!(result.results[0].magnet.starts_with("magnet:?xt=urn:btih:"));
        let wire = serde_json::to_string(&result).expect("serialize results");
        assert!(
            !wire.contains("https://"),
            "no details URL reaches the renderer"
        );
        assert!(
            !wire.contains("sentinel"),
            "no credential reaches the renderer"
        );
    }

    #[test]
    fn source_search_year_is_optional_when_season_supplied() {
        let http = ProwlarrHttp {
            fetch: Box::new(|request| match request.path.as_str() {
                "/api/v1/indexer" => Ok(serde_json::json!([{"id": 1, "enable": true}]).to_string()),
                "/api/v1/indexer/1/newznab" => Ok(concat!(
                    r#"<rss><channel>"#,
                    r#"<item><title>Show.Name.S01E02.1080p.mkv</title>"#,
                    r#"<magnet:link>magnet:?xt=urn:btih:OK</magnet:link></item>"#,
                    r#"<item><title>Show.Name.1999.S01E02.1080p.mkv</title>"#,
                    r#"<magnet:link>magnet:?xt=urn:btih:WRONG</magnet:link></item>"#,
                    r#"</channel></rss>"#
                )
                .to_string()),
                other => Err(format!("unexpected path: {other}")),
            }),
        };
        let args = SourceSearchArgs {
            title: "Show Name".to_string(),
            year: Some(2020),
            season: Some(1),
            episode: Some(2),
        };

        let result = source_search_with(&http, "http://prowlarr.invalid", "sentinel", &args);

        assert_eq!(
            result.results.len(),
            1,
            "no-year title kept, wrong-year title dropped"
        );
        assert_eq!(result.results[0].magnet, "magnet:?xt=urn:btih:OK");
    }

    #[test]
    fn source_search_sorts_by_seeders_and_caps_at_ten() {
        let http = ProwlarrHttp {
            fetch: Box::new(|request| match request.path.as_str() {
                "/api/v1/indexer" => Ok(serde_json::json!([{"id": 1, "enable": true}]).to_string()),
                "/api/v1/indexer/1/newznab" => {
                    let mut xml = String::from("<rss><channel>");
                    for n in 0..12 {
                        xml.push_str(&format!(r#"<item><title>Show.S01E01.match.{n:02}</title>"#));
                        xml.push_str(&format!(
                            r#"<magnet:link>magnet:?xt=urn:btih:{n:02}</magnet:link>"#
                        ));
                        xml.push_str(&format!(
                            r#"<torznab:attr name="seeders" value="{n}"/></item>"#
                        ));
                    }
                    xml.push_str("</channel></rss>");
                    Ok(xml)
                }
                other => Err(format!("unexpected path: {other}")),
            }),
        };
        let args = SourceSearchArgs {
            title: "Show".to_string(),
            year: None,
            season: Some(1),
            episode: Some(1),
        };

        let result = source_search_with(&http, "http://prowlarr.invalid", "sentinel", &args);

        assert_eq!(result.results.len(), 10, "cap at ten");
        assert_eq!(result.results[0].seeders, 11, "most-seeded first");
        assert_eq!(result.results[9].seeders, 2);
        assert!(result
            .results
            .iter()
            .all(|candidate| candidate.id.starts_with("1-")));
        let wire = serde_json::to_string(&result).expect("serialize results");
        assert!(!wire.contains("sentinel"));
    }

    #[test]
    fn source_search_without_configuration_makes_no_call() {
        use std::sync::{Arc, Mutex};

        let calls = Arc::new(Mutex::new(0usize));
        let counter = calls.clone();
        let http = ProwlarrHttp {
            fetch: Box::new(move |_| {
                *counter.lock().expect("count call") += 1;
                Ok("[]".to_string())
            }),
        };
        let args = SourceSearchArgs {
            title: "Anything".to_string(),
            year: None,
            season: None,
            episode: None,
        };

        assert!(source_search_with(&http, "  ", "key", &args)
            .results
            .is_empty());
        assert!(
            source_search_with(&http, "http://prowlarr.invalid", "   ", &args)
                .results
                .is_empty()
        );
        assert_eq!(
            *calls.lock().expect("counted calls"),
            0,
            "no call without both inputs"
        );
    }

    /// Offline Prowlarr fixture: three native definitions plus two custom ids
    /// that must never be posted (no matching `definitionName` in Prowlarr).
    fn seed_schema() -> Value {
        serde_json::json!([
            {
                "definitionName": "yts",
                "name": "YTS",
                "implementation": "YTS",
                "configContract": "YTSSettings",
                "protocol": "torrent",
                "fields": [{"name": "baseUrl", "value": ""}],
                "indexerUrls": ["https://yts.example"]
            },
            {
                "definitionName": "nyaasi",
                "name": "Nyaa.si",
                "implementation": "NyaaSi",
                "configContract": "NyaaSiSettings",
                "protocol": "torrent",
                "fields": [{"name": "baseUrl", "value": ""}],
                "indexerUrls": ["https://nyaa.example"]
            },
            {
                "definitionName": "thepiratebay",
                "name": "The Pirate Bay",
                "implementation": "ThePirateBay",
                "configContract": "ThePirateBaySettings",
                "protocol": "torrent",
                "fields": [{"name": "baseUrl", "value": ""}],
                "indexerUrls": ["https://tpb.example"]
            }
        ])
    }

    fn fake_prowlarr(calls: Arc<Mutex<Vec<ProwlarrRequest>>>, existing: Value) -> ProwlarrHttp {
        ProwlarrHttp {
            fetch: Box::new(move |request| {
                calls.lock().expect("record call").push(request.clone());
                match request.path.as_str() {
                    "/api/v1/indexer/schema" => Ok(seed_schema().to_string()),
                    "/api/v1/indexer" if request.body.is_none() => Ok(existing.to_string()),
                    "/api/v1/indexer" => Ok(serde_json::json!({"id": 1}).to_string()),
                    other => Err(format!("unexpected path: {other}")),
                }
            }),
        }
    }

    #[test]
    fn seed_indexers_adds_native_definitions_and_skips_custom_ones() {
        use std::sync::{Arc, Mutex};

        const KEY: &str = "sentinel-prowlarr-key";
        const URL: &str = "http://prowlarr.invalid:9696";
        let calls = Arc::new(Mutex::new(Vec::new()));
        let http = fake_prowlarr(calls.clone(), serde_json::json!([]));

        let result = seed_indexers_with(&http, URL, KEY);

        assert_eq!(result.added, ["yts", "nyaasi", "thepiratebay"]);
        assert!(result
            .skipped
            .iter()
            .any(|reason| reason.contains("eztv-api") && reason.contains("not in Prowlarr")));
        assert!(result
            .skipped
            .iter()
            .any(|reason| reason.contains("animetosho") && reason.contains("not in Prowlarr")));

        let recorded = calls.lock().expect("recorded calls");
        let posts: Vec<&ProwlarrRequest> = recorded
            .iter()
            .filter(|request| request.body.is_some())
            .collect();
        assert_eq!(
            posts.len(),
            3,
            "only the three native definitions are posted"
        );
        for post in &posts {
            let body = post.body.as_ref().expect("post body");
            assert!(
                body["fields"]
                    .as_array()
                    .is_some_and(|fields| !fields.is_empty()),
                "the schema's own fields must be forwarded; Prowlarr returns 400 without them"
            );
            let definition = body["settings"]["definitionFile"].as_str().unwrap_or("");
            assert!(["yts", "nyaasi", "thepiratebay"].contains(&definition));
            assert!(body["settings"]["baseUrl"]
                .as_str()
                .is_some_and(|url| !url.is_empty()));
            assert_eq!(body["appProfileId"], 1);
            assert_eq!(body["enable"], true);
            assert_eq!(body["priority"], 1);
            assert_eq!(body["tags"], serde_json::json!([]));
        }

        // No credential or address reaches the renderer-facing result.
        let wire = serde_json::to_string(&result).expect("serialize result");
        assert!(!wire.contains(KEY));
        assert!(!wire.contains("http://"));
    }

    #[test]
    fn seed_indexers_skips_an_already_present_definition() {
        use std::sync::{Arc, Mutex};

        let calls = Arc::new(Mutex::new(Vec::new()));
        let http = fake_prowlarr(calls.clone(), serde_json::json!([{"name": "YTS"}]));

        let result = seed_indexers_with(&http, "http://prowlarr.invalid:9696", "sentinel-key");

        assert!(!result.added.iter().any(|id| id == "yts"));
        assert!(result
            .skipped
            .iter()
            .any(|reason| reason.contains("yts") && reason.contains("already present")));
        let posted: Vec<String> = calls
            .lock()
            .expect("recorded calls")
            .iter()
            .filter_map(|request| {
                request.body.as_ref().map(|body| {
                    body["settings"]["definitionFile"]
                        .as_str()
                        .unwrap_or("")
                        .to_string()
                })
            })
            .collect();
        assert_eq!(posted, ["nyaasi", "thepiratebay"]);
    }

    #[test]
    fn seed_indexers_without_configuration_returns_an_empty_result() {
        use std::sync::{Arc, Mutex};

        let calls = Arc::new(Mutex::new(Vec::new()));
        let http = fake_prowlarr(calls.clone(), serde_json::json!([]));

        let result = seed_indexers_with(&http, "  ", "sentinel-key");
        assert!(result.added.is_empty() && result.skipped.is_empty());

        let result = seed_indexers_with(&http, "http://prowlarr.invalid:9696", "   ");
        assert!(result.added.is_empty() && result.skipped.is_empty());
        assert!(
            calls.lock().expect("recorded calls").is_empty(),
            "no Prowlarr call without both inputs"
        );
    }

    #[test]
    fn cleanup_terminal_states_map_honestly() {
        assert_eq!(state_name(CleanupState::Removed), "removed");
        assert_eq!(state_name(CleanupState::Failed), "failed");
    }

    #[test]
    fn live_playback_session_excludes_the_currently_playing_media() {
        // Uses the authoritative in-memory session, not a stale DB field.
        let media = "test-playing-media-cleanup";
        on_playback_started(media);
        assert!(is_playing_live(media));
        assert!(!is_playing_live("some-other-media"));

        let grace = REPLAY_GRACE.as_secs() as i64;
        let mut candidate = CleanupCandidate {
            completion_confirmed: true,
            app_managed: true,
            completed_at: 0,
            now: grace + 100,
            grace_seconds: grace,
            actively_playing: is_playing_live(media),
            prefetch_target: false,
        };
        assert_eq!(
            media_acquisition::is_eligible(&candidate),
            CleanupDecision::NotEligible(media_acquisition::NotEligibleReason::ActivelyPlaying)
        );

        on_playback_stopped();
        assert!(!is_playing_live(media));
        candidate.actively_playing = is_playing_live(media);
        assert_eq!(
            media_acquisition::is_eligible(&candidate),
            CleanupDecision::Eligible
        );
    }

    #[tokio::test]
    async fn prefetch_reads_the_operator_source_from_the_store() {
        use app_core::application::LibraryService;

        let dir = std::env::temp_dir().join(format!("mh-r01-prefetch-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let pool = connect_path(&dir.join("library.db"))
            .await
            .expect("connect and migrate");
        let service = LibraryService::new(SqliteLibraryRepository::new(pool.clone()));
        service
            .set_item_source("s1e2", "magnet:?xt=urn:btih:ABC123&dn=ep")
            .await
            .expect("store operator source");
        let state = AppState::new(service, "test-token");

        assert_eq!(
            source_for(&state, "s1e2").await.as_deref(),
            Some("magnet:?xt=urn:btih:ABC123&dn=ep"),
            "prefetch must use the source the operator supplied"
        );
        assert!(
            source_for(&state, "s1e3").await.is_none(),
            "no source is invented for an unmapped episode"
        );

        pool.close().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    /// End-to-end offline path: operator TSVs in a directory, a library item
    /// with a matching title/year, and the opt-in on -> display fields populated.
    /// Opt-in off or unknown media id degrades to all `None`. No network.
    #[tokio::test]
    async fn imdb_lookup_serves_offline_fields_only_when_enabled() {
        use app_core::application::LibraryService;
        use media_metadata::{FileImdbFetch, ImdbOptions, ImdbProvider};

        let dir = std::env::temp_dir().join(format!("mh-imdb-route-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("title.basics.tsv"),
            "tconst\ttitleType\tprimaryTitle\toriginalTitle\tisAdult\tstartYear\tendYear\truntimeMinutes\tgenres\n\
tt0111161\tmovie\tThe Shawshank Redemption\tThe Shawshank Redemption\t0\t1994\t\\N\t142\tDrama\n",
        )
        .expect("basics fixture");
        std::fs::write(
            dir.join("title.ratings.tsv"),
            "tconst\taverageRating\tnumVotes\n\
tt0111161\t9.3\t2500000\n",
        )
        .expect("ratings fixture");
        std::fs::write(
            dir.join("age_ratings.tsv"),
            "tconst\tcertification\n\
tt0111161\tR\n",
        )
        .expect("age ratings fixture");

        let pool = connect_path(&dir.join("library.db"))
            .await
            .expect("connect and migrate");
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO media_items (id, kind, title, year, created_at, updated_at) \
             VALUES ('m-imdb', 'movie', 'The Shawshank Redemption', 1994, ?, ?)",
        )
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .expect("insert media");
        let service = || LibraryService::new(SqliteLibraryRepository::new(pool.clone()));

        let enabled = AppState::new(service(), "test-token").with_imdb(ImdbProvider::with_fetch(
            ImdbOptions { enabled: true },
            Box::new(FileImdbFetch::new(&dir)),
        ));
        let display = lookup_display(&enabled, "m-imdb").await;
        assert_eq!(display.imdb_rating, Some(9.3));
        assert_eq!(display.vote_count, Some(2_500_000));
        assert_eq!(display.age_rating.as_deref(), Some("R"));

        // Opt-in off: no fields are served, even though the files exist.
        let disabled = AppState::new(service(), "test-token");
        let empty = lookup_display(&disabled, "m-imdb").await;
        assert_eq!(empty.imdb_rating, None);
        assert_eq!(empty.vote_count, None);
        assert_eq!(empty.age_rating, None);

        // Unknown media id also degrades to an empty display.
        let unknown = lookup_display(&enabled, "m-missing").await;
        assert_eq!(unknown.imdb_rating, None);

        // The same fields travel through the signed host route the Tauri
        // `imdb_lookup` command calls — display-only JSON, no path or token.
        use axum::body::{to_bytes, Body};
        use axum::http::{header, Request, StatusCode};
        use std::time::{SystemTime, UNIX_EPOCH};
        use tower::ServiceExt;

        const TOKEN: &str = "test-token"; // matches the AppState built above
        let router = crate::server::build_router(enabled);
        let body = serde_json::json!({ "mediaId": "m-imdb" }).to_string();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_secs()
            .to_string();
        let signature =
            crate::auth::sign(TOKEN, &timestamp, "POST", IMDB_LOOKUP_PATH, body.as_bytes());
        let request = Request::builder()
            .method("POST")
            .uri(IMDB_LOOKUP_PATH)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
            .header(crate::auth::HEADER_TIMESTAMP, &timestamp)
            .header(crate::auth::HEADER_SIGNATURE, &signature)
            .body(Body::from(body))
            .expect("build signed request");
        let response = router.oneshot(request).await.expect("route responds");
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), 1024 * 1024)
            .await
            .expect("read body");
        let wire: serde_json::Value = serde_json::from_slice(&bytes).expect("JSON body");
        assert_eq!(wire["imdbRating"], serde_json::json!(9.3));
        assert_eq!(wire["voteCount"], serde_json::json!(2_500_000));
        assert_eq!(wire["ageRating"], serde_json::json!("R"));
        let dir_text = dir.to_string_lossy();
        assert!(
            !String::from_utf8_lossy(&bytes).contains(dir_text.as_ref()),
            "the dataset directory path must not reach the renderer"
        );

        pool.close().await;
        std::fs::remove_dir_all(&dir).ok();
    }
}

/// Integration coverage through the PRODUCTION reconciliation path: real
/// SQLite state, real candidate building, real `reconcile`, real persistence.
/// Only qB's effect (and its verification probe) is faked — no live engine, no
/// real media, no injected eligibility predicate. Finding 3 / AC-10.
#[cfg(test)]
mod production_wiring_tests {
    use std::cell::RefCell;
    use std::collections::HashSet;
    use std::rc::Rc;

    use app_core::application::LibraryService;
    use app_core::domain::{DownloadItem, DownloadState};
    use media_acquisition::{QbError, TorrentStatus};
    use media_persistence::sqlite::{connect_path, CleanupRepository, SqliteLibraryRepository};

    use super::*;

    struct FakeQb {
        removed: Rc<RefCell<Vec<(String, bool)>>>,
        live: Rc<RefCell<HashSet<String>>>,
    }

    impl TorrentApi for FakeQb {
        fn add(&self, _source: &str, _save_path: &str) -> Result<String, QbError> {
            Ok("fake-hash".to_string())
        }
        fn status(&self, hash: &str) -> Result<TorrentStatus, QbError> {
            Ok(TorrentStatus {
                hash: hash.to_string(),
                state: "ready".to_string(),
                progress: 1.0,
            })
        }
        fn files(&self, _hash: &str) -> Result<Vec<String>, QbError> {
            Ok(Vec::new())
        }
        fn remove(&self, hash: &str, delete_files: bool) -> Result<(), QbError> {
            self.removed
                .borrow_mut()
                .push((hash.to_string(), delete_files));
            if delete_files {
                // qB owns the rw bind: a confirmed deletion really removes the
                // file from the shared /downloads store.
                self.live.borrow_mut().clear();
            }
            Ok(())
        }
    }

    fn temp_db(tag: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("mh-final-fix-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        (dir.clone(), dir.join("library.db"))
    }

    fn probe(live: &Rc<RefCell<HashSet<String>>>) -> impl Fn(&str) -> bool + '_ {
        let live = live.clone();
        move |path: &str| live.borrow().contains(path)
    }

    #[tokio::test]
    async fn app_managed_grace_elapsed_completed_entry_is_actually_removed() {
        let (_dir, db) = temp_db("removed");
        let pool = connect_path(&db).await.expect("connect and migrate");
        let cleanup_repo = CleanupRepository::new(pool.clone());
        let service = LibraryService::new(SqliteLibraryRepository::new(pool));
        let state = AppState::new(service, "test-token");

        const PATH: &str = "/downloads/show/e01.mkv";
        let item = DownloadItem {
            id: "d-remove".to_string(),
            torrent_hash: "h-remove".to_string(),
            media_id: Some("m-remove".to_string()),
            state: DownloadState::Ready,
            progress: 1.0,
        };
        // The production port persists item + torrent task + file mapping.
        state
            .service
            .download_add(&item, PATH, "magnet:?xt=urn:btih:remove", SAVE_PATH)
            .await
            .expect("download_add persists item and mapping");
        let past = (chrono::Utc::now() - chrono::Duration::hours(48)).to_rfc3339();
        cleanup_repo
            .confirm("m-remove", &past)
            .await
            .expect("confirm");

        let live = Rc::new(RefCell::new(HashSet::from([PATH.to_string()])));
        let removed = Rc::new(RefCell::new(Vec::new()));
        let api = FakeQb {
            removed: removed.clone(),
            live: live.clone(),
        };
        let exists = probe(&live);

        let processed = run_cleanup_pass_with(&state, &cleanup_repo, api, &exists)
            .await
            .expect("pass");

        assert_eq!(processed, 1);
        assert_eq!(
            *removed.borrow(),
            vec![("h-remove".to_string(), true)],
            "qB was asked to delete the torrent AND its files"
        );
        assert!(live.borrow().is_empty(), "the file is really gone");
        let entry = cleanup_repo
            .state("m-remove")
            .await
            .expect("state")
            .expect("entry exists");
        assert_eq!(entry.state, "removed", "honest terminal state");
        assert!(
            entry.outcome.unwrap_or_default().contains("removed 1 file"),
            "visible removal outcome"
        );
    }

    #[tokio::test]
    async fn zero_removal_is_never_reported_removed_through_production_wiring() {
        let (_dir, db) = temp_db("kept");
        let pool = connect_path(&db).await.expect("connect and migrate");
        let cleanup_repo = CleanupRepository::new(pool.clone());
        let service = LibraryService::new(SqliteLibraryRepository::new(pool));
        let state = AppState::new(service, "test-token");

        const A_PATH: &str = "/downloads/show/e01.mkv";
        const B_PATH: &str = "/downloads/show/e02.mkv";
        // Two episodes of one torrent: B is still needed (not watched), so the
        // target A must not be deleted by core (read-only) nor by qB (would
        // delete B too). Zero deletions must NOT be reported as Removed.
        for (id, media, path) in [("d-a", "m-a", A_PATH), ("d-b", "m-b", B_PATH)] {
            let item = DownloadItem {
                id: id.to_string(),
                torrent_hash: "h-keep".to_string(),
                media_id: Some(media.to_string()),
                state: DownloadState::Ready,
                progress: 1.0,
            };
            state
                .service
                .download_add(&item, path, "magnet:?xt=urn:btih:keep", SAVE_PATH)
                .await
                .expect("download_add persists item and mapping");
        }
        let past = (chrono::Utc::now() - chrono::Duration::hours(48)).to_rfc3339();
        cleanup_repo.confirm("m-a", &past).await.expect("confirm");

        let live = Rc::new(RefCell::new(HashSet::from([
            A_PATH.to_string(),
            B_PATH.to_string(),
        ])));
        let removed = Rc::new(RefCell::new(Vec::new()));
        let api = FakeQb {
            removed: removed.clone(),
            live: live.clone(),
        };
        let exists = probe(&live);

        let processed = run_cleanup_pass_with(&state, &cleanup_repo, api, &exists)
            .await
            .expect("pass");

        assert_eq!(processed, 1);
        assert_eq!(
            *removed.borrow(),
            vec![("h-keep".to_string(), false)],
            "task removed with deleteFiles=false; all files kept"
        );
        assert_eq!(live.borrow().len(), 2, "both files survive: no data loss");
        let entry = cleanup_repo
            .state("m-a")
            .await
            .expect("state")
            .expect("entry exists");
        assert_eq!(entry.state, "failed", "zero deletions are not 'removed'");
        assert!(
            !entry.outcome.unwrap_or_default().contains("removed"),
            "no false removal claim"
        );
    }

    struct RefusingProbe;
    impl ProbeRunner for RefusingProbe {
        fn run(
            &self,
            _container_path: &str,
        ) -> Result<media_acquisition::ProbeOutput, media_acquisition::ProbeError> {
            Ok(media_acquisition::ProbeOutput {
                success: false,
                stdout: String::new(),
            })
        }
    }

    struct AcceptingProbe;
    impl ProbeRunner for AcceptingProbe {
        fn run(
            &self,
            _container_path: &str,
        ) -> Result<media_acquisition::ProbeOutput, media_acquisition::ProbeError> {
            Ok(media_acquisition::ProbeOutput {
                success: true,
                stdout: r#"{"format":{"format_name":"matroska","duration":"1.0"}}"#.to_string(),
            })
        }
    }

    /// Production wiring: real SQLite, real persistence port, real gate — only
    /// the probe's observation is faked. Proves a non-Clean result blocks the
    /// same `detail` call the renderer's `library_detail` route makes.
    #[tokio::test]
    async fn probe_failure_is_persisted_and_blocks_playback_through_production_wiring() {
        let (_dir, db) = temp_db("inspect");
        let pool = connect_path(&db).await.expect("connect and migrate");
        let repo = SqliteLibraryRepository::new(pool.clone());
        let service = LibraryService::new(SqliteLibraryRepository::new(pool.clone()));

        const PATH: &str = "/downloads/bad/e01.mkv";
        let item = DownloadItem {
            id: "d-inspect".to_string(),
            torrent_hash: "h-inspect".to_string(),
            media_id: Some("m-inspect".to_string()),
            state: DownloadState::Ready,
            progress: 1.0,
        };
        service
            .download_add(&item, PATH, "magnet:?xt=urn:btih:inspect", SAVE_PATH)
            .await
            .expect("download_add persists the item");
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO media_items (id, kind, title, created_at, updated_at) \
             VALUES ('m-inspect', 'movie', 'title', ?, ?)",
        )
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .expect("insert media");

        let result = inspect_and_record_with(&repo, RefusingProbe, "d-inspect", PATH)
            .await
            .expect("inspection completes");
        assert_eq!(result, InspectionState::Suspicious);
        assert_eq!(
            repo.verification_state_for_media("m-inspect")
                .await
                .unwrap()
                .as_deref(),
            Some("suspicious")
        );
        let error = service.detail("m-inspect").await.unwrap_err();
        assert_eq!(error.code, AppErrorCode::MediaUnsupported);
        assert!(!error.message.contains("/downloads"), "display-only reason");

        // A clean result must re-enable the very same call: no stale refusal.
        let result = inspect_and_record_with(&repo, AcceptingProbe, "d-inspect", PATH)
            .await
            .expect("inspection completes");
        assert_eq!(result, InspectionState::Clean);
        // Task 2: detail now checks both gates; set the Defender half too.
        repo.set_defender_verification("d-inspect", "clean", None)
            .await
            .expect("record defender clean");
        assert!(service.detail("m-inspect").await.is_ok());
    }
}
