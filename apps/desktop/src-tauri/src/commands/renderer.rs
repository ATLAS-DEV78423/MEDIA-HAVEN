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
use mediahaven_contract::{sign as sign_request, COMMANDS};
use serde::Serialize;
use serde_json::{json, Value};

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

/// Display-only IMDb/OMDb fields for a media id, from the opt-in offline TSV
/// datasets on the backend. A host-internal route, not a renderer POST command,
/// so the 24-command parity surface is unchanged (ADR-0018).
#[tauri::command]
pub fn imdb_lookup(media_id: String) -> Result<Value, CommandError> {
    post_internal("host/imdb_lookup", json!({ "mediaId": media_id }))
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
    // 1. The backend stays the authority on what to play. `PlaybackSession`
    //    is display-only: no path and no token (ADR-0015).
    let session = post("playback_start", json!({ "request": request }))?;
    let media_id = request
        .get("mediaId")
        .and_then(Value::as_str)
        .ok_or_else(|| CommandError::new(AppErrorCode::InvalidRequest))?
        .to_string();
    let start = request
        .get("startPositionSeconds")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);

    // 2. Host-only: container path -> A9 mount map -> host file -> mpv. The
    //    backend already marked this media as the live session; if any host
    //    step fails below, release that gate or cleanup stays blocked until
    //    restart (finding 6). The original error is preserved.
    let started = (|| -> Result<(), CommandError> {
        let container_path = get_source(&media_id)?.ok_or_else(CommandError::source_unavailable)?;
        let host_map = load_host_path_map()?;
        let host_path = resolve_for_media(&host_map, &container_path)
            .map_err(|_| CommandError::source_unavailable())?;
        (*state)
            .open_media(&host_path, start)
            .map_err(|error| match error {
                crate::player::PlayerError::Mpv(_) => CommandError::engine_unavailable(),
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

/// A named host→engine control command (play/pause/seek/volume). Not a
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

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
}
