//! Host-only discovery clients: operator Torznab search and opt-in TMDB/AniList catalogs.
//!
//! No indexer/tracker/content-site list and no acquisition instruction is
//! bundled. The operator supplies an endpoint/key (direct Torznab, or a local
//! Prowlarr instance whose enabled indexers are discovered at runtime); absent
//! config makes both clients zero-request no-ops. Only a title, year, query and poster path ever
//! leave the machine. All parsing/sanitisation happens here on the host, and
//! every display field is bounded and control-character free before it can
//! reach the renderer. HTTP transport is injected so tests use fake responses
//! only — no live network call exists in any test (ADR-0024).

use serde::Serialize;
use serde_json::Value;
use std::io::Read;
use std::time::Duration;

/// Maximum raw response body accepted from any remote endpoint (1 MiB).
pub const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
/// Prowlarr `/api/v1/search` returns far more JSON than one Torznab RSS page.
pub const MAX_PROWLARR_BYTES: usize = 4 * 1024 * 1024;
/// Maximum ranked search candidates returned to the renderer.
pub const MAX_SHORTLIST: usize = 20;
/// Maximum catalog cards returned to the renderer.
pub const MAX_CATALOG: usize = 40;
/// Real Prowlarr magnet URIs routinely exceed 2 KiB (tracker + web seeds).
const MAX_URI_LEN: usize = 8192;

/// Pluggable HTTP fetch. Receives the fully-built URL and an optional API key
/// that must be sent as an `X-Api-Key` header (Prowlarr JSON); URL-only clients
/// pass `None`. Production implementations are [`http_fetch`] and
/// [`http_fetch_with_api_key`]; tests inject fakes.
pub type FetchFn = dyn Fn(&str, Option<&str>) -> Result<Vec<u8>, String> + Send + Sync;
pub type AniListFetchFn = dyn Fn(&str, &str) -> Result<Vec<u8>, String> + Send + Sync;

/// Failure modes kept deliberately opaque: the command layer degrades to an
/// empty list and never forwards this text to the renderer.
#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    #[error("search response too large")]
    ResponseTooLarge,
    #[error("search request failed")]
    Fetch,
}

/// A ranked, display-only source candidate. `magnet` is handed to the existing
/// `source_set` verbatim by the frontend after a human confirms the choice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortlistItem {
    pub title: String,
    pub size_bytes: i64,
    pub seeders: i64,
    pub resolution: String,
    pub source: String,
    pub magnet: String,
}

/// A display-only catalog card. `kind` is forced into the closed set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogItem {
    pub id: String,
    pub title: String,
    pub year: Option<i64>,
    pub kind: String,
    pub overview: String,
    pub poster_path: Option<String>,
    pub poster_url: Option<String>,
    pub provider: String,
}

/// The real host HTTP transport: rustls, caller-chosen body cap and a 10s
/// timeout. The cap is enforced both from Content-Length and while streaming,
/// so a chunked/absent Content-Length cannot grow the buffer past `max`.
pub fn http_fetch_bounded(url: &str, max: usize) -> Result<Vec<u8>, String> {
    http_fetch_with(url, max, None)
}

/// Prowlarr v3 times out on `apikey=` query auth; it expects `X-Api-Key`. The
/// key is only ever placed in that header, never in the URL or a log line.
/// Uses the Prowlarr JSON body cap and the same bounded streaming as
/// [`http_fetch_bounded`].
pub fn http_fetch_with_api_key(url: &str, api_key: &str) -> Result<Vec<u8>, String> {
    http_fetch_with(url, MAX_PROWLARR_BYTES, Some(api_key))
}

fn http_fetch_with(url: &str, max: usize, api_key: Option<&str>) -> Result<Vec<u8>, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        // Operator endpoints (Prowlarr/Torznab) are local or explicitly
        // configured. Never route them through a system proxy; some setups
        // leave proxy variables set and break loopback requests.
        .no_proxy()
        .user_agent("MEDIA_HAVEN/0.1")
        // Disable redirects: callers validate the initial URL's origin (e.g.
        // same-origin discovery), and auto-following would send the request to
        // an unvalidated destination, bypassing that rule.
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| error.to_string())?;
    let mut request = client.get(url);
    if let Some(key) = api_key {
        request = request.header("X-Api-Key", key);
    }
    let mut response = request.send().map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("http {}", response.status().as_u16()));
    }
    if response
        .content_length()
        .is_some_and(|length| length > max as u64)
    {
        return Err("response too large".to_string());
    }
    let mut bounded = response.by_ref().take(max as u64 + 1);
    let mut bytes = Vec::new();
    bounded
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > max {
        return Err("response too large".to_string());
    }
    Ok(bytes)
}

/// Default transport for Torznab/TMDB: unchanged 1 MiB cap.
pub fn http_fetch(url: &str) -> Result<Vec<u8>, String> {
    http_fetch_bounded(url, MAX_RESPONSE_BYTES)
}

// --- Torznab search --------------------------------------------------------

/// Build a bounded, deterministically ranked shortlist from one operator-
/// configured Torznab endpoint. Missing endpoint/key/title means zero fetch
/// calls and an empty list. XML is parsed with a bounded hand-rolled scanner —
/// no extra crate. Results are capped at [`MAX_SHORTLIST`].
pub fn torznab_search(
    endpoint: &str,
    key: &str,
    title: &str,
    year: Option<i64>,
    fetch: &FetchFn,
) -> Result<Vec<ShortlistItem>, DiscoveryError> {
    let endpoint = endpoint.trim();
    let key = key.trim();
    let title = title.trim();
    if endpoint.is_empty() || key.is_empty() || title.is_empty() || is_path_like(title) {
        return Ok(Vec::new());
    }

    let query = match year {
        Some(year) => format!("{title} {year}"),
        None => title.to_string(),
    };
    let separator = if endpoint.contains('?') { '&' } else { '?' };
    let url = format!(
        "{endpoint}{separator}t=search&q={}&apikey={}",
        pct(&query),
        pct(key)
    );

    let bytes = fetch(&url, None).map_err(|_| DiscoveryError::Fetch)?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(DiscoveryError::ResponseTooLarge);
    }
    let xml = String::from_utf8_lossy(&bytes);

    let mut items = Vec::new();
    for block in rss_items(&xml) {
        let Some(raw_title) = tag_text(block, "title") else {
            continue;
        };
        let Some(display_title) = sanitize_display(&raw_title, 200) else {
            continue;
        };
        // A title carrying a path or traversal sequence is hostile data, never
        // something to display or hand to source_set.
        if is_path_like(&display_title) {
            continue;
        }
        let attrs = torznab_attrs(block);
        let magnet = attr_value(block, &attrs, "magneturl")
            .or_else(|| tag_text(block, "link"))
            .or_else(|| tag_text(block, "guid"))
            .and_then(|value| sanitize_magnet(&value));
        let Some(magnet) = magnet else { continue };
        let size_bytes = attr_value(block, &attrs, "size")
            .or_else(|| tag_text(block, "size"))
            .and_then(|value| decode_entities(&value).parse::<i64>().ok())
            .filter(|size| *size >= 0)
            .unwrap_or(0);
        let seeders = attr_value(block, &attrs, "seeders")
            .or_else(|| tag_text(block, "seeders"))
            .and_then(|value| decode_entities(&value).parse::<i64>().ok())
            .filter(|count| *count >= 0)
            .unwrap_or(0);
        let resolution = detect_resolution(&display_title);
        let source = attr_value(block, &attrs, "source")
            .or_else(|| tag_text(block, "source"))
            .and_then(|value| sanitize_display(&value, 80))
            .unwrap_or_else(|| "Torznab".to_string());

        items.push(ShortlistItem {
            title: display_title,
            size_bytes,
            seeders,
            resolution,
            source,
            magnet,
        });
    }

    Ok(rank_shortlist(items))
}

// --- Prowlarr v3 JSON search -----------------------------------------------

/// Search a Prowlarr v3 instance through its JSON API. Some operator installs
/// expose per-indexer Torznab URLs that 404 while `/api/v1/search` works; this
/// client covers that case. Blank config or a path-like title means zero fetch
/// calls. The caller injects the transport (production: [`http_fetch_bounded`]
/// with [`MAX_PROWLARR_BYTES`]); this function enforces the cap itself too.
pub fn prowlarr_search(
    base: &str,
    key: &str,
    title: &str,
    year: Option<i64>,
    fetch: &FetchFn,
) -> Result<Vec<ShortlistItem>, DiscoveryError> {
    let base = base.trim().trim_end_matches('/');
    let key = key.trim();
    let title = title.trim();
    if base.is_empty() || key.is_empty() || title.is_empty() || is_path_like(title) {
        return Ok(Vec::new());
    }

    let query = match year {
        Some(year) => format!("{title} {year}"),
        None => title.to_string(),
    };
    let separator = if base.contains('?') { '&' } else { '?' };
    // Prowlarr v3 rejects query-parameter auth; the key travels as X-Api-Key
    // through the fetch seam so it never appears in this URL.
    let url = format!("{base}/api/v1/search{separator}query={}", pct(&query));

    let bytes = fetch(&url, Some(key)).map_err(|_| DiscoveryError::Fetch)?;
    if bytes.len() > MAX_PROWLARR_BYTES {
        return Err(DiscoveryError::ResponseTooLarge);
    }
    let Ok(Value::Array(rows)) = serde_json::from_slice::<Value>(&bytes) else {
        return Ok(Vec::new());
    };

    let mut items = Vec::new();
    for row in rows {
        let Some(raw_title) = row.get("title").and_then(Value::as_str) else {
            continue;
        };
        let Some(display_title) = sanitize_display(raw_title, 200).filter(|t| !is_path_like(t))
        else {
            continue;
        };
        let Some(magnet) = row
            .get("magnetUrl")
            .and_then(Value::as_str)
            .and_then(sanitize_magnet)
        else {
            continue;
        };
        let size_bytes = json_i64(row.get("size"));
        let seeders = json_i64(row.get("seeders"));
        let source = row
            .get("indexer")
            .and_then(Value::as_str)
            .and_then(|raw| sanitize_display(raw, 80))
            .unwrap_or_else(|| "Prowlarr".to_string());
        let resolution = prowlarr_resolution(&display_title);
        items.push(ShortlistItem {
            title: display_title,
            size_bytes,
            seeders,
            resolution,
            source,
            magnet,
        });
        if items.len() == MAX_SHORTLIST {
            break;
        }
    }
    Ok(rank_shortlist(items))
}

/// JSON numbers and numeric strings are both accepted; negatives and junk
/// become 0 so a malformed row cannot distort ranking.
fn json_i64(value: Option<&Value>) -> i64 {
    value
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str().and_then(|raw| raw.parse().ok()))
        })
        .filter(|number| *number >= 0)
        .unwrap_or(0)
}

/// Exact lowercase tokens only, in descending quality: no case folding, so a
/// mis-cased token cannot be presented as a real resolution label.
fn prowlarr_resolution(title: &str) -> String {
    for token in ["2160p", "1080p", "720p", "480p"] {
        if title.contains(token) {
            return token.to_string();
        }
    }
    String::new()
}

// --- Prowlarr endpoint discovery and multi-endpoint search -----------------

/// Maximum indexer endpoints discovered from one Prowlarr instance.
const MAX_PROWLARR_ENDPOINTS: usize = 10;

/// Resolve the enabled indexers of a local Prowlarr instance into bare Torznab
/// endpoint URLs. Blank config means zero fetch calls. Only the base URL and
/// key are used; hostile or non-HTTP endpoints are dropped, never returned to
/// the search layer. Discovered endpoints must share the base URL's origin:
/// the Prowlarr key is appended to each one, so a foreign host must never see
/// it, and an indexer definition must not turn discovery into an SSRF pivot.
pub fn prowlarr_torznab_endpoints(
    base_url: &str,
    key: &str,
    fetch: &FetchFn,
) -> Result<Vec<String>, DiscoveryError> {
    let base = base_url.trim().trim_end_matches('/');
    let key = key.trim();
    if base.is_empty() || key.is_empty() {
        return Ok(Vec::new());
    }
    // Validate the configured origin before any request; an unusable base
    // fails closed with zero calls.
    let Some(base_origin) = sanitize_endpoint(base).and_then(|url| endpoint_origin(&url)) else {
        return Ok(Vec::new());
    };
    let url = format!("{base}/api/v1/indexer?apikey={}", pct(key));
    let bytes = fetch(&url, None).map_err(|_| DiscoveryError::Fetch)?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(DiscoveryError::ResponseTooLarge);
    }
    let Ok(Value::Array(indexers)) = serde_json::from_slice::<Value>(&bytes) else {
        return Ok(Vec::new());
    };

    let mut endpoints = Vec::new();
    for indexer in indexers {
        if !indexer
            .get("enable")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            continue;
        }
        let candidate = indexer
            .get("torznabUrl")
            .and_then(Value::as_str)
            .filter(|url| !url.trim().is_empty())
            .map(str::to_string)
            .or_else(|| {
                indexer
                    .get("id")
                    .and_then(Value::as_u64)
                    .map(|id| format!("{base}/api/v1/indexer/{id}/torznab"))
            });
        // Our search appends its own `?t=search...apikey=...`, so strip any
        // query the indexer definition carried.
        let Some(candidate) = candidate.and_then(|raw| {
            let bare = raw.split('?').next().unwrap_or("").trim();
            sanitize_endpoint(bare)
                .filter(|url| endpoint_origin(url).as_ref() == Some(&base_origin))
        }) else {
            continue;
        };
        if !endpoints.contains(&candidate) {
            endpoints.push(candidate);
        }
        if endpoints.len() == MAX_PROWLARR_ENDPOINTS {
            break;
        }
    }
    Ok(endpoints)
}

/// Deterministic order: seeders desc, size desc, resolution desc, title asc.
pub fn rank_shortlist(mut items: Vec<ShortlistItem>) -> Vec<ShortlistItem> {
    items.sort_by(|a, b| {
        b.seeders
            .cmp(&a.seeders)
            .then(b.size_bytes.cmp(&a.size_bytes))
            .then(resolution_rank(&b.resolution).cmp(&resolution_rank(&a.resolution)))
            .then(a.title.cmp(&b.title))
    });
    items.truncate(MAX_SHORTLIST);
    items
}

/// Search every discovered endpoint and return one globally ranked shortlist.
/// A single endpoint behaves exactly like [`torznab_search`]. Any endpoint
/// failure fails the whole call so the command layer degrades to empty results.
pub fn torznab_search_multi(
    endpoints: &[String],
    key: &str,
    title: &str,
    year: Option<i64>,
    fetch: &FetchFn,
) -> Result<Vec<ShortlistItem>, DiscoveryError> {
    let mut items = Vec::new();
    for endpoint in endpoints {
        items.extend(torznab_search(endpoint, key, title, year, fetch)?);
    }
    Ok(rank_shortlist(items))
}

/// An endpoint must be an absolute HTTP(S) URL with a host and no traversal,
/// userinfo, backslash, fragment, control character or extra query.
fn sanitize_endpoint(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.is_empty()
        || value.len() > MAX_URI_LEN
        || value
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace() || is_format_char(ch))
    {
        return None;
    }
    let (scheme, rest) = value.split_once("://")?;
    if !(scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https"))
        || rest.is_empty()
        || rest.starts_with('/')
        || rest.contains('\\')
        || rest.contains("..")
        || rest.contains('@')
        || rest.contains('#')
    {
        return None;
    }
    Some(value.to_string())
}

/// Scheme (lowercased), host (lowercased) and effective port of an endpoint.
/// Missing ports default to 80/443, so `http://x` and `http://x:80` match.
/// Invalid or unusual authorities return `None` and are treated as hostile.
fn endpoint_origin(url: &str) -> Option<(String, String, u16)> {
    let (scheme, rest) = url.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    let default_port = match scheme.as_str() {
        "http" => 80,
        "https" => 443,
        _ => return None,
    };
    let authority = rest.split(['/', '?', '#']).next()?;
    if authority.is_empty() || authority.contains('@') || authority.contains('\\') {
        return None;
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (host, port.parse::<u16>().ok()?),
        None => (authority, default_port),
    };
    // Unbracketed IPv6 hosts are refused rather than guessed.
    if host.is_empty() || host.contains(':') || host.contains("..") {
        return None;
    }
    Some((scheme, host.to_ascii_lowercase(), port))
}

// --- AniList catalog --------------------------------------------------------

const ANILIST_ENDPOINT: &str = "https://graphql.anilist.co";
const ANILIST_POSTER_PREFIX: &str = "https://s4.anilist.co/file/anilistcdn/media/anime/cover/";
const ANILIST_QUERY: &str = r#"query ($page: Int, $search: String, $sort: [MediaSort]) {
  Page(page: $page, perPage: 40) {
    media(
      type: ANIME
      search: $search
      sort: $sort
      isAdult: false
      format_in: [TV, TV_SHORT, MOVIE, OVA, ONA, SPECIAL]
    ) {
      id
      isAdult
      format
      title { english romaji native }
      startDate { year }
      description(asHtml: false)
      coverImage { extraLarge large medium }
    }
  }
}"#;

/// Query one page of public AniList anime metadata. No authentication, caching
/// or library/watch-history data; only an optional title search is sent.
pub fn anilist_list(search: Option<&str>, fetch: &AniListFetchFn) -> Vec<CatalogItem> {
    let search = search.map(str::trim).filter(|value| !value.is_empty());
    if search.is_some_and(|value| value.chars().count() > 200) {
        return Vec::new();
    }
    let request = serde_json::json!({
        "query": ANILIST_QUERY,
        "variables": {
            "page": 1,
            "search": search,
            "sort": [if search.is_some() { "SEARCH_MATCH" } else { "POPULARITY_DESC" }],
        },
    })
    .to_string();
    let Ok(body) = fetch(ANILIST_ENDPOINT, &request) else {
        return Vec::new();
    };
    if body.len() > MAX_RESPONSE_BYTES {
        return Vec::new();
    }
    parse_anilist_catalog(&body)
}

fn parse_anilist_catalog(body: &[u8]) -> Vec<CatalogItem> {
    let Ok(value) = serde_json::from_slice::<Value>(body) else {
        return Vec::new();
    };
    let Some(media) = value
        .get("data")
        .and_then(|data| data.get("Page"))
        .and_then(|page| page.get("media"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for hit in media {
        if hit.get("isAdult").and_then(Value::as_bool) != Some(false) {
            continue;
        }
        let format = hit
            .get("format")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let kind = match format {
            "MOVIE" => "movie",
            "TV" | "TV_SHORT" | "OVA" | "ONA" | "SPECIAL" => "series",
            _ => continue,
        };
        let title = ["english", "romaji", "native"]
            .iter()
            .find_map(|field| hit.get("title")?.get(field)?.as_str())
            .and_then(|raw| sanitize_display(raw, 200));
        let Some(title) = title.filter(|title| !is_path_like(title)) else {
            continue;
        };
        let Some(id) = hit.get("id").and_then(Value::as_i64).filter(|id| *id > 0) else {
            continue;
        };
        let year = hit
            .get("startDate")
            .and_then(|date| date.get("year"))
            .and_then(Value::as_i64)
            .filter(|year| (1870..=2200).contains(year));
        let overview = hit
            .get("description")
            .and_then(Value::as_str)
            .and_then(|raw| sanitize_display(raw, 1000))
            .unwrap_or_default();
        let poster_url = ["extraLarge", "large", "medium"]
            .iter()
            .find_map(|field| hit.get("coverImage")?.get(field)?.as_str())
            .and_then(sanitize_anilist_poster);
        out.push(CatalogItem {
            id: format!("anilist-{id}"),
            title,
            year,
            kind: kind.to_string(),
            overview,
            poster_path: None,
            poster_url,
            provider: "anilist".to_string(),
        });
        if out.len() == MAX_CATALOG {
            break;
        }
    }
    out
}

fn sanitize_anilist_poster(raw: &str) -> Option<String> {
    let value = raw.trim();
    let path = value.strip_prefix(ANILIST_POSTER_PREFIX)?;
    if path.is_empty()
        || value.len() > 2048
        || path.contains("..")
        || path.contains("//")
        || path.chars().any(|ch| matches!(ch, '?' | '#' | '\\'))
        || path.chars().any(|ch| ch.is_control() || is_format_char(ch))
        || !path
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '_' | '-' | '.'))
    {
        return None;
    }
    Some(value.to_string())
}

/// One bounded POST to the public AniList GraphQL endpoint; redirects are
/// disabled and the response cap is enforced while streaming.
pub fn anilist_http_fetch(url: &str, body: &str) -> Result<Vec<u8>, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| error.to_string())?;
    let mut response = client
        .post(url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body.to_string())
        .send()
        .map_err(|error| error.to_string())?;
    if !response.status().is_success()
        || response
            .content_length()
            .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err("AniList request failed".to_string());
    }
    let mut bounded = response.by_ref().take(MAX_RESPONSE_BYTES as u64 + 1);
    let mut bytes = Vec::new();
    bounded
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err("AniList response too large".to_string());
    }
    Ok(bytes)
}

// --- TMDB catalog ----------------------------------------------------------

/// Fetch browse/search results from the opt-in TMDB v3 API. Blank key or an
/// unknown path means zero calls and an empty catalog. Only the query and key
/// leave the machine. Results are capped at [`MAX_CATALOG`].
pub fn tmdb_list(path: &str, query: &str, key: &str, fetch: &FetchFn) -> Vec<CatalogItem> {
    let key = key.trim();
    if key.is_empty() {
        return Vec::new();
    }
    let (path, default_kind) = match path {
        "movie/popular" => ("movie/popular", "movie"),
        "tv/popular" => ("tv/popular", "tv"),
        "search/multi" => ("search/multi", "movie"),
        _ => return Vec::new(),
    };
    let mut url = format!("https://api.themoviedb.org/3/{path}?api_key={}", pct(key));
    if path == "search/multi" {
        url.push_str("&query=");
        url.push_str(&pct(query.trim()));
    }

    let Ok(bytes) = fetch(&url, None) else {
        return Vec::new();
    };
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Vec::new();
    }
    parse_catalog(&bytes, default_kind)
}

fn parse_catalog(body: &[u8], default_kind: &str) -> Vec<CatalogItem> {
    let Ok(value) = serde_json::from_slice::<Value>(body) else {
        return Vec::new();
    };
    let Some(results) = value.get("results").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for hit in results {
        let media_type = hit
            .get("media_type")
            .and_then(Value::as_str)
            .unwrap_or(default_kind);
        let kind = match media_type {
            "movie" => "movie",
            "tv" => "series",
            _ => continue, // skip person and unknown media types
        };
        let Some(title) = hit
            .get("title")
            .or_else(|| hit.get("name"))
            .and_then(Value::as_str)
            .and_then(|raw| sanitize_display(raw, 200))
        else {
            continue;
        };
        if is_path_like(&title) {
            continue;
        }
        let Some(id) = hit.get("id").and_then(Value::as_i64) else {
            continue;
        };
        if id <= 0 {
            continue;
        }
        let year = hit
            .get("release_date")
            .or_else(|| hit.get("first_air_date"))
            .and_then(Value::as_str)
            .and_then(|date| date.get(0..4))
            .and_then(|part| part.parse::<i64>().ok())
            .filter(|year| (1870..=2200).contains(year));
        let overview = hit
            .get("overview")
            .and_then(Value::as_str)
            .and_then(|raw| sanitize_display(raw, 1000))
            .unwrap_or_default();
        let poster_path = hit
            .get("poster_path")
            .and_then(Value::as_str)
            .and_then(sanitize_poster);
        out.push(CatalogItem {
            id: id.to_string(),
            title,
            year,
            kind: kind.to_string(),
            overview,
            poster_path,
            poster_url: None,
            provider: "tmdb".to_string(),
        });
        if out.len() == MAX_CATALOG {
            break;
        }
    }
    out
}

// --- Sanitisation and bounded parsing --------------------------------------

/// Percent-encode a query component. Keeps only unreserved bytes verbatim.
fn pct(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for &byte in raw.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push_str(&format!("{byte:02X}"));
        }
    }
    out
}

/// Decode the XML entities that can legally appear in RSS display fields.
fn decode_entities(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '&' {
            out.push(ch);
            continue;
        }
        let mut entity = String::new();
        while let Some(&next) = chars.peek() {
            if next == ';' {
                chars.next();
                break;
            }
            if entity.len() >= 12 {
                break;
            }
            entity.push(next);
            chars.next();
        }
        match entity.as_str() {
            "amp" => out.push('&'),
            "lt" => out.push('<'),
            "gt" => out.push('>'),
            "quot" => out.push('"'),
            "apos" => out.push('\''),
            _ if entity.starts_with("#x") || entity.starts_with("#X") => {
                if let Ok(code) = u32::from_str_radix(&entity[2..], 16) {
                    if let Some(decoded) = char::from_u32(code) {
                        out.push(decoded);
                    }
                }
            }
            _ if entity.starts_with('#') => {
                if let Ok(code) = entity[1..].parse::<u32>() {
                    if let Some(decoded) = char::from_u32(code) {
                        out.push(decoded);
                    }
                }
            }
            _ => {
                out.push('&');
                out.push_str(&entity);
                out.push(';');
            }
        }
    }
    out
}

/// Collapse whitespace, drop control characters, cap length. Blank -> `None`.
fn sanitize_display(raw: &str, max_chars: usize) -> Option<String> {
    let decoded = decode_entities(raw);
    let joined = decoded
        .chars()
        .filter(|ch| !ch.is_control() && !is_format_char(*ch))
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if joined.is_empty() {
        return None;
    }
    Some(joined.chars().take(max_chars).collect())
}

/// Magnets keep their `&` separators (whitespace must NOT be collapsed), but
/// still lose control characters and are length-bounded. Must be a magnet URI.
fn sanitize_magnet(raw: &str) -> Option<String> {
    let decoded = decode_entities(raw);
    let cleaned: String = decoded.chars().filter(|ch| !ch.is_control()).collect();
    let trimmed = cleaned.trim();
    if !trimmed.starts_with("magnet:?") || trimmed.len() > MAX_URI_LEN {
        return None;
    }
    Some(trimmed.to_string())
}

/// TMDB/CDN poster paths are relative (`/abc.jpg`). Accept only that shape so a
/// hostile payload cannot become a local path, URL or script.
fn sanitize_poster(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.is_empty()
        || value.len() > 512
        || !value.starts_with('/')
        || value.starts_with("//")
        || value.contains("..")
        || value.contains('\\')
        || value
            .chars()
            .any(|ch| ch.is_control() || is_format_char(ch))
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '_' | '-' | '.'))
    {
        return None;
    }
    Some(value.to_string())
}

/// Bidirectional/format controls can visually reorder displayed text without
/// leaving a printable trace; reject them rather than render or store them.
fn is_format_char(ch: char) -> bool {
    matches!(
        ch as u32,
        0x202A..=0x202E | 0x2066..=0x2069 | 0x200B..=0x200D | 0xFEFF
    )
}

/// Reject separators and Windows drive prefixes as hostile data. A bare `..`
/// is not a path (and breaks legitimate titles such as "The Movie ... Final
/// Cut"); traversal sequences always carry a separator, which is already
/// rejected here.
fn is_path_like(value: &str) -> bool {
    let trimmed = value.trim();
    let bytes = trimmed.as_bytes();
    trimmed.contains('/')
        || trimmed.contains('\\')
        || (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
}

fn detect_resolution(title: &str) -> String {
    let lower = title.to_ascii_lowercase();
    for token in ["2160p", "4k", "1080p", "720p", "576p", "480p"] {
        if lower.contains(token) {
            return token.to_string();
        }
    }
    String::new()
}

fn resolution_rank(resolution: &str) -> i64 {
    match resolution {
        "2160p" | "4k" => 2160,
        "1080p" => 1080,
        "720p" => 720,
        "576p" => 576,
        "480p" => 480,
        _ => 0,
    }
}

/// Bounded `<item>...</item>` body scanner.
fn rss_items(xml: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<item") {
        let Some(open_end) = rest[start..].find('>') else {
            break;
        };
        let body_start = start + open_end + 1;
        let Some(close_len) = rest[body_start..].find("</item>") else {
            break;
        };
        out.push(&rest[body_start..body_start + close_len]);
        rest = &rest[body_start + close_len + "</item>".len()..];
    }
    out
}

/// Text of the first `<name>...</name>` tag in a block.
fn tag_text(block: &str, name: &str) -> Option<String> {
    let open = format!("<{name}>");
    let start = block.find(&open)? + open.len();
    let close = format!("</{name}>");
    let end = block[start..].find(&close)? + start;
    let text = &block[start..end];
    let text = text.strip_prefix("<![CDATA[").unwrap_or(text);
    let text = text.strip_suffix("]]>").unwrap_or(text);
    Some(text.to_string())
}

/// All `name="..."` / `value="..."` pairs in a block's tags (Torznab attrs).
fn torznab_attrs(block: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = block;
    while let Some(start) = rest.find('<') {
        let Some(end) = rest[start..].find('>') else {
            break;
        };
        let tag = &rest[start + 1..start + end];
        if let (Some(name), Some(value)) = (tag_attr(tag, "name"), tag_attr(tag, "value")) {
            // Keep attrs raw: the downstream sanitiser decodes exactly once,
            // so `&amp;lt;` cannot become a real `<` after a second decode.
            out.push((name, value));
        }
        rest = &rest[start + end + 1..];
    }
    out
}

fn tag_attr(tag: &str, key: &str) -> Option<String> {
    let needle = format!("{key}=\"");
    let start = tag.find(&needle)? + needle.len();
    let end = tag[start..].find('"')? + start;
    Some(tag[start..end].to_string())
}

/// Prefer a Torznab `attr`, else a plain tag, for a named value.
fn attr_value(block: &str, attrs: &[(String, String)], key: &str) -> Option<String> {
    attrs
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(key))
        .map(|(_, value)| value.clone())
        .or_else(|| tag_text(block, key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn fake(body: Vec<u8>) -> Box<FetchFn> {
        Box::new(move |_url: &str, _key: Option<&str>| Ok(body.clone()))
    }

    fn counting(body: &'static str) -> (Box<FetchFn>, std::sync::Arc<AtomicUsize>) {
        let calls = std::sync::Arc::new(AtomicUsize::new(0));
        let record = calls.clone();
        (
            Box::new(move |_url: &str, _key: Option<&str>| {
                record.fetch_add(1, Ordering::SeqCst);
                Ok(body.as_bytes().to_vec())
            }),
            calls,
        )
    }

    fn item(title: &str, size: &str, seeders: &str, magnet: &str) -> String {
        format!(
            "<item><title>{title}</title><size>{size}</size>\
             <seeders>{seeders}</seeders><link>{magnet}</link>\
             <source>idx-&amp;co</source></item>"
        )
    }

    fn rss(items: &[String]) -> Vec<u8> {
        format!("<rss><channel>{}</channel></rss>", items.concat()).into_bytes()
    }

    #[test]
    fn torznab_ranks_seeders_then_size_then_resolution_then_title() {
        let xml = rss(&[
            item("Old 720p", "100", "2", "magnet:?xt=urn:btih:a"),
            item("Big 1080p", "900", "5", "magnet:?xt=urn:btih:b"),
            item("Small 1080p", "100", "5", "magnet:?xt=urn:btih:c"),
            item("A 1080p", "100", "5", "magnet:?xt=urn:btih:d"),
        ]);
        let items = torznab_search("http://idx", "key", "Title", Some(2020), &fake(xml)).unwrap();
        let titles: Vec<_> = items.iter().map(|item| item.title.as_str()).collect();
        assert_eq!(
            titles,
            vec!["Big 1080p", "A 1080p", "Small 1080p", "Old 720p"]
        );
        assert_eq!(items[0].seeders, 5);
        assert_eq!(items[0].size_bytes, 900);
        assert_eq!(items[0].resolution, "1080p");
        assert_eq!(items[0].source, "idx-&co");
        assert!(items[0].magnet.starts_with("magnet:?xt=urn:btih:"));
    }

    #[test]
    fn torznab_sanitises_hostile_data_and_skips_path_titles() {
        let xml = rss(&[
            // control chars + entity-encoded title; kept but cleaned
            item(
                "Clean\u{0001} &amp; Safe",
                "10",
                "3",
                "magnet:?xt=urn:btih:ok",
            ),
            // path-like title: rejected
            item("C:\\evil\\movie", "10", "9", "magnet:?xt=urn:btih:bad"),
            item("../../etc/passwd", "10", "9", "magnet:?xt=urn:btih:bad2"),
            // non-magnet link: rejected
            item("Not a magnet", "10", "9", "http://evil/file.torrent"),
            // oversized display field is capped, not passed through
            item(&"X".repeat(400), "10", "1", "magnet:?xt=urn:btih:cap"),
        ]);
        let items = torznab_search("http://idx", "key", "Title", None, &fake(xml)).unwrap();
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|item| !item.title.contains('\u{0001}')));
        assert_eq!(items[0].title, "Clean & Safe");
        assert!(items.iter().any(|item| item.title.chars().count() <= 200));
    }

    #[test]
    fn torznab_caps_response_and_shortlist() {
        let huge = vec![b'x'; MAX_RESPONSE_BYTES + 1];
        assert!(matches!(
            torznab_search("http://idx", "key", "Title", None, &fake(huge)),
            Err(DiscoveryError::ResponseTooLarge)
        ));

        let many: Vec<String> = (0..25)
            .map(|index| {
                item(
                    &format!("Item {index:02}"),
                    "10",
                    "1",
                    "magnet:?xt=urn:btih:x",
                )
            })
            .collect();
        let items = torznab_search("http://idx", "key", "Title", None, &fake(rss(&many))).unwrap();
        assert_eq!(items.len(), MAX_SHORTLIST);
    }

    #[test]
    fn torznab_missing_config_makes_zero_calls() {
        let (fetch, calls) = counting("<rss></rss>");
        assert!(torznab_search("", "key", "Title", None, &fetch)
            .unwrap()
            .is_empty());
        assert!(torznab_search("http://idx", "", "Title", None, &fetch)
            .unwrap()
            .is_empty());
        assert!(torznab_search("http://idx", "key", "", None, &fetch)
            .unwrap()
            .is_empty());
        assert!(torznab_search("http://idx", "key", "../evil", None, &fetch)
            .unwrap()
            .is_empty());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn tmdb_browse_and_search_parse_sanitise_and_force_kind() {
        let browse = br#"{"results":[
            {"id":1,"title":"Movie &amp; One","release_date":"2021-05-01","overview":"a\u0001b","poster_path":"/p.jpg","media_type":"movie"},
            {"id":2,"name":"Show","first_air_date":"2019-01-01","overview":"ok","poster_path":"http://evil/x","media_type":"tv"},
            {"id":3,"name":"Person","media_type":"person"}
        ]}"#;
        let items = tmdb_list("movie/popular", "", "key", &fake(browse.to_vec()));
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].kind, "movie");
        assert_eq!(items[0].title, "Movie & One");
        assert_eq!(items[0].year, Some(2021));
        assert_eq!(items[0].overview, "ab");
        assert_eq!(items[0].poster_path.as_deref(), Some("/p.jpg"));
        assert_eq!(items[1].kind, "series");
        assert_eq!(items[1].poster_path, None);

        let search = br#"{"results":[{"id":9,"name":"../evil","media_type":"tv"}]}"#;
        assert!(tmdb_list("search/multi", "q", "key", &fake(search.to_vec())).is_empty());
    }

    #[test]
    fn anilist_browse_and_search_map_anime_without_trusting_remote_fields() {
        let response = br#"{"data":{"Page":{"media":[
            {"id":21,"isAdult":false,"format":"MOVIE","title":{"english":"Film","romaji":"Eiga"},"startDate":{"year":2004},"description":"A story.","coverImage":{"extraLarge":"https://s4.anilist.co/file/anilistcdn/media/anime/cover/large/21.jpg"}},
            {"id":22,"isAdult":false,"format":"TV","title":{"english":null,"romaji":"Series"},"startDate":{"year":2020},"description":"Show.","coverImage":{"large":"https://evil.example/poster.jpg"}},
            {"id":23,"isAdult":true,"format":"TV","title":{"english":"Adult"},"startDate":{"year":2020}}
        ]}}}"#;
        let seen = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
        let capture = seen.clone();
        let fetch: Box<AniListFetchFn> = Box::new(move |url, body| {
            *capture.lock().unwrap() = Some(body.to_string());
            assert_eq!(url, "https://graphql.anilist.co");
            Ok(response.to_vec())
        });

        let items = anilist_list(Some("Film"), &fetch);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, "anilist-21");
        assert_eq!(items[0].title, "Film");
        assert_eq!(items[0].year, Some(2004));
        assert_eq!(items[0].kind, "movie");
        assert_eq!(items[0].provider, "anilist");
        assert_eq!(
            items[0].poster_url.as_deref(),
            Some("https://s4.anilist.co/file/anilistcdn/media/anime/cover/large/21.jpg")
        );
        assert_eq!(items[1].title, "Series");
        assert_eq!(items[1].kind, "series");
        assert_eq!(items[1].poster_url, None);

        let request: Value =
            serde_json::from_str(seen.lock().unwrap().as_deref().unwrap()).unwrap();
        assert!(request["query"].as_str().unwrap().contains("type: ANIME"));
        assert!(request["query"]
            .as_str()
            .unwrap()
            .contains("isAdult: false"));
        assert_eq!(request["variables"]["search"], "Film");
        assert_eq!(request["variables"]["page"], 1);
    }

    #[test]
    fn anilist_browse_sorts_popular_and_caps_catalog_results() {
        let captured = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
        let capture = captured.clone();
        let fetch: Box<AniListFetchFn> = Box::new(move |_, body| {
            *capture.lock().unwrap() = Some(body.to_string());
            let media: Vec<String> = (1..=MAX_CATALOG + 5)
                .map(|id| {
                    format!(
                        r#"{{"id":{id},"isAdult":false,"format":"TV","title":{{"romaji":"Show {id}"}}}}"#
                    )
                })
                .collect();
            Ok(format!(
                "{{\"data\":{{\"Page\":{{\"media\":[{}]}}}}}}",
                media.join(",")
            )
            .into_bytes())
        });

        let items = anilist_list(None, &fetch);
        assert_eq!(items.len(), MAX_CATALOG);
        let request: Value =
            serde_json::from_str(captured.lock().unwrap().as_deref().unwrap()).unwrap();
        assert_eq!(request["variables"]["search"], Value::Null);
        assert_eq!(request["variables"]["sort"][0], "POPULARITY_DESC");
    }

    #[test]
    fn anilist_caps_responses_and_skips_oversized_searches() {
        let huge = vec![b'x'; MAX_RESPONSE_BYTES + 1];
        let fetch: Box<AniListFetchFn> = Box::new(move |_, _| Ok(huge.clone()));
        assert!(anilist_list(None, &fetch).is_empty());

        let calls = std::sync::Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        let wrapped: Box<AniListFetchFn> = Box::new(move |_, _| {
            counter.fetch_add(1, Ordering::SeqCst);
            Ok(br#"{"data":{"Page":{"media":[]}}}"#.to_vec())
        });
        assert!(anilist_list(Some("  \n"), &wrapped).is_empty());
        assert_eq!(calls.load(Ordering::SeqCst), 1, "blank search is browse");
        assert!(anilist_list(Some(&"x".repeat(201)), &wrapped).is_empty());
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "oversized query is rejected"
        );
    }

    #[test]
    fn tmdb_caps_response_and_catalog_and_checks_config() {
        let huge = vec![b'x'; MAX_RESPONSE_BYTES + 1];
        assert!(tmdb_list("movie/popular", "", "key", &fake(huge)).is_empty());

        let many: Vec<String> = (0..45)
            .map(|index| format!(r#"{{"id":{index},"title":"T{index}","media_type":"movie"}}"#))
            .collect();
        let body = format!("{{\"results\":[{}]}}", many.join(","));
        let items = tmdb_list("movie/popular", "", "key", &fake(body.into_bytes()));
        assert_eq!(items.len(), MAX_CATALOG);

        let (fetch, calls) = counting(r#"{"results":[]}"#);
        assert!(tmdb_list("movie/popular", "", "", &fetch).is_empty());
        assert!(tmdb_list("unknown/path", "", "key", &fetch).is_empty());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn url_carries_only_query_and_key() {
        let (fetch, _calls) = counting("<rss></rss>");
        let seen = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let record = seen.clone();
        let capture: Box<FetchFn> = Box::new(move |url: &str, key: Option<&str>| {
            *record.lock().unwrap() = url.to_string();
            fetch(url, key)
        });
        let _ = torznab_search("http://idx/api", "s3cr3t", "My Movie", Some(2020), &capture);
        let url = seen.lock().unwrap().clone();
        assert!(url.starts_with("http://idx/api?t=search&q=My%20Movie%202020&apikey=s3cr3t"));
        assert!(!url.contains("C:\\"), "no path may leave the machine");
    }

    // --- Round 1 review fixes ----------------------------------------------

    #[test]
    fn poster_rejects_protocol_relative_and_format_chars() {
        assert_eq!(sanitize_poster("//evil.com/x"), None);
        assert_eq!(sanitize_poster("/ok\u{202E}.jpg"), None);
        assert_eq!(sanitize_poster("/ok.jpg").as_deref(), Some("/ok.jpg"));
    }

    #[test]
    fn magnet_cap_accepts_real_world_lengths_and_rejects_oversized() {
        let hash = "a".repeat(2500 - "magnet:?xt=urn:btih:".len());
        let magnet = format!("magnet:?xt=urn:btih:{hash}");
        assert_eq!(magnet.len(), 2500);
        assert_eq!(sanitize_magnet(&magnet).as_deref(), Some(magnet.as_str()));

        let oversized = format!("magnet:?xt=urn:btih:{}", "a".repeat(MAX_URI_LEN));
        assert_eq!(sanitize_magnet(&oversized), None);
    }

    #[test]
    fn tag_text_unwraps_cdata() {
        let xml = rss(&[item(
            "<![CDATA[Clean &amp; Safe]]>",
            "10",
            "1",
            "magnet:?xt=urn:btih:cdata",
        )]);
        let items = torznab_search("http://idx", "key", "Title", None, &fake(xml)).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Clean & Safe");
    }

    #[test]
    fn dots_in_titles_are_not_paths() {
        let xml = rss(&[item(
            "The Movie ... Final Cut",
            "10",
            "1",
            "magnet:?xt=urn:btih:dots",
        )]);
        let items = torznab_search("http://idx", "key", "Title", None, &fake(xml)).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "The Movie ... Final Cut");
    }

    #[test]
    fn format_chars_never_reach_the_renderer() {
        let xml = rss(&[item(
            "Bad\u{202E}\u{200B}Title",
            "10",
            "1",
            "magnet:?xt=urn:btih:fmt",
        )]);
        let items = torznab_search("http://idx", "key", "Title", None, &fake(xml)).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "BadTitle");
        assert!(!items[0].title.contains('\u{202E}'));
        assert!(!items[0].title.contains('\u{200B}'));
    }

    #[test]
    fn entities_decode_once_so_markup_cannot_reach_the_renderer() {
        // A double decode would turn `&amp;lt;` into a real `<`.
        let xml = rss(&[item(
            "A &amp;lt;script&amp;gt; B",
            "10",
            "1",
            "magnet:?xt=urn:btih:once",
        )]);
        let items = torznab_search("http://idx", "key", "Title", None, &fake(xml)).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "A &lt;script&gt; B");
        assert!(!items[0].title.contains('<'));
        assert!(!items[0].title.contains('>'));
        assert_eq!(items[0].source, "idx-&co");
    }

    #[test]
    fn endpoint_with_existing_query_uses_ampersand() {
        let seen = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let record = seen.clone();
        let capture: Box<FetchFn> = Box::new(move |url: &str, _key: Option<&str>| {
            *record.lock().unwrap() = url.to_string();
            Ok("<rss></rss>".as_bytes().to_vec())
        });
        let _ = torznab_search("http://idx/api?t=foo", "key", "Title", None, &capture);
        assert!(seen
            .lock()
            .unwrap()
            .starts_with("http://idx/api?t=foo&t=search&q=Title&apikey=key"));
    }

    // --- Prowlarr v3 JSON search ------------------------------------------

    fn row(title: &str, size: &str, seeders: &str, magnet: &str, indexer: &str) -> String {
        format!(
            r#"{{"title":"{title}","magnetUrl":"{magnet}","size":{size},"seeders":{seeders},"indexer":"{indexer}"}}"#
        )
    }

    #[test]
    fn prowlarr_search_maps_fields_and_ranks_like_torznab() {
        let body = format!(
            "[{}]",
            [
                row("Old 720p", "\"100\"", "2", "magnet:?xt=urn:btih:a", "idx-a"),
                row("Big 1080p", "900", "5", "magnet:?xt=urn:btih:b", "idx-b"),
                row("Small 1080p", "100", "5", "magnet:?xt=urn:btih:c", "idx-c"),
            ]
            .join(",")
        );
        let items = prowlarr_search(
            "http://prowl:9117",
            "key",
            "Title",
            None,
            &fake(body.into_bytes()),
        )
        .unwrap();
        let titles: Vec<_> = items.iter().map(|item| item.title.as_str()).collect();
        assert_eq!(titles, vec!["Big 1080p", "Small 1080p", "Old 720p"]);
        assert_eq!(items[0].seeders, 5);
        assert_eq!(items[0].size_bytes, 900);
        assert_eq!(items[0].resolution, "1080p");
        assert_eq!(items[0].source, "idx-b");
        assert!(items[0].magnet.starts_with("magnet:?xt=urn:btih:"));
        assert_eq!(items[2].resolution, "720p");
        assert_eq!(items[2].size_bytes, 100); // numeric string parsed
    }

    #[test]
    fn prowlarr_search_rejects_hostile_rows_and_uses_exact_resolution() {
        let body = r#"[
            {"title":"Clean\u0001 &amp; Safe 480p","magnetUrl":"magnet:?xt=urn:btih:ok","indexer":"idx"},
            {"title":"../evil","magnetUrl":"magnet:?xt=urn:btih:path","indexer":"idx"},
            {"title":"C:\\evil","magnetUrl":"magnet:?xt=urn:btih:drive","indexer":"idx"},
            {"title":"Not a magnet","magnetUrl":"http://evil/x","indexer":"idx"},
            {"title":"Missing magnet"},
            {"title":"UPPER 1080P","magnetUrl":"magnet:?xt=urn:btih:upper","indexer":"idx"}
        ]"#;
        let items = prowlarr_search(
            "http://prowl",
            "key",
            "Title",
            None,
            &fake(body.as_bytes().to_vec()),
        )
        .unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Clean & Safe 480p");
        assert_eq!(items[0].resolution, "480p");
        assert_eq!(items[1].title, "UPPER 1080P");
        assert_eq!(items[1].resolution, "");
        assert!(!items[0].title.contains('\u{0001}'));
    }

    #[test]
    fn prowlarr_search_enforces_the_byte_cap() {
        let huge = vec![b'x'; MAX_PROWLARR_BYTES + 1];
        assert!(matches!(
            prowlarr_search("http://prowl", "key", "Title", None, &fake(huge)),
            Err(DiscoveryError::ResponseTooLarge)
        ));
    }

    #[test]
    fn prowlarr_search_malformed_json_is_empty() {
        for body in ["not json", r#"{"results":[]}"#, "[]"] {
            assert!(prowlarr_search(
                "http://prowl",
                "key",
                "Title",
                None,
                &fake(body.as_bytes().to_vec())
            )
            .unwrap()
            .is_empty());
        }
    }

    #[test]
    fn prowlarr_search_blank_config_makes_zero_calls() {
        let (fetch, calls) = counting("[]");
        assert!(prowlarr_search("", "key", "Title", None, &fetch)
            .unwrap()
            .is_empty());
        assert!(prowlarr_search("http://prowl", "", "Title", None, &fetch)
            .unwrap()
            .is_empty());
        assert!(prowlarr_search("http://prowl", "key", "", None, &fetch)
            .unwrap()
            .is_empty());
        assert!(
            prowlarr_search("http://prowl", "key", "../evil", None, &fetch)
                .unwrap()
                .is_empty()
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn prowlarr_search_sends_key_as_header_not_in_the_url() {
        let (fetch, seen) = keyed_json_fetch("[]".to_string());
        let _ = prowlarr_search("http://prowl:9117/", "k/ey", "My Movie", Some(2020), &fetch);
        assert_eq!(
            seen.lock().unwrap().as_slice(),
            &[(
                "http://prowl:9117/api/v1/search?query=My%20Movie%202020".to_string(),
                Some("k/ey".to_string())
            )]
        );

        let (fetch, seen) = keyed_json_fetch("[]".to_string());
        let _ = prowlarr_search("http://prowl/base?x=1", "key", "T & Y", None, &fetch);
        assert_eq!(
            seen.lock().unwrap().as_slice(),
            &[(
                "http://prowl/base?x=1/api/v1/search&query=T%20%26%20Y".to_string(),
                Some("key".to_string())
            )]
        );
        assert!(seen
            .lock()
            .unwrap()
            .iter()
            .all(|(url, _)| !url.contains("apikey=")));
    }

    #[test]
    fn prowlarr_search_caps_at_twenty_and_hides_fetch_errors() {
        let rows: Vec<String> = (0..25)
            .map(|index| {
                row(
                    &format!("Item {index:02}"),
                    "10",
                    "1",
                    "magnet:?xt=urn:btih:x",
                    "idx",
                )
            })
            .collect();
        let body = format!("[{}]", rows.join(","));
        let items = prowlarr_search(
            "http://prowl",
            "key",
            "Title",
            None,
            &fake(body.into_bytes()),
        )
        .unwrap();
        assert_eq!(items.len(), MAX_SHORTLIST);

        let fetch: Box<FetchFn> = Box::new(|_, _| Err("boom".to_string()));
        assert!(matches!(
            prowlarr_search("http://prowl", "key", "Title", None, &fetch),
            Err(DiscoveryError::Fetch)
        ));
    }

    // --- Prowlarr endpoint discovery --------------------------------------

    fn json_fetch(body: String) -> (Box<FetchFn>, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let record = seen.clone();
        (
            Box::new(move |url: &str, _key: Option<&str>| {
                record.lock().unwrap().push(url.to_string());
                Ok(body.as_bytes().to_vec())
            }),
            seen,
        )
    }

    /// Captures both the URL and the header key so tests can prove auth left
    /// the query string and now travels via the fetch seam.
    type KeyedSeen = std::sync::Arc<std::sync::Mutex<Vec<(String, Option<String>)>>>;

    fn keyed_json_fetch(body: String) -> (Box<FetchFn>, KeyedSeen) {
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let record = seen.clone();
        (
            Box::new(move |url: &str, key: Option<&str>| {
                record
                    .lock()
                    .unwrap()
                    .push((url.to_string(), key.map(str::to_string)));
                Ok(body.as_bytes().to_vec())
            }),
            seen,
        )
    }

    #[test]
    fn prowlarr_filters_enabled_and_prefers_torznab_url() {
        let body = r#"[
            {"id":1,"enable":true,"torznabUrl":"  http://prowl:9117/idx-one/api?apikey=leak  "},
            {"id":2,"enable":false,"torznabUrl":"http://disabled/api"},
            {"id":3,"enable":true,"torznabUrl":""},
            {"id":4,"enable":true},
            {"id":5,"enable":true,"torznabUrl":null},
            {"id":6,"enable":"true","torznabUrl":"http://string-enable/api"}
        ]"#;
        let (fetch, seen) = json_fetch(body.to_string());
        let endpoints = prowlarr_torznab_endpoints("http://prowl:9117/", "k/ey", &fetch).unwrap();
        assert_eq!(
            endpoints,
            vec![
                "http://prowl:9117/idx-one/api".to_string(),
                "http://prowl:9117/api/v1/indexer/3/torznab".to_string(),
                "http://prowl:9117/api/v1/indexer/4/torznab".to_string(),
                "http://prowl:9117/api/v1/indexer/5/torznab".to_string(),
            ]
        );
        assert_eq!(
            seen.lock().unwrap().as_slice(),
            &["http://prowl:9117/api/v1/indexer?apikey=k%2Fey".to_string()]
        );
    }

    #[test]
    fn prowlarr_rejects_hostile_endpoints_and_caps_at_ten() {
        let mut entries: Vec<String> = vec![
            r#"{"id":1,"enable":true,"torznabUrl":"ftp://evil/x"}"#,
            r#"{"id":2,"enable":true,"torznabUrl":"/relative/path"}"#,
            r#"{"id":3,"enable":true,"torznabUrl":"C:\\evil\\x"}"#,
            r#"{"id":4,"enable":true,"torznabUrl":"http://evil/../x"}"#,
            r#"{"id":5,"enable":true,"torznabUrl":"http://user@evil/x"}"#,
            r#"{"id":6,"enable":true,"torznabUrl":"http://evil/x#frag"}"#,
            r#"{"id":7,"enable":true,"torznabUrl":"http://evil/x\u0001"}"#,
            r#"{"id":8,"enable":true,"torznabUrl":"javascript:alert(1)"}"#,
            r#"{"id":9,"enable":true,"torznabUrl":"http://attacker.example/x"}"#,
            r#"{"id":10,"enable":true,"torznabUrl":"http://prowl:9118/x"}"#,
        ]
        .into_iter()
        .map(str::to_string)
        .collect();
        entries.extend((11..27).map(|id| {
            format!(r#"{{"id":{id},"enable":true,"torznabUrl":"http://prowl/idx{id}/api"}}"#)
        }));
        let (fetch, _) = json_fetch(format!("[{}]", entries.join(",")));
        let endpoints = prowlarr_torznab_endpoints("http://prowl", "key", &fetch).unwrap();
        assert_eq!(endpoints.len(), MAX_PROWLARR_ENDPOINTS);
        assert_eq!(endpoints[0], "http://prowl/idx11/api");
        assert!(endpoints
            .iter()
            .all(|url| url.starts_with("http://prowl/idx")));

        let (fetch, _) = json_fetch(format!("[{}]", entries[..10].join(",")));
        assert!(prowlarr_torznab_endpoints("http://prowl", "key", &fetch)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn prowlarr_endpoints_must_match_the_configured_origin() {
        let body = r#"[
            {"id":1,"enable":true,"torznabUrl":"http://127.0.0.1:9117/ok"},
            {"id":2,"enable":true,"torznabUrl":"http://attacker.example/x"},
            {"id":3,"enable":true,"torznabUrl":"http://127.0.0.1:9118/x"}
        ]"#;
        let (fetch, _) = json_fetch(body.to_string());
        let endpoints = prowlarr_torznab_endpoints("http://127.0.0.1:9117", "key", &fetch).unwrap();
        assert_eq!(endpoints, vec!["http://127.0.0.1:9117/ok".to_string()]);

        // Host and scheme are case-insensitive; an omitted port means 80/443.
        let body = r#"[
            {"id":1,"enable":true,"torznabUrl":"HTTP://PROWL:80/x"},
            {"id":2,"enable":true,"torznabUrl":"https://prowl/x"}
        ]"#;
        let (fetch, _) = json_fetch(body.to_string());
        let endpoints = prowlarr_torznab_endpoints("http://PROWL", "key", &fetch).unwrap();
        assert_eq!(endpoints, vec!["HTTP://PROWL:80/x".to_string()]);
    }

    #[test]
    fn prowlarr_blank_config_makes_zero_calls_and_bad_json_is_empty() {
        let (fetch, calls) = counting("[]");
        assert!(prowlarr_torznab_endpoints("", "key", &fetch)
            .unwrap()
            .is_empty());
        assert!(prowlarr_torznab_endpoints("http://prowl", "  ", &fetch)
            .unwrap()
            .is_empty());
        assert!(prowlarr_torznab_endpoints("not a url", "key", &fetch)
            .unwrap()
            .is_empty());
        assert_eq!(calls.load(Ordering::SeqCst), 0);

        let (fetch, _) = json_fetch("not json".to_string());
        assert!(prowlarr_torznab_endpoints("http://prowl", "key", &fetch)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn torznab_multi_merges_ranks_and_truncates() {
        let fetch: Box<FetchFn> = Box::new(|url: &str, _key: Option<&str>| {
            let (tag, size) = if url.contains("idx-one") {
                ("one", "10")
            } else {
                ("two", "20")
            };
            let items: Vec<String> = (1..=12)
                .map(|seeders| {
                    item(
                        &format!("{tag}-{seeders:02}"),
                        size,
                        &seeders.to_string(),
                        &format!("magnet:?xt=urn:btih:{tag}{seeders}"),
                    )
                })
                .collect();
            Ok(rss(&items))
        });
        let endpoints = vec![
            "http://idx-one/api".to_string(),
            "http://idx-two/api".to_string(),
        ];
        let items = torznab_search_multi(&endpoints, "key", "Title", None, &fetch).unwrap();
        assert_eq!(items.len(), MAX_SHORTLIST);
        assert_eq!(items[0].title, "two-12");
        assert_eq!(items[0].seeders, 12);
        assert_eq!(items[0].size_bytes, 20);
        assert_eq!(items.last().unwrap().title, "one-03");

        let single = torznab_search("http://idx-one/api", "key", "Title", None, &fetch).unwrap();
        let multi_one = torznab_search_multi(
            &["http://idx-one/api".to_string()],
            "key",
            "Title",
            None,
            &fetch,
        )
        .unwrap();
        assert_eq!(single, multi_one);
    }
}
