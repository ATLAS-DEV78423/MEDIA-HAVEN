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
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::player::{resolve_for_media, spawn_status_poller, PlayerState};
use crate::secret::{account_for, load_or_create_token, TOKEN_ACCOUNT, TOKEN_SERVICE};
use crate::supervisor::Supervisor;
use crate::{render_mounts, write_atomic, RootSpec};

/// The loopback API published by the supervisor. Matches `core`'s default bind.
const API_ADDR: &str = "127.0.0.1:8080";

/// The workspace definition is compiled in, then staged to an absolute temp
/// path: `wsl.exe` resolves `-f` inside the distro, where a Windows-relative
/// path does not exist. No path is ever returned to the renderer.
const COMPOSE_YAML: &str = include_str!("../../../../../infrastructure/containers/compose.yaml");

fn stage_compose() -> std::io::Result<PathBuf> {
    let path = std::env::temp_dir().join("mediahaven-compose.yaml");
    std::fs::write(&path, COMPOSE_YAML.as_bytes())?;
    Ok(path)
}

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

// --- Host-side mount persistence --------------------------------------------

/// One persisted library root: the stable container path the backend scans and
/// the host path the supervisor actually binds. Stored only on the host; the
/// renderer never sees either field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MountRecord {
    container_path: String,
    host_path: String,
}

/// Host state lives beside the acquisition root, under `%APPDATA%\MEDIA_HAVEN`.
fn host_state_dir() -> Result<PathBuf, CommandError> {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| CommandError::new(AppErrorCode::Internal))?;
    Ok(base.join("MEDIA_HAVEN"))
}

fn mounts_file() -> Result<PathBuf, CommandError> {
    Ok(host_state_dir()?.join("mounts.json"))
}

fn override_file() -> Result<PathBuf, CommandError> {
    Ok(host_state_dir()?.join("override.yaml"))
}

/// Path-parameterised core of [`load_mount_records`] so it can be tested
/// without mutating process-global environment variables.
fn load_mount_records_from(path: &Path) -> Result<Vec<MountRecord>, CommandError> {
    let raw = match std::fs::read_to_string(path) {
        Ok(text) => text,
        // A first run has no file yet: that is an empty mount set, not an error.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        // Never surface the failing path or I/O detail to the renderer.
        Err(_) => return Err(CommandError::new(AppErrorCode::InvalidRequest)),
    };
    serde_json::from_str(&raw).map_err(|_| CommandError::new(AppErrorCode::InvalidRequest))
}

fn load_mount_records() -> Result<Vec<MountRecord>, CommandError> {
    load_mount_records_from(&mounts_file()?)
}

/// Path-parameterised core of [`save_mount_records`]. The authority file is
/// written atomically so a crash cannot leave it truncated.
fn save_mount_records_to(path: &Path, records: &[MountRecord]) -> Result<(), CommandError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| CommandError::new(AppErrorCode::Internal))?;
    }
    let json = serde_json::to_string_pretty(records)
        .map_err(|_| CommandError::new(AppErrorCode::Internal))?;
    write_atomic(path, &json).map_err(|_| CommandError::new(AppErrorCode::Internal))
}

fn save_mount_records(records: &[MountRecord]) -> Result<(), CommandError> {
    save_mount_records_to(&mounts_file()?, records)
}

/// Turn persisted records into the specs that drive BOTH the override fragment
/// (`/downloads:ro`) and the persisted container→host map. Each record keeps
/// its stored container path, so removing a sibling never re-indexes it.
fn records_to_specs(records: &[MountRecord], download_root: &Path) -> Vec<RootSpec> {
    let mut specs: Vec<RootSpec> = records
        .iter()
        .enumerate()
        .map(|(i, record)| {
            let mut spec = RootSpec::new(format!("root-{i}"), PathBuf::from(&record.host_path));
            spec.container_path = Some(record.container_path.clone());
            spec.download_root = Some(download_root.to_path_buf());
            spec
        })
        .collect();
    if specs.is_empty() {
        // No library root yet: the acquisition mount must still be rendered.
        let mut spec = RootSpec::new("downloads", download_root);
        spec.download_root = Some(download_root.to_path_buf());
        specs.push(spec);
    }
    specs
}

/// Keep only records whose host path is still a valid, existing directory.
/// Reusing `render_mounts` on one spec applies the full lexical/containment/
/// reparse/existence validation, so a moved, deleted or offline folder is
/// skipped rather than bricking startup or playback. Pure: the persisted list
/// is never mutated, so a temporarily offline drive can come back later.
fn resolvable_records(records: &[MountRecord]) -> Vec<MountRecord> {
    records
        .iter()
        .filter(|record| {
            let spec = RootSpec::new("resolvable", PathBuf::from(&record.host_path));
            render_mounts(std::slice::from_ref(&spec)).is_ok()
        })
        .cloned()
        .collect()
}

/// Render the override from current host records and stage it atomically so a
/// later `compose up` binds the right host folders into `core`. Stale records
/// are skipped; the authority file on disk is left untouched.
fn render_and_stage_override(records: &[MountRecord]) -> Result<(), CommandError> {
    let download_root = acquisition_root()?;
    let available = resolvable_records(records);
    let specs = records_to_specs(&available, &download_root);
    let rendered = render_mounts(&specs).map_err(|_| CommandError::new(AppErrorCode::Internal))?;
    write_atomic(&override_file()?, &rendered.fragment)
        .map_err(|_| CommandError::new(AppErrorCode::Internal))
}

/// Validate a candidate host path by reusing `render_mounts`' lexical,
/// containment and reparse checks. `MountError` detail (which can embed the
/// path) is deliberately discarded; the renderer only gets a fixed error.
fn validate_host_path(path: &str) -> Result<(), CommandError> {
    let spec = RootSpec::new("candidate", path);
    render_mounts(std::slice::from_ref(&spec))
        .map(|_| ())
        .map_err(|_| CommandError::new(AppErrorCode::InvalidRequest))
}

/// Next stable container index: one past the highest `/media/root-<n>` suffix,
/// or zero when there are none. `root-0` and `root-7` yield `root-8`.
fn next_container_index(records: &[MountRecord]) -> usize {
    records
        .iter()
        .filter_map(|record| record.container_path.strip_prefix("/media/root-"))
        .filter_map(|suffix| suffix.parse::<usize>().ok())
        .max()
        .map_or(0, |highest| highest + 1)
}

/// Build the supervisor exactly as startup does: same distro default, staged
/// compose, loopback address and staged override file.
fn build_supervisor() -> Result<Supervisor, CommandError> {
    let distro =
        std::env::var("MEDIAHAVEN_WSL_DISTRO").unwrap_or_else(|_| "MEDIA_HAVEN".to_string());
    let address: std::net::SocketAddr = API_ADDR
        .parse()
        .map_err(|_| CommandError::new(AppErrorCode::Internal))?;
    let compose = stage_compose().map_err(|_| CommandError::new(AppErrorCode::Internal))?;
    Ok(Supervisor::new(distro, compose, address).with_override(override_file()?))
}

/// Recreate `core` so it picks up the freshly staged override, then wait for
/// the loopback port. Used only by root add/remove, never by startup.
fn remount_core() -> Result<(), CommandError> {
    let supervisor = build_supervisor()?;
    supervisor
        .ensure_running()
        .map_err(|_| CommandError::new(AppErrorCode::Internal))?;
    supervisor
        .wait_ready()
        .map_err(|_| CommandError::new(AppErrorCode::Internal))
}

/// Best-effort cleanup of a backend root left behind when an add fails after
/// the backend accepted it (applied but response lost). Every error is ignored:
/// host rollback stays the primary authority and this must never mask it.
fn compensate_backend_root(container_path: &str) {
    if let Ok(roots) = post("library_roots", json!({})) {
        let orphan_id = roots
            .get("roots")
            .and_then(Value::as_array)
            .and_then(|entries| {
                entries
                    .iter()
                    .find(|entry| entry.get("path").and_then(Value::as_str) == Some(container_path))
            })
            .and_then(|entry| entry.get("id").and_then(Value::as_str))
            .map(str::to_string);
        if let Some(id) = orphan_id {
            let _ = post("library_root_remove", json!({ "rootId": id }));
        }
    }
}

/// Rebuild A9's container→host map from the persisted host records. Corrupt
/// state fails closed; stale/offline records are skipped so a missing folder
/// cannot block playback of the remaining, still-resolvable roots.
fn load_host_path_map() -> Result<BTreeMap<String, PathBuf>, CommandError> {
    let records = load_mount_records().map_err(|_| CommandError::source_unavailable())?;
    let download_root = acquisition_root().map_err(|_| CommandError::source_unavailable())?;
    let available = resolvable_records(&records);
    let specs = records_to_specs(&available, &download_root);
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
    // Validate before touching any host state. A bad path yields a fixed error.
    validate_host_path(&path)?;

    let mut records = load_mount_records()?;
    let container_path = format!("/media/root-{}", next_container_index(&records));
    records.push(MountRecord {
        container_path: container_path.clone(),
        host_path: path,
    });

    let outcome = (|| -> Result<Value, CommandError> {
        save_mount_records(&records)?;
        render_and_stage_override(&records)?;
        remount_core()?;
        // The backend runs in `core`, where only the container path exists.
        post(
            "library_root_add",
            json!({ "displayName": display_name, "path": container_path }),
        )
    })();

    if let Err(error) = outcome {
        // Never leave host state ahead of the backend: restore the previous
        // record set, re-stage and remount before returning the original error.
        records.pop();
        // The backend may have applied the add even though its reply was lost;
        // remove that orphan root too (best effort, never replaces `error`).
        compensate_backend_root(&container_path);
        let _ = save_mount_records(&records);
        let _ = render_and_stage_override(&records);
        let _ = remount_core();
        return Err(error);
    }
    outcome
}

#[tauri::command]
pub fn library_root_remove(root_id: String) -> Result<Value, CommandError> {
    let roots = post("library_roots", json!({}))?;
    let container_path = roots
        .get("roots")
        .and_then(Value::as_array)
        .and_then(|entries| {
            entries
                .iter()
                .find(|entry| entry.get("id").and_then(Value::as_str) == Some(root_id.as_str()))
        })
        .and_then(|entry| entry.get("path").and_then(Value::as_str))
        .map(str::to_string);

    if let Some(container_path) = container_path {
        let original = load_mount_records()?;
        let mut records = original.clone();
        let before = records.len();
        // Surviving records keep their stored container paths: no re-indexing.
        records.retain(|record| record.container_path != container_path);
        if records.len() != before {
            let staged = (|| -> Result<(), CommandError> {
                save_mount_records(&records)?;
                render_and_stage_override(&records)?;
                remount_core()
            })();
            if let Err(error) = staged {
                // Mirror add: restore the original list, re-stage and remount
                // before surfacing the original failure.
                let _ = save_mount_records(&original);
                let _ = render_and_stage_override(&original);
                let _ = remount_core();
                return Err(error);
            }
        }
    }
    // Removing an unknown id is still idempotent on the backend side.
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

/// Display-only discovery catalogue (TMDB). No path, token or upstream URL
/// crosses this boundary: posters are relative hashed `/artwork/<hash>` refs.
#[tauri::command]
pub fn discover_catalog(kind: &str, page: u32) -> Result<Value, CommandError> {
    post("discover_catalog", json!({ "kind": kind, "page": page }))
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

/// One-click setup of the bundled sources in the operator's own Prowlarr. The
/// address and key stay in the backend's environment; the renderer sends no
/// credentials and receives only `{ added, skipped }` counts and fixed reasons.
#[tauri::command]
pub fn indexers_seed() -> Result<Value, String> {
    post_internal("host/indexers_seed", json!({}))
        .map_err(|_| "Sources could not be set up. Try again.".to_string())
}

/// Search the operator's enabled Prowlarr indexers for path-free candidates.
/// The renderer sends only a title (plus optional year/season/episode); the host
/// reads the address/key from its own environment. No auto-download or playback.
#[tauri::command]
pub fn source_search(args: Value) -> Result<Value, String> {
    post_internal("host/source_search", args)
        .map_err(|_| "Sources could not be found. Try again.".to_string())
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

/// Fixed command errors: a stored secret must never reach the renderer, not
/// even through an error's `Display` (which may contain an I/O path).
const UNSUPPORTED_PROVIDER: &str = "unsupported provider";
const SECRET_STORE_FAILED: &str = "secret operation failed";

/// Store one of the three optional provider secrets (ADR-0026/0028).
#[tauri::command]
pub fn provider_secret_set(kind: &str, value: &str) -> Result<(), String> {
    let account = account_for(kind).ok_or_else(|| UNSUPPORTED_PROVIDER.to_string())?;
    crate::secret::store_secret(TOKEN_SERVICE, account, value)
        .map_err(|_| SECRET_STORE_FAILED.to_string())
}

/// Report only whether a provider secret is configured; never its value.
#[tauri::command]
pub fn provider_secret_has(kind: &str) -> Result<bool, String> {
    let account = account_for(kind).ok_or_else(|| UNSUPPORTED_PROVIDER.to_string())?;
    crate::secret::load_secret(TOKEN_SERVICE, account)
        .map(|value| value.is_some())
        .map_err(|_| SECRET_STORE_FAILED.to_string())
}

/// Remove a configured provider secret.
#[tauri::command]
pub fn provider_secret_clear(kind: &str) -> Result<(), String> {
    let account = account_for(kind).ok_or_else(|| UNSUPPORTED_PROVIDER.to_string())?;
    crate::secret::delete_secret(TOKEN_SERVICE, account)
        .map_err(|_| SECRET_STORE_FAILED.to_string())
}

/// Start the backend at UI mount. Only a fixed status string crosses this
/// boundary: no path, distro name, env value or engine detail (plan Task 1).
#[tauri::command]
pub fn runtime_start() -> Result<String, String> {
    let distro =
        std::env::var("MEDIAHAVEN_WSL_DISTRO").unwrap_or_else(|_| "MEDIA_HAVEN".to_string());
    let address: std::net::SocketAddr =
        API_ADDR.parse().map_err(|_| "setup-required".to_string())?;
    let compose = stage_compose().map_err(|_| "failed".to_string())?;
    // Host records are the authority on what to mount; stage the override
    // before the supervisor starts so `core` sees the right bind sources.
    let records = load_mount_records().map_err(|_| "failed".to_string())?;
    render_and_stage_override(&records).map_err(|_| "failed".to_string())?;
    let override_path = override_file().map_err(|_| "failed".to_string())?;
    let supervisor = Supervisor::new(distro, compose, address).with_override(override_path);
    // Readiness is detected by the UI's normal API calls; a blocking poll here
    // would hold the command open past the startup deadline.
    let result = supervisor.ensure_running();
    Ok(crate::supervisor::startup_status(&result).to_string())
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

        let records = [MountRecord {
            container_path: "/media/root-0".to_string(),
            host_path: movies.to_string_lossy().into_owned(),
        }];
        let specs = records_to_specs(&records, &downloads);
        let rendered = render_mounts(&specs).expect("override renders");

        // The override fragment has the real read-only acquisition bind.
        assert_eq!(rendered.fragment.matches("/downloads:ro").count(), 1);
        // mpv resolves acquired files through the persisted host map.
        let map = rendered.host_path_map();
        assert_eq!(
            map["/downloads"],
            std::fs::canonicalize(&downloads).unwrap()
        );
        assert_eq!(
            map["/media/root-0"],
            std::fs::canonicalize(&movies).unwrap()
        );

        // With no library roots the acquisition mount is still rendered.
        let empty = records_to_specs(&[], &downloads);
        let only_downloads = render_mounts(&empty).expect("download-only render");
        assert!(only_downloads.host_path_map().contains_key("/downloads"));

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn mount_records_roundtrip_and_corrupt_file_is_fixed_error() {
        let base =
            std::env::temp_dir().join(format!("mh-mount-records-roundtrip-{}", std::process::id()));
        let file = base.join("mounts.json");
        let _ = std::fs::remove_dir_all(&base);

        // A missing file is a legitimate empty mount set.
        assert!(load_mount_records_from(&file)
            .expect("missing file is empty")
            .is_empty());

        let records = vec![
            MountRecord {
                container_path: "/media/root-0".into(),
                host_path: r"C:\Media\Movies".into(),
            },
            MountRecord {
                container_path: "/media/root-7".into(),
                host_path: r"C:\Media\Shows".into(),
            },
        ];
        save_mount_records_to(&file, &records).expect("records save");
        assert_eq!(
            load_mount_records_from(&file).expect("records load"),
            records
        );

        // Corrupt JSON produces a fixed error with no leaked path text.
        std::fs::write(&file, b"{ not json").unwrap();
        let error = load_mount_records_from(&file).expect_err("corrupt file fails");
        assert_eq!(error.code, AppErrorCode::InvalidRequest);
        assert!(!serde_json::to_string(&error).unwrap().contains("Media"));

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn stale_records_are_filtered_while_valid_ones_are_kept() {
        let base = std::env::temp_dir().join(format!("mh-stale-records-{}", std::process::id()));
        let live = base.join("live");
        let stale = base.join("moved-or-offline");
        let file_path = base.join("not-a-directory.txt");
        std::fs::create_dir_all(&live).unwrap();
        std::fs::write(&file_path, b"x").unwrap();

        let records = [
            MountRecord {
                container_path: "/media/root-0".into(),
                host_path: stale.to_string_lossy().into_owned(),
            },
            MountRecord {
                container_path: "/media/root-1".into(),
                host_path: live.to_string_lossy().into_owned(),
            },
            MountRecord {
                container_path: "/media/root-2".into(),
                host_path: file_path.to_string_lossy().into_owned(),
            },
        ];

        let filtered = resolvable_records(&records);
        assert_eq!(
            filtered.len(),
            1,
            "only the live directory survives: {filtered:?}"
        );
        assert_eq!(filtered[0].container_path, "/media/root-1");
        // The filter is pure: the persisted list is left intact for recovery.
        assert_eq!(records.len(), 3);

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn next_container_index_uses_the_highest_existing_suffix() {
        assert_eq!(next_container_index(&[]), 0);
        let records = [
            MountRecord {
                container_path: "/media/root-2".into(),
                host_path: "a".into(),
            },
            MountRecord {
                container_path: "/media/root-0".into(),
                host_path: "b".into(),
            },
            MountRecord {
                container_path: "/downloads".into(),
                host_path: "c".into(),
            },
            MountRecord {
                container_path: "/media/root-not-a-number".into(),
                host_path: "d".into(),
            },
        ];
        assert_eq!(next_container_index(&records), 3);
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
    fn compose_declares_the_tmdb_key_for_core_with_a_blank_safe_default() {
        const COMPOSE: &str = include_str!("../../../../../infrastructure/containers/compose.yaml");
        // Compose injects only variables declared in a service's environment
        // map. The supervisor forwards the key into the process environment,
        // so core must declare it or artwork lookup never sees it in the
        // mandatory container.
        let core = COMPOSE
            .split("  core:")
            .nth(1)
            .and_then(|rest| rest.split("  probe:").next())
            .expect("core service block");
        assert!(
            core.contains(r#"MEDIAHAVEN_TMDB_API_KEY: "${MEDIAHAVEN_TMDB_API_KEY:-}""#),
            "core must declare the TMDB key with a blank-safe default, got:\n{core}"
        );
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
    fn provider_secret_commands_accept_all_three_kinds_without_echoing_a_value() {
        // A blank value fails validation inside the store before any DPAPI
        // access: reaching the fixed store error proves the kind passed the
        // allowlist. No synthetic value is ever written to the host store.
        for kind in ["tmdb", "prowlarr_url", "prowlarr_key"] {
            assert_eq!(
                provider_secret_set(kind, "   ").unwrap_err(),
                SECRET_STORE_FAILED,
                "kind {kind} must pass the allowlist"
            );
        }
        for kind in ["imdb", "prowlarr", "TMDB"] {
            for error in [
                provider_secret_set(kind, "SENTINEL").unwrap_err(),
                provider_secret_has(kind).unwrap_err(),
                provider_secret_clear(kind).unwrap_err(),
            ] {
                assert_eq!(error, UNSUPPORTED_PROVIDER);
                assert!(!error.contains("SENTINEL"));
            }
        }
    }

    #[test]
    fn compose_declares_the_prowlarr_inputs_for_core_with_blank_safe_defaults() {
        let core = COMPOSE_YAML
            .split("  core:")
            .nth(1)
            .and_then(|rest| rest.split("  probe:").next())
            .expect("core service block");
        for line in [
            r#"MEDIAHAVEN_PROWLARR_URL: "${MEDIAHAVEN_PROWLARR_URL:-}""#,
            r#"MEDIAHAVEN_PROWLARR_API_KEY: "${MEDIAHAVEN_PROWLARR_API_KEY:-}""#,
        ] {
            assert!(
                core.contains(line),
                "core must declare {line}, got:\n{core}"
            );
        }
    }

    #[test]
    fn staged_compose_is_an_absolute_existing_file() {
        let path = stage_compose().expect("stage compose to a temp file");
        assert!(
            path.is_absolute(),
            "compose path must be absolute: {path:?}"
        );
        assert!(path.is_file(), "compose file must exist: {path:?}");
        let text = std::fs::read_to_string(&path).expect("read staged compose");
        assert!(text.contains("services:"));
        assert!(text.contains("MEDIAHAVEN_PROWLARR_API_KEY"));
        let _ = std::fs::remove_file(&path);
    }
}
