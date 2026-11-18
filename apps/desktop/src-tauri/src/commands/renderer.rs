//! Signed host-side forwarder plus the 24 named renderer commands.
//!
//! The renderer invokes a named command with the request fields; this module
//! builds the closed JSON body the API expects, signs it with the per-install
//! token and forwards it over a loopback TCP connection. Only the backend's
//! JSON body is returned; on failure the renderer gets a legible code from the
//! closed `AppErrorCode` set, never raw backend text.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use app_core::domain::AppErrorCode;
use mediahaven_contract::{sign as sign_request, COMMANDS};
use serde::Serialize;
use serde_json::{json, Value};

use crate::secret::{load_or_create_token, TOKEN_ACCOUNT, TOKEN_SERVICE};

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
    assert!(
        COMMANDS.contains(&command),
        "unregistered command: {command}"
    );

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

/// Load the install token and forward one named command.
fn post(command: &str, body: Value) -> Result<Value, CommandError> {
    let token = load_or_create_token(TOKEN_SERVICE, TOKEN_ACCOUNT)
        .map_err(|_| CommandError::new(AppErrorCode::CredentialMismatch))?;
    let bearer = token.as_hex();
    post_with_token(API_ADDR, &bearer, command, &body)
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
pub fn playback_start(request: Value) -> Result<Value, CommandError> {
    post("playback_start", json!({ "request": request }))
}

#[tauri::command]
pub fn playback_stop() -> Result<Value, CommandError> {
    post("playback_stop", json!({}))
}

#[tauri::command]
pub fn playback_checkpoint(request: Value) -> Result<Value, CommandError> {
    post("playback_checkpoint", json!({ "request": request }))
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
}
