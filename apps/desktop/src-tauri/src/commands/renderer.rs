//! Signed host-side forwarder plus the 24 named renderer commands.
//!
//! The renderer invokes a named command with the request fields; this module
//! builds the closed JSON body the API expects, signs it with the per-install
//! token and forwards it over a loopback TCP connection. Only the backend's
//! JSON body is returned; on failure the renderer gets a legible code from the
//! closed `AppErrorCode` set, never raw backend text.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use app_core::domain::AppErrorCode;
use media_acquisition::{defender_scan, inspect_with_probe};
use mediahaven_contract::{sign as sign_request, COMMANDS};
use serde::Serialize;
use serde_json::{json, Value};
use tauri::Manager;

use crate::player::{resolve_for_media, spawn_status_poller, PlayerState};
use crate::secret::{load_or_create_token, TOKEN_ACCOUNT, TOKEN_SERVICE};
use crate::{render_mounts, RootSpec};

/// The loopback API published by the supervisor. Matches `core`'s default bind.
const API_ADDR: &str = "127.0.0.1:8080";

/// Error returned to the renderer. Fixed messages only: backend text never
/// crosses this boundary.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: AppErrorCode,
    pub message: &'static str,
    pub retryable: bool,
}

impl CommandError {
    fn new(code: AppErrorCode) -> Self {
        let (message, retryable) = match code {
            AppErrorCode::RuntimeUnavailable => ("The backend runtime is unavailable.", true),
            AppErrorCode::RuntimeUnhealthy => ("The backend runtime is unhealthy.", true),
            AppErrorCode::BackendUnreachable => ("The backend could not be reached.", true),
            AppErrorCode::CredentialMismatch => ("The install credentials were rejected.", false),
            AppErrorCode::LibraryRootOffline => ("The library root is offline.", true),
            AppErrorCode::LibraryRootUnreadable => ("The library root cannot be read.", false),
            AppErrorCode::MediaUnsupported => ("The media type is not supported.", false),
            AppErrorCode::MediaMissing => ("The media file is missing.", false),
            AppErrorCode::StorageFull => ("Storage is full.", false),
            AppErrorCode::NotFound => ("The requested item was not found.", false),
            AppErrorCode::InvalidRequest => ("The request was rejected.", false),
            AppErrorCode::Internal => ("The backend request failed.", true),
        };
        Self {
            code,
            message,
            retryable,
        }
    }

    /// ADR-0015 fail-closed state. Fixed, legible message: no backend text,
    /// path or token reaches the renderer.
    pub fn source_unavailable() -> Self {
        Self {
            code: AppErrorCode::MediaMissing,
            message: "source unavailable: the media file is no longer under a declared root",
            retryable: false,
        }
    }

    /// Distinct from resolution failure: the source is fine but mpv could not
    /// be spawned (e.g. `MEDIAHAVEN_MPV` is unset and mpv is not on PATH).
    pub fn engine_unavailable() -> Self {
        Self {
            code: AppErrorCode::RuntimeUnavailable,
            message: "The playback engine could not be started.",
            retryable: true,
        }
    }

    /// No child video surface was attached: mpv must not open its own window.
    pub fn surface_unavailable() -> Self {
        Self {
            code: AppErrorCode::RuntimeUnavailable,
            message: "player surface unavailable",
            retryable: true,
        }
    }
}

/// The backend's `AppError` wire shape; only the code is consumed.
#[derive(serde::Deserialize)]
struct BackendCode {
    code: AppErrorCode,
}

/// Map an HTTP status to the closed error set as a fallback when the backend
/// body cannot be parsed. Backend response text is intentionally ignored.
fn status_code(status: u16) -> AppErrorCode {
    match status {
        400 => AppErrorCode::InvalidRequest,
        401 => AppErrorCode::CredentialMismatch,
        404 => AppErrorCode::NotFound,
        422 => AppErrorCode::MediaUnsupported,
        507 => AppErrorCode::StorageFull,
        503 => AppErrorCode::RuntimeUnavailable,
        _ => AppErrorCode::Internal,
    }
}

fn unix_seconds() -> Result<String, CommandError> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| CommandError::new(AppErrorCode::Internal))?
        .as_secs();
    Ok(seconds.to_string())
}

/// Sign and POST one named command. `bearer` is the token string the API holds
/// as its HMAC key and bearer credential; its bytes are the HMAC key.
fn post_with_token(
    addr: &str,
    bearer: &str,
    command: &str,
    body: &Value,
) -> Result<Value, CommandError> {
    let body_bytes =
        serde_json::to_vec(body).map_err(|_| CommandError::new(AppErrorCode::Internal))?;
    let timestamp = unix_seconds()?;
    let path = format!("/{command}");
    let signature = sign_request(bearer.as_bytes(), &timestamp, "POST", &path, &body_bytes);

    let request = format!(
        "POST {path} HTTP/1.1\r\n\
         Host: {addr}\r\n\
         Authorization: Bearer {bearer}\r\n\
         X-MediaHaven-Timestamp: {timestamp}\r\n\
         X-MediaHaven-Sig: {signature}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n",
        body_bytes.len()
    );

    let mut stream = TcpStream::connect(addr)
        .map_err(|_| CommandError::new(AppErrorCode::BackendUnreachable))?;
    let timeout = Some(Duration::from_secs(10));
    let _ = stream.set_read_timeout(timeout);
    let _ = stream.set_write_timeout(timeout);

    stream
        .write_all(request.as_bytes())
        .and_then(|()| stream.write_all(&body_bytes))
        .map_err(|_| CommandError::new(AppErrorCode::BackendUnreachable))?;

    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|_| CommandError::new(AppErrorCode::BackendUnreachable))?;

    let split = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| CommandError::new(AppErrorCode::Internal))?;
    let status_line = String::from_utf8_lossy(&raw[..split]);
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| CommandError::new(AppErrorCode::Internal))?;
    if !(200..300).contains(&status) {
        // The backend returns the exact code in its JSON body. Status alone is
        // lossy: 404 covers both not_found and media_missing, and 503 covers
        // five distinct codes with different retryability. Prefer the code;
        // fall back to the status mapping only if the body is unparseable.
        // Backend message text is discarded either way — fixed messages only.
        let code = serde_json::from_slice::<BackendCode>(&raw[split + 4..])
            .map(|body| body.code)
            .unwrap_or_else(|_| status_code(status));
        return Err(CommandError::new(code));
    }
    serde_json::from_slice(&raw[split + 4..]).map_err(|_| CommandError::new(AppErrorCode::Internal))
}

/// Load the install token and forward one named renderer command.
fn post(command: &str, body: Value) -> Result<Value, CommandError> {
    assert!(
        COMMANDS.contains(&command),
        "unregistered command: {command}"
    );
    let token = load_or_create_token(TOKEN_SERVICE, TOKEN_ACCOUNT)
        .map_err(|_| CommandError::new(AppErrorCode::CredentialMismatch))?;
    let bearer = token.as_hex();
    post_with_token(API_ADDR, &bearer, command, &body)
}

/// Signed POST to a host-internal route. Same wire format and token as a
/// renderer command, but deliberately outside `COMMANDS` and the 24-route parity
/// set: only the host calls these paths (ADR-0018).
fn post_internal(path: &str, body: Value) -> Result<Value, CommandError> {
    let token = load_or_create_token(TOKEN_SERVICE, TOKEN_ACCOUNT)
        .map_err(|_| CommandError::new(AppErrorCode::CredentialMismatch))?;
    let bearer = token.as_hex();
    post_with_token(API_ADDR, &bearer, path, &body)
}

/// Signed GET to the host-internal `playback_source` route. It returns a
/// *container-path identity* only; resolution to a host file stays in player.rs.
/// GET keeps this lookup out of the 24 renderer POST commands (ADR-0015).
fn get_source(media_id: &str) -> Result<Option<String>, CommandError> {
    let token = load_or_create_token(TOKEN_SERVICE, TOKEN_ACCOUNT)
        .map_err(|_| CommandError::new(AppErrorCode::CredentialMismatch))?;
    let bearer = token.as_hex();
    let path = format!("/playback_source/{media_id}");
    let timestamp = unix_seconds()?;
    let signature = sign_request(bearer.as_bytes(), &timestamp, "GET", &path, b"");
    let request = format!(
        "GET {path} HTTP/1.1\r\n\
         Host: {API_ADDR}\r\n\
         Authorization: Bearer {bearer}\r\n\
         X-MediaHaven-Timestamp: {timestamp}\r\n\
         X-MediaHaven-Sig: {signature}\r\n\
         Connection: close\r\n\r\n"
    );

    let mut stream = TcpStream::connect(API_ADDR)
        .map_err(|_| CommandError::new(AppErrorCode::BackendUnreachable))?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    stream
        .write_all(request.as_bytes())
        .map_err(|_| CommandError::new(AppErrorCode::BackendUnreachable))?;
    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|_| CommandError::new(AppErrorCode::BackendUnreachable))?;

    let split = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| CommandError::new(AppErrorCode::Internal))?;
    let status_line = String::from_utf8_lossy(&raw[..split]);
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| CommandError::new(AppErrorCode::Internal))?;
    if !(200..300).contains(&status) {
        let code = serde_json::from_slice::<BackendCode>(&raw[split + 4..])
            .map(|body| body.code)
            .unwrap_or_else(|_| status_code(status));
        return Err(CommandError::new(code));
    }
    let body: Value = serde_json::from_slice(&raw[split + 4..])
        .map_err(|_| CommandError::new(AppErrorCode::Internal))?;
    Ok(body.get("path").and_then(Value::as_str).map(str::to_string))
}

/// Read the stored Defender gate for a media id. `None` means a library
/// import (no download row), which does not need a host scan.
fn defender_state_for(media_id: &str) -> Result<Option<String>, CommandError> {
    let value = post_internal("host/defender_state", json!({ "mediaId": media_id }))?;
    parse_defender_state(&value)
}

/// Keep the wire shape at the host/backend boundary closed. An unknown shape
/// fails to Internal; it is never treated as a clean scan.
fn parse_defender_state(value: &Value) -> Result<Option<String>, CommandError> {
    match value.get("state") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(state)) => Ok(Some(state.clone())),
        _ => Err(CommandError::new(AppErrorCode::Internal)),
    }
}

/// Store only a closed Defender state spelling. No path or scanner output is
/// sent back to the backend: the host keeps both strictly local.
fn record_defender_state(media_id: &str, state: &str) -> Result<(), CommandError> {
    post_internal(
        "host/defender_result",
        json!({ "mediaId": media_id, "state": state }),
    )?;
    Ok(())
}

/// The probe needs the host's Docker CLI, so it runs here, not in `core`.
/// Only the closed state spelling goes back; no path or ffprobe output does.
fn probe_and_record(media_id: &str, container_path: &str) -> Result<(), CommandError> {
    let state = inspect_with_probe(container_path).as_str().to_string();
    post_internal(
        "host/acquisition_inspect",
        json!({ "mediaId": media_id, "state": state }),
    )?;
    Ok(())
}

/// Persisted host directory behind the container's `/downloads` (ADR-0018,
/// storage option B). It is app-internal: the path never enters a DTO.
const ACQUISITION_ROOT_FILE: &str = "acquisition-root.txt";
/// The env var compose and the renderer both use for the host download root.
const DOWNLOAD_ROOT_ENV: &str = "MEDIAHAVEN_DOWNLOAD_ROOT";

fn acquisition_root() -> Result<PathBuf, CommandError> {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| CommandError::new(AppErrorCode::Internal))?;
    let dir = base.join("MEDIA_HAVEN");
    std::fs::create_dir_all(&dir).map_err(|_| CommandError::new(AppErrorCode::Internal))?;
    let file = dir.join(ACQUISITION_ROOT_FILE);
    // An explicit override is persisted too, so a later render without the
    // env var resolves the same directory.
    let path = std::env::var_os(DOWNLOAD_ROOT_ENV)
        .map(PathBuf::from)
        .or_else(|| {
            std::fs::read_to_string(&file)
                .ok()
                .map(|s| PathBuf::from(s.trim()))
        })
        .unwrap_or_else(|| dir.join("downloads"));
    std::fs::create_dir_all(&path).map_err(|_| CommandError::new(AppErrorCode::Internal))?;
    std::fs::write(&file, path.to_string_lossy().as_bytes())
        .map_err(|_| CommandError::new(AppErrorCode::Internal))?;
    Ok(path)
}

/// Build the specs that drive BOTH the override fragment (`/downloads:ro`) and
/// the persisted container→host map. Without `download_root`, acquired files
/// could not be resolved by mpv at runtime (Task 2 review finding).
fn root_specs(roots: &Value, download_root: &Path) -> Result<Vec<RootSpec>, CommandError> {
    let entries = roots
        .get("roots")
        .and_then(Value::as_array)
        .ok_or_else(|| CommandError::new(AppErrorCode::Internal))?;
    let mut specs: Vec<RootSpec> = entries
        .iter()
        .filter_map(|root| {
            let id = root.get("id")?.as_str()?;
            let path = root.get("path")?.as_str()?;
            Some(RootSpec::new(id, path))
        })
        .collect();
    if specs.is_empty() {
        // No library root yet: the acquisition mount must still be rendered.
        specs.push(RootSpec::new("downloads", download_root));
    }
    for spec in &mut specs {
        spec.download_root = Some(download_root.to_path_buf());
    }
    Ok(specs)
}

/// Rebuild A9's container→host map from the persisted roots. A stale or
/// missing root makes `render_mounts` fail, so resolution fails closed.
fn load_host_path_map() -> Result<BTreeMap<String, PathBuf>, CommandError> {
    let value = post("library_roots", json!({}))?;
    let specs = root_specs(&value, &acquisition_root()?)?;
    render_mounts(&specs)
        .map(|rendered| rendered.host_path_map())
        .map_err(|_| CommandError::source_unavailable())
}

// --- Library ---------------------------------------------------------------

#[tauri::command]
pub fn library_home() -> Result<Value, CommandError> {
    post("library_home", json!({}))
}

#[tauri::command]
pub fn library_movies(query: Value) -> Result<Value, CommandError> {
    post("library_movies", json!({ "query": query }))
}

#[tauri::command]
pub fn library_shows(query: Value) -> Result<Value, CommandError> {
    post("library_shows", json!({ "query": query }))
}

#[tauri::command]
pub fn library_detail(id: String) -> Result<Value, CommandError> {
    post("library_detail", json!({ "id": id }))
}

#[tauri::command]
pub fn library_episodes(series_id: String, season_number: i64) -> Result<Value, CommandError> {
    post(
        "library_episodes",
        json!({ "seriesId": series_id, "seasonNumber": season_number }),
    )
}

#[tauri::command]
pub fn library_search(query: Value) -> Result<Value, CommandError> {
    post("library_search", json!({ "query": query }))
}

#[tauri::command]
pub fn library_roots() -> Result<Value, CommandError> {
    post("library_roots", json!({}))
}

#[tauri::command]
pub fn library_root_add(display_name: String, path: String) -> Result<Value, CommandError> {
    post(
        "library_root_add",
        json!({ "displayName": display_name, "path": path }),
    )
}

#[tauri::command]
pub fn library_root_remove(root_id: String) -> Result<Value, CommandError> {
    post("library_root_remove", json!({ "rootId": root_id }))
}

#[tauri::command]
pub fn library_scan(root_id: String) -> Result<Value, CommandError> {
    post("library_scan", json!({ "rootId": root_id }))
}

#[tauri::command]
pub fn library_watchlist() -> Result<Value, CommandError> {
    post("library_watchlist", json!({}))
}

// --- Watchlist -------------------------------------------------------------

#[tauri::command]
pub fn watchlist_add(media_id: String) -> Result<Value, CommandError> {
    post("watchlist_add", json!({ "mediaId": media_id }))
}

#[tauri::command]
pub fn watchlist_remove(media_id: String) -> Result<Value, CommandError> {
    post("watchlist_remove", json!({ "mediaId": media_id }))
}

#[tauri::command]
pub fn watchlist_reorder(ordered_ids: Vec<String>) -> Result<Value, CommandError> {
    post("watchlist_reorder", json!({ "orderedIds": ordered_ids }))
}

// --- Queue -----------------------------------------------------------------

#[tauri::command]
pub fn queue_list() -> Result<Value, CommandError> {
    post("queue_list", json!({}))
}

#[tauri::command]
pub fn queue_add(media_id: String) -> Result<Value, CommandError> {
    post("queue_add", json!({ "mediaId": media_id }))
}

#[tauri::command]
pub fn queue_remove(queue_id: String) -> Result<Value, CommandError> {
    post("queue_remove", json!({ "queueId": queue_id }))
}

#[tauri::command]
pub fn queue_reorder(ordered_ids: Vec<String>) -> Result<Value, CommandError> {
    post("queue_reorder", json!({ "orderedIds": ordered_ids }))
}

// --- Downloads and settings ------------------------------------------------

#[tauri::command]
pub fn downloads_list() -> Result<Value, CommandError> {
    post("downloads_list", json!({}))
}

// --- Acquisition (host-side, named commands only) ---------------------------

/// Validate a user-chosen acquisition directory. Only an absolute path is
/// accepted; it is a host setting, never a renderer download/file path.
fn validate_download_root(raw: &str) -> Result<PathBuf, CommandError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(CommandError::new(AppErrorCode::InvalidRequest));
    }
    let path = PathBuf::from(trimmed);
    if !path.is_absolute() {
        return Err(CommandError::new(AppErrorCode::InvalidRequest));
    }
    Ok(path)
}

/// Add a user-supplied source. The host forwards to `core`, where the qB
/// client runs on the `acquire` network. Not a renderer POST command.
#[tauri::command]
pub fn download_add(source: String) -> Result<Value, CommandError> {
    post_internal("host/acquisition_add", json!({ "source": source }))
}

/// Attach an operator-supplied source to an item (R-01 / ADR-0021). Forwarded to
/// the host-internal core route; the renderer never sees a path or credential.
#[tauri::command]
pub fn source_set(media_id: String, source: String) -> Result<Value, CommandError> {
    post_internal(
        "host/acquisition_source_set",
        json!({ "mediaId": media_id, "source": source }),
    )
}

/// Add an operator-picked catalog title as a local wanted item. Forwarded to
/// the host-internal core route; no path or token reaches the renderer.
#[tauri::command]
pub fn catalog_wanted_add(
    title: String,
    year: Option<i64>,
    kind: String,
) -> Result<Value, CommandError> {
    post_internal(
        "host/catalog_wanted_add",
        json!({ "title": title, "year": year, "kind": kind }),
    )
}

/// Display-only IMDb/OMDb fields for a media id, from the opt-in offline TSV
/// datasets on the backend. A host-internal route, not a renderer POST command,
/// so the 24-command parity surface is unchanged (ADR-0018).
#[tauri::command]
pub fn imdb_lookup(media_id: String) -> Result<Value, CommandError> {
    post_internal("host/imdb_lookup", json!({ "mediaId": media_id }))
}

// --- Discovery (host-only, opt-in; never in COMMANDS) -----------------------

// DPAPI identities for operator-supplied config. Valid identities only.
const DISCOVERY_SERVICE: &str = "mediahaven";
const TORZNAB_URL_ACCOUNT: &str = "torznab-url";
const TORZNAB_KEY_ACCOUNT: &str = "torznab-key";
const PROWLARR_URL_ACCOUNT: &str = "prowlarr-url";
const PROWLARR_KEY_ACCOUNT: &str = "prowlarr-key";
const TMDB_KEY_ACCOUNT: &str = "tmdb-key";
/// Only used when a Prowlarr key exists but no URL was configured.
const PROWLARR_DEFAULT_URL: &str = "http://127.0.0.1:9117";

/// Read operator config from the secret store when present, else env. A stored
/// or env value that is blank is treated as absent. Read failures fail closed
/// to env/None rather than leaking an error to the renderer.
fn config_value(env_name: &str, service: &str, account: &str) -> Option<String> {
    if let Ok(Some(value)) = crate::secret::load_string(service, account) {
        let trimmed = value.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    std::env::var(env_name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn tmdb_enabled() -> bool {
    std::env::var("MEDIAHAVEN_TMDB_OPTIN").is_ok_and(|value| value == "1")
}

fn setup_required() -> Value {
    json!({ "items": [], "setup_required": true })
}

/// The resolved operator config for one source search. `direct` means the URL
/// is already a Torznab endpoint; otherwise it is a Prowlarr base URL whose
/// enabled indexers must be discovered first.
struct SourceConfig {
    endpoint: String,
    key: String,
    direct: bool,
}

type ConfigLookup<'a> = dyn Fn(&str, &str, &str) -> Option<String> + 'a;

/// Resolve the endpoint first, then pick the key that belongs to that mode:
/// explicit Torznab URL uses the Torznab key (Prowlarr key as fallback), while
/// Prowlarr discovery requires the Prowlarr key. A Torznab key without its URL
/// must not be sent to Prowlarr. No key means no request.
fn resolve_source_config(get: &ConfigLookup) -> Option<SourceConfig> {
    let direct_endpoint = get(
        "MEDIAHAVEN_TORZNAB_URL",
        DISCOVERY_SERVICE,
        TORZNAB_URL_ACCOUNT,
    );
    let prowlarr_key = get(
        "MEDIAHAVEN_PROWLARR_API_KEY",
        DISCOVERY_SERVICE,
        PROWLARR_KEY_ACCOUNT,
    );
    if let Some(endpoint) = direct_endpoint {
        let key = get(
            "MEDIAHAVEN_TORZNAB_KEY",
            DISCOVERY_SERVICE,
            TORZNAB_KEY_ACCOUNT,
        )
        .or(prowlarr_key)?;
        return Some(SourceConfig {
            endpoint,
            key,
            direct: true,
        });
    }
    let key = prowlarr_key?;
    let endpoint = get(
        "MEDIAHAVEN_PROWLARR_URL",
        DISCOVERY_SERVICE,
        PROWLARR_URL_ACCOUNT,
    )
    .unwrap_or_else(|| PROWLARR_DEFAULT_URL.to_string());
    Some(SourceConfig {
        endpoint,
        key,
        direct: false,
    })
}

/// Config and transport are injected so the alias/default rules are testable
/// without touching the process environment or making a network call.
fn source_search_with(
    config: Option<SourceConfig>,
    title: &str,
    year: Option<i64>,
    fetch: &crate::discovery::FetchFn,
) -> Value {
    let Some(config) = config else {
        return setup_required();
    };
    let result = if config.direct {
        crate::discovery::torznab_search(&config.endpoint, &config.key, title, year, fetch)
    } else {
        // Prowlarr v3 JSON API first: some installs return 404 on per-indexer
        // Torznab URLs. Any failure or empty result falls back to the existing
        // discovery + multi-Torznab path.
        match crate::discovery::prowlarr_search(&config.endpoint, &config.key, title, year, fetch) {
            Ok(items) if !items.is_empty() => Ok(items),
            _ => crate::discovery::prowlarr_torznab_endpoints(&config.endpoint, &config.key, fetch)
                .and_then(|endpoints| {
                    crate::discovery::torznab_search_multi(
                        &endpoints,
                        &config.key,
                        title,
                        year,
                        fetch,
                    )
                }),
        }
    };
    let (items, error) = match result {
        Ok(items) => (items, None),
        Err(_) => (Vec::new(), Some("Source search could not be reached.")),
    };
    let mut body = json!({ "items": items, "setup_required": false });
    if let Some(error) = error {
        body["error"] = json!(error);
    }
    body
}

/// Ranked source shortlist for a library title. No config => empty list plus
/// a display-only `setup_required` flag, zero requests and zero error leakage.
/// A Prowlarr key works without a URL; the magnet flows to `source_set` only
/// after a human confirms the choice. The Prowlarr JSON path passes the
/// resolved key as an `X-Api-Key` header; URL-only Torznab/TMDB paths keep the
/// existing 1 MiB fetch. All paths share the 4 MiB response bound here.
#[tauri::command]
pub fn source_search(title: String, year: Option<i64>) -> Result<Value, CommandError> {
    let config =
        resolve_source_config(&|name, service, account| config_value(name, service, account));
    let bounded: Box<crate::discovery::FetchFn> = Box::new(|url, api_key| match api_key {
        // Prowlarr's resolved key arrives through the seam; never in the URL.
        Some(key) => crate::discovery::http_fetch_with_api_key(url, key),
        None => crate::discovery::http_fetch_bounded(url, crate::discovery::MAX_PROWLARR_BYTES),
    });
    Ok(source_search_with(config, &title, year, &bounded))
}

/// URL-only adapter for the widened fetch seam, used by the TMDB clients.
fn url_only_fetch(url: &str, _key: Option<&str>) -> Result<Vec<u8>, String> {
    crate::discovery::http_fetch(url)
}

/// Opt-in TMDB catalog browse. `kind` is `"movie"` or `"tv"`; anything else is
/// a no-op. Requires both the opt-in flag and an operator API key.
#[tauri::command]
pub fn catalog_browse(kind: String) -> Result<Value, CommandError> {
    if !tmdb_enabled() {
        return Ok(setup_required());
    }
    let key = match config_value(
        "MEDIAHAVEN_TMDB_API_KEY",
        DISCOVERY_SERVICE,
        TMDB_KEY_ACCOUNT,
    ) {
        Some(key) => key,
        None => return Ok(setup_required()),
    };
    let path = match kind.as_str() {
        "movie" => "movie/popular",
        "tv" => "tv/popular",
        _ => return Ok(json!({ "items": [], "setup_required": false })),
    };
    let items = crate::discovery::tmdb_list(path, "", &key, &url_only_fetch);
    Ok(json!({ "items": items, "setup_required": false }))
}

/// Opt-in TMDB multi-search. Same gate as browse; only the query and key leave.
#[tauri::command]
pub fn catalog_search(query: String) -> Result<Value, CommandError> {
    if !tmdb_enabled() {
        return Ok(setup_required());
    }
    let key = match config_value(
        "MEDIAHAVEN_TMDB_API_KEY",
        DISCOVERY_SERVICE,
        TMDB_KEY_ACCOUNT,
    ) {
        Some(key) => key,
        None => return Ok(setup_required()),
    };
    let items = crate::discovery::tmdb_list("search/multi", &query, &key, &url_only_fetch);
    Ok(json!({ "items": items, "setup_required": false }))
}

/// Read-only public AniList browse/search. Results are fetched on demand and
/// returned transiently; the request contains no local library or watch data.
#[tauri::command]
pub fn catalog_anime(query: Option<String>) -> Result<Value, CommandError> {
    let query = query.unwrap_or_default();
    if query.chars().count() > 200 {
        return Ok(json!({ "items": [], "setup_required": false }));
    }
    let fetch: Box<crate::discovery::AniListFetchFn> =
        Box::new(crate::discovery::anilist_http_fetch);
    let query = if query.trim().is_empty() {
        None
    } else {
        Some(query)
    };
    let items = crate::discovery::anilist_list(query.as_deref(), &fetch);
    Ok(json!({ "items": items, "setup_required": false }))
}

/// Current acquisition rows (path-free identities) from the host-internal route.
#[tauri::command]
pub fn download_status() -> Result<Value, CommandError> {
    post_internal("host/acquisition_status", json!({}))
}

/// Remove a task by id, keeping its files (Task 6 owns scoped deletion).
#[tauri::command]
pub fn download_remove(id: String) -> Result<Value, CommandError> {
    post_internal("host/acquisition_remove", json!({ "id": id }))
}

/// Read the host acquisition directory the renderer/compose both use.
#[tauri::command]
pub fn download_root_get() -> Result<String, CommandError> {
    Ok(acquisition_root()?.to_string_lossy().into_owned())
}

/// Persist a new host acquisition directory. Matches `MEDIAHAVEN_DOWNLOAD_ROOT`
/// so the value the renderer reports is the one compose binds into qB.
#[tauri::command]
pub fn download_root_set(path: String) -> Result<String, CommandError> {
    let path = validate_download_root(&path)?;
    std::fs::create_dir_all(&path).map_err(|_| CommandError::new(AppErrorCode::InvalidRequest))?;
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| CommandError::new(AppErrorCode::Internal))?;
    let file = base.join("MEDIA_HAVEN").join(ACQUISITION_ROOT_FILE);
    std::fs::write(&file, path.to_string_lossy().as_bytes())
        .map_err(|_| CommandError::new(AppErrorCode::Internal))?;
    std::env::set_var(DOWNLOAD_ROOT_ENV, &path);
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn settings_get() -> Result<Value, CommandError> {
    post("settings_get", json!({}))
}

#[tauri::command]
pub fn settings_set(patch: Value) -> Result<Value, CommandError> {
    post("settings_set", json!({ "patch": patch }))
}

// --- Playback --------------------------------------------------------------

#[tauri::command]
pub fn playback_start(
    request: Value,
    state: tauri::State<'_, PlayerState>,
    app: tauri::AppHandle,
) -> Result<Value, CommandError> {
    let media_id = request
        .get("mediaId")
        .and_then(Value::as_str)
        .ok_or_else(|| CommandError::new(AppErrorCode::InvalidRequest))?
        .to_string();
    let start = request
        .get("startPositionSeconds")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);

    // 1. Host-only resolution: container path -> A9 mount map -> host file.
    //    This must happen before the backend play gate, because Windows
    //    Defender can only scan the real host path on this machine.
    let container_path = get_source(&media_id)?.ok_or_else(CommandError::source_unavailable)?;
    let host_map = load_host_path_map()?;
    let host_path = resolve_for_media(&host_map, &container_path)
        .map_err(|_| CommandError::source_unavailable())?;

    // 2. A download row with a non-clean Defender state needs one host-side
    //    scan. `None` means a library import, which has no Defender row and no
    //    acquisition gate. The result is stored by closed state spelling; no
    //    path or scanner output returns to the backend. The backend remains
    //    the authority: it re-checks the stored gate below.
    if let Some(stored) = defender_state_for(&media_id)? {
        // The probe is a download-only gate. `defender_state_for` returning
        // Some means a download row exists, so a library import is skipped.
        probe_and_record(&media_id, &container_path)?;
        if stored != "clean" {
            let host_path_text = host_path
                .to_str()
                .ok_or_else(|| CommandError::new(AppErrorCode::InvalidRequest))?;
            let scanned = defender_scan(host_path_text);
            record_defender_state(&media_id, scanned.as_str())?;
        }
    }

    // 3. The backend stays the authority on what to play. `PlaybackSession`
    //    is display-only: no path and no token (ADR-0015).
    let session = post("playback_start", json!({ "request": request }))?;

    // 4. The backend already marked this media as the live session; if any
    //    host step fails below, release that gate or cleanup stays blocked
    //    until restart (finding 6). The original error is preserved.
    let started = (|| -> Result<(), CommandError> {
        (*state)
            .open_media(&host_path, start)
            .map_err(|error| match error {
                crate::player::PlayerError::Mpv(_) => CommandError::engine_unavailable(),
                crate::player::PlayerError::NoSurface => CommandError::surface_unavailable(),
                _ => CommandError::source_unavailable(),
            })?;
        state.begin(&media_id);
        spawn_status_poller((*state).clone(), app, media_id);
        Ok(())
    })();
    if started.is_err() {
        let _ = post("playback_stop", json!({}));
    }
    started?;
    Ok(session)
}

#[tauri::command]
pub fn playback_stop(state: tauri::State<'_, PlayerState>) -> Result<Value, CommandError> {
    let result = post("playback_stop", json!({}));
    state.clear_progress();
    state.stop();
    result
}

#[tauri::command]
pub fn playback_checkpoint(
    request: Value,
    state: tauri::State<'_, PlayerState>,
) -> Result<Value, CommandError> {
    let media_id = request
        .get("mediaId")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let position = request
        .get("positionSeconds")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let duration = request
        .get("durationSeconds")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);

    if !media_id.is_empty() {
        state.observe_checkpoint(&media_id, position, duration);
        if let Some((id, watched, dur, pos)) = state.take_completion() {
            // Finding 3: the host calls the signed /host/ confirm route on a
            // B5-shaped completion; the backend re-checks B5 before it writes
            // any cleanup entry. A failed confirm must never fail the progress
            // save, so the result is deliberately ignored here.
            let _ = post_internal(
                "host/acquisition_cleanup_confirm",
                json!({
                    "mediaId": id,
                    "watchedSeconds": watched,
                    "durationSeconds": dur,
                    "positionSeconds": pos,
                    "endedNormally": true,
                    "userStopped": false
                }),
            );
        }
    }
    post("playback_checkpoint", json!({ "request": request }))
}

/// A named host→engine control command (play/pause/seek/volume/stop). Not a
/// passthrough and not a backend capability: it only drives the local mpv
/// instance and accepts no path, token or arbitrary command name.
#[tauri::command]
pub fn player_control(
    action: String,
    value: Option<f64>,
    state: tauri::State<'_, PlayerState>,
) -> Result<Value, CommandError> {
    (*state)
        .control(&action, value)
        .map_err(|_| CommandError::new(AppErrorCode::InvalidRequest))?;
    Ok(Value::Null)
}

/// Host-only: attach the native video surface inside our single window.
/// Accepts four finite numbers; never a path, token, HWND value or URL.
#[tauri::command]
pub fn player_attach_surface(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    state: tauri::State<'_, PlayerState>,
    app: tauri::AppHandle,
) -> Result<Value, CommandError> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| CommandError::new(AppErrorCode::InvalidRequest))?;
    let parent = window
        .hwnd()
        .map(|hwnd| hwnd.0 as usize)
        .map_err(|_| CommandError::new(AppErrorCode::InvalidRequest))?;
    let dpi = window
        .scale_factor()
        .map(|factor| (factor * 96.0).round() as u32)
        .unwrap_or(96);
    (*state)
        .attach_surface(x, y, width, height, dpi, parent)
        .map_err(|_| CommandError::new(AppErrorCode::InvalidRequest))?;
    Ok(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn defender_state_wire_shape_is_closed() {
        assert_eq!(
            parse_defender_state(&serde_json::json!({ "state": "clean" })).unwrap(),
            Some("clean".to_string())
        );
        assert_eq!(
            parse_defender_state(&serde_json::json!({ "state": null })).unwrap(),
            None,
            "a library import has no Defender row"
        );
        assert_eq!(parse_defender_state(&serde_json::json!({})).unwrap(), None);
        assert!(
            parse_defender_state(&serde_json::json!({ "state": 1 })).is_err(),
            "an unexpected wire shape must never be read as a clean scan"
        );
    }

    /// A loopback stub verifies the signed request and answers twice: a real
    /// JSON body, then an error whose body must not leak to the renderer.
    #[test]
    fn signed_forwarder_round_trips_and_never_leaks_backend_text() {
        let ok_body = r#"{"scanned":1}"#;
        let hostile_body = r#"{"message":"C:\\secret\\token leaked"}"#;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let ok_owned = ok_body.to_string();
        let hostile_owned = hostile_body.to_string();

        let server = thread::spawn(move || {
            for (status, response_body) in [
                ("200 OK", ok_owned.as_str()),
                ("500 Internal Server Error", hostile_owned.as_str()),
            ] {
                let (mut socket, _) = listener.accept().unwrap();
                let mut raw = Vec::new();
                let mut buffer = [0u8; 1024];
                let (head_end, content_length) = loop {
                    let read = socket.read(&mut buffer).unwrap();
                    assert!(read > 0, "the forwarder must send a request");
                    raw.extend_from_slice(&buffer[..read]);
                    if let Some(end) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&raw[..end]).to_ascii_lowercase();
                        let length: usize = head
                            .lines()
                            .find_map(|line| {
                                line.strip_prefix("content-length:")
                                    .map(|value| value.trim().parse().unwrap())
                            })
                            .unwrap();
                        break (end, length);
                    }
                };
                while raw.len() < head_end + 4 + content_length {
                    let read = socket.read(&mut buffer).unwrap();
                    raw.extend_from_slice(&buffer[..read]);
                }

                let head = String::from_utf8_lossy(&raw[..head_end]);
                let header = |name: &str| {
                    head.lines()
                        .find_map(|line| line.strip_prefix(name))
                        .unwrap()
                        .trim()
                        .to_string()
                };
                let bearer = header("Authorization: Bearer ");
                let timestamp = header("X-MediaHaven-Timestamp: ");
                let signature = header("X-MediaHaven-Sig: ");
                let body = &raw[head_end + 4..head_end + 4 + content_length];
                assert_eq!(body, b"{}");
                assert_eq!(
                    signature,
                    sign_request(bearer.as_bytes(), &timestamp, "POST", "/library_home", body)
                );

                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response_body}",
                    response_body.len()
                );
                socket.write_all(response.as_bytes()).unwrap();
            }
        });

        let value = post_with_token(&addr, "unit-token", "library_home", &json!({})).unwrap();
        assert_eq!(value, json!({ "scanned": 1 }));

        let error = post_with_token(&addr, "unit-token", "library_home", &json!({})).unwrap_err();
        assert_eq!(error.code, AppErrorCode::Internal);
        assert_eq!(error.message, "The backend request failed.");
        assert!(!serde_json::to_string(&error).unwrap().contains("secret"));

        server.join().unwrap();
    }

    #[test]
    fn backend_code_is_preferred_over_status() {
        // 404 normally means NotFound, but the backend sends media_missing.
        // The exact code must win so retryability/message are right.
        let body = r#"{"code":"media_missing","message":"x","retryable":false}"#;
        let decoded: BackendCode = serde_json::from_str(body).unwrap();
        assert_eq!(decoded.code, AppErrorCode::MediaMissing);
        assert_ne!(status_code(404), AppErrorCode::MediaMissing);
    }

    #[test]
    fn status_codes_map_to_the_closed_set() {
        assert_eq!(status_code(400), AppErrorCode::InvalidRequest);
        assert_eq!(status_code(401), AppErrorCode::CredentialMismatch);
        assert_eq!(status_code(404), AppErrorCode::NotFound);
        assert_eq!(status_code(422), AppErrorCode::MediaUnsupported);
        assert_eq!(status_code(503), AppErrorCode::RuntimeUnavailable);
        assert_eq!(status_code(507), AppErrorCode::StorageFull);
        assert_eq!(status_code(500), AppErrorCode::Internal);
    }

    #[test]
    fn surface_unavailable_is_legible_and_retryable() {
        let error = CommandError::surface_unavailable();
        assert_eq!(error.code, AppErrorCode::RuntimeUnavailable);
        assert_eq!(error.message, "player surface unavailable");
        assert!(error.retryable);
    }

    #[test]
    fn acquisition_root_is_rendered_and_added_to_the_host_map() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let base = std::env::temp_dir().join(format!(
            "mh-task3-downloads-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let movies = base.join("movies");
        let downloads = base.join("downloads");
        std::fs::create_dir_all(&movies).unwrap();
        std::fs::create_dir_all(&downloads).unwrap();

        let roots = json!({ "roots": [{ "id": "r1", "path": movies.to_string_lossy() }] });
        let specs = root_specs(&roots, &downloads).expect("specs carry the download root");
        let rendered = render_mounts(&specs).expect("override renders");

        // The override fragment has the real read-only acquisition bind.
        assert_eq!(rendered.fragment.matches("/downloads:ro").count(), 1);
        // mpv resolves acquired files through the persisted host map.
        let map = rendered.host_path_map();
        assert_eq!(
            map["/downloads"],
            std::fs::canonicalize(&downloads).unwrap()
        );
        assert!(map.contains_key("/media/root-0"));

        // With no library roots the acquisition mount is still rendered.
        let empty = root_specs(&json!({ "roots": [] }), &downloads).unwrap();
        let only_downloads = render_mounts(&empty).expect("download-only render");
        assert!(only_downloads.host_path_map().contains_key("/downloads"));

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn compose_binds_qb_downloads_to_the_host_acquisition_root() {
        const COMPOSE: &str = include_str!("../../../../../infrastructure/containers/compose.yaml");
        // qB writes to the same host dir the supervisor binds read-only into
        // core, so mpv resolves exactly the files qB downloaded.
        assert!(
            COMPOSE
                .contains("${MEDIAHAVEN_DOWNLOAD_ROOT:?host acquisition directory}:/downloads:rw"),
            "qB /downloads must be a required host bind, got:\n{COMPOSE}"
        );
        assert!(
            !COMPOSE.contains("downloads:/downloads:rw"),
            "qB must not use a separate named volume for /downloads"
        );
        assert!(
            !COMPOSE.lines().any(|line| line == "  downloads:"),
            "the unused downloads named volume must be removed"
        );
        // The stores core and probe rely on must survive the fix.
        assert!(COMPOSE.contains("  core-data:") && COMPOSE.contains("  staging:"));
    }

    #[test]
    fn download_root_setting_requires_an_absolute_path() {
        assert!(validate_download_root("").is_err());
        assert!(validate_download_root("  ").is_err());
        assert!(validate_download_root("relative/downloads").is_err());
        let absolute = std::env::temp_dir().join("mh-download-root-setting");
        let validated =
            validate_download_root(&absolute.to_string_lossy()).expect("absolute path accepted");
        assert_eq!(validated, absolute);
    }

    #[test]
    fn catalog_wanted_add_is_wired_to_the_handler_surface() {
        // The renderer invokes this name; lib.rs must register it in
        // generate_handler! or the call dies with "command not found" (ADR-0024).
        let host = include_str!("../lib.rs");
        assert!(
            host.contains("commands::catalog_wanted_add,"),
            "catalog_wanted_add must be registered in generate_handler!"
        );
        // Host-internal only: the 24-command POST surface stays unchanged.
        assert!(!COMMANDS.contains(&"catalog_wanted_add"));
    }

    // --- Source config alias/default resolution (host-side, injected) ------

    const INDEXERS: &str =
        r#"[{"id":3,"enable":true,"torznabUrl":"http://127.0.0.1:9117/idx/api?apikey=hidden"}]"#;
    const RSS: &str = "<rss><channel><item><title>Movie 1080p</title><size>10</size>\
        <seeders>2</seeders><link>magnet:?xt=urn:btih:a</link></item></channel></rss>";

    fn source_fetch() -> (
        Box<crate::discovery::FetchFn>,
        std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    ) {
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let record = seen.clone();
        (
            Box::new(move |url: &str, _key: Option<&str>| {
                record.lock().unwrap().push(url.to_string());
                if url.contains("/api/v1/indexer?apikey=") {
                    Ok(INDEXERS.as_bytes().to_vec())
                } else {
                    Ok(RSS.as_bytes().to_vec())
                }
            }),
            seen,
        )
    }

    #[test]
    fn source_search_uses_prowlarr_alias_and_local_default() {
        let (fetch, seen) = source_fetch();
        let get = |name: &str, _: &str, _: &str| match name {
            "MEDIAHAVEN_PROWLARR_API_KEY" => Some("prowl-key".to_string()),
            _ => None,
        };
        let value = source_search_with(resolve_source_config(&get), "Movie", Some(2020), &fetch);
        assert_eq!(value["setup_required"], false);
        assert_eq!(value["items"][0]["title"], "Movie 1080p");
        assert_eq!(
            seen.lock().unwrap().as_slice(),
            &[
                "http://127.0.0.1:9117/api/v1/search?query=Movie%202020".to_string(),
                "http://127.0.0.1:9117/api/v1/indexer?apikey=prowl-key".to_string(),
                "http://127.0.0.1:9117/idx/api?t=search&q=Movie%202020&apikey=prowl-key"
                    .to_string(),
            ]
        );
    }

    #[test]
    fn source_search_reports_a_failed_endpoint() {
        let fetch: Box<crate::discovery::FetchFn> =
            Box::new(|_: &str, _: Option<&str>| Err("unreachable".to_string()));
        let get = |name: &str, _: &str, _: &str| match name {
            "MEDIAHAVEN_PROWLARR_API_KEY" => Some("prowl-key".to_string()),
            _ => None,
        };
        let value = source_search_with(resolve_source_config(&get), "Movie", None, &fetch);
        assert_eq!(value["setup_required"], false);
        assert_eq!(value["items"].as_array().map(Vec::len), Some(0));
        assert_eq!(value["error"], "Source search could not be reached.");
    }

    #[test]
    fn source_search_prefers_prowlarr_json_with_header_auth() {
        const JSON: &str = r#"[{"title":"Movie 720p","magnetUrl":"magnet:?xt=urn:btih:j","size":5,"seeders":1,"indexer":"idx"}]"#;
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let record = seen.clone();
        let fetch: Box<crate::discovery::FetchFn> =
            Box::new(move |url: &str, key: Option<&str>| {
                record
                    .lock()
                    .unwrap()
                    .push((url.to_string(), key.map(str::to_string)));
                if url.contains("/api/v1/search") {
                    Ok(JSON.as_bytes().to_vec())
                } else {
                    Ok(INDEXERS.as_bytes().to_vec())
                }
            });
        let get = |name: &str, _: &str, _: &str| match name {
            "MEDIAHAVEN_PROWLARR_API_KEY" => Some("prowl-key".to_string()),
            _ => None,
        };
        let value = source_search_with(resolve_source_config(&get), "Movie", None, &fetch);
        assert_eq!(value["setup_required"], false);
        assert_eq!(value["items"][0]["title"], "Movie 720p");
        assert_eq!(value["items"][0]["resolution"], "720p");
        // No fallback call, no key in the URL, and the key reached the fetch
        // seam so the production transport can send it as X-Api-Key.
        assert_eq!(
            seen.lock().unwrap().as_slice(),
            &[(
                "http://127.0.0.1:9117/api/v1/search?query=Movie".to_string(),
                Some("prowl-key".to_string())
            )]
        );
    }

    #[test]
    fn source_search_falls_back_when_prowlarr_json_fails() {
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let record = seen.clone();
        let fetch: Box<crate::discovery::FetchFn> =
            Box::new(move |url: &str, _key: Option<&str>| {
                record.lock().unwrap().push(url.to_string());
                if url.contains("/api/v1/search") {
                    Err("http 500".to_string())
                } else if url.contains("/api/v1/indexer?apikey=") {
                    Ok(INDEXERS.as_bytes().to_vec())
                } else {
                    Ok(RSS.as_bytes().to_vec())
                }
            });
        let get = |name: &str, _: &str, _: &str| match name {
            "MEDIAHAVEN_PROWLARR_API_KEY" => Some("prowl-key".to_string()),
            _ => None,
        };
        let value = source_search_with(resolve_source_config(&get), "Movie", None, &fetch);
        assert_eq!(value["setup_required"], false);
        assert_eq!(value["items"][0]["title"], "Movie 1080p");
        assert_eq!(seen.lock().unwrap().len(), 3);
    }

    #[test]
    fn source_search_prefers_torznab_key_url_and_prowlarr_url_override() {
        let (fetch, seen) = source_fetch();

        // An explicit Torznab URL/key goes straight to search: no discovery.
        let direct = |name: &str, _: &str, _: &str| match name {
            "MEDIAHAVEN_TORZNAB_URL" => Some("http://explicit/api".to_string()),
            "MEDIAHAVEN_TORZNAB_KEY" => Some("tz-key".to_string()),
            "MEDIAHAVEN_PROWLARR_API_KEY" => Some("prowl-key".to_string()),
            _ => None,
        };
        let value = source_search_with(resolve_source_config(&direct), "Movie", None, &fetch);
        assert_eq!(value["setup_required"], false);
        assert_eq!(
            seen.lock().unwrap().as_slice(),
            &["http://explicit/api?t=search&q=Movie&apikey=tz-key".to_string()]
        );

        // Prowlarr key without a Torznab key prefers the Prowlarr URL.
        seen.lock().unwrap().clear();
        let override_url = |name: &str, _: &str, _: &str| match name {
            "MEDIAHAVEN_PROWLARR_API_KEY" => Some("prowl-key".to_string()),
            "MEDIAHAVEN_PROWLARR_URL" => Some("http://custom:9117/".to_string()),
            _ => None,
        };
        let value = source_search_with(resolve_source_config(&override_url), "Movie", None, &fetch);
        assert_eq!(value["setup_required"], false);
        assert_eq!(
            seen.lock().unwrap()[0],
            "http://custom:9117/api/v1/search?query=Movie"
        );
        assert_eq!(
            seen.lock().unwrap()[1],
            "http://custom:9117/api/v1/indexer?apikey=prowl-key"
        );
    }

    #[test]
    fn source_search_discovery_mode_uses_the_prowlarr_key() {
        let (fetch, seen) = source_fetch();
        let both_keys = |name: &str, _: &str, _: &str| match name {
            "MEDIAHAVEN_TORZNAB_KEY" => Some("tz-key".to_string()),
            "MEDIAHAVEN_PROWLARR_API_KEY" => Some("prowl-key".to_string()),
            "MEDIAHAVEN_PROWLARR_URL" => Some("http://127.0.0.1:9117".to_string()),
            _ => None,
        };
        let value = source_search_with(resolve_source_config(&both_keys), "Movie", None, &fetch);
        assert_eq!(value["setup_required"], false);
        let seen = seen.lock().unwrap();
        assert!(seen.iter().all(|url| !url.contains("tz-key")));
        // The JSON search carries the key via header; discovery/Torznab keep
        // the existing query auth, so only those URLs end with apikey=.
        assert!(seen.iter().any(|url| url.ends_with("apikey=prowl-key")));
        assert!(seen
            .iter()
            .any(|url| url.contains("/api/v1/search?") && !url.contains("apikey=")));

        // A Torznab key with no Torznab URL cannot stand in for a Prowlarr key.
        let (fetch, seen) = source_fetch();
        let torznab_only = |name: &str, _: &str, _: &str| match name {
            "MEDIAHAVEN_TORZNAB_KEY" => Some("tz-key".to_string()),
            "MEDIAHAVEN_PROWLARR_URL" => Some("http://127.0.0.1:9117".to_string()),
            _ => None,
        };
        assert_eq!(
            source_search_with(resolve_source_config(&torznab_only), "Movie", None, &fetch),
            setup_required()
        );
        assert!(seen.lock().unwrap().is_empty());
    }

    #[test]
    fn source_search_without_a_key_makes_zero_calls() {
        let (fetch, seen) = source_fetch();
        let none = |_: &str, _: &str, _: &str| None;
        assert_eq!(
            source_search_with(resolve_source_config(&none), "Movie", None, &fetch),
            setup_required()
        );
        // A Torznab key alone with no URL is still incomplete config.
        let partial = |name: &str, _: &str, _: &str| match name {
            "MEDIAHAVEN_TORZNAB_KEY" => Some("tz-key".to_string()),
            _ => None,
        };
        assert_eq!(
            source_search_with(resolve_source_config(&partial), "Movie", None, &fetch),
            setup_required()
        );
        assert!(seen.lock().unwrap().is_empty());
    }
}
