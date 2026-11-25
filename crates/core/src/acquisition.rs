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
    MediaAvailability, MediaDetail, MAX_ITEM_SOURCE_LEN,
};
use app_core::ports::LibraryRepository;
use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use media_acquisition::{
    inspect_with_runner, missing_episodes, next_two_in_season, reconcile, AcquireError,
    CleanupCandidate, CleanupDecision, CleanupError, CleanupState, CleanupWorker, EpisodeRef,
    InspectionState, ManagedFile, PrefetchScheduler, ProbeRunner, QbClient, TorrentApi,
    DEFAULT_BUFFER_LIMIT_BYTES, REPLAY_GRACE, SAVE_PATH,
};
use media_persistence::sqlite::{
    connect_path, CleanupRepository, ManagedFileRow, SqliteLibraryRepository,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Digest;

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
const CATALOG_WANTED_ADD_PATH: &str = "/host/catalog_wanted_add";
const DEFENDER_STATE_PATH: &str = "/host/defender_state";
const DEFENDER_RESULT_PATH: &str = "/host/defender_result";

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

/// Host-only Defender gate input. No filesystem path is accepted: the Windows
/// host resolves the media id to a host path itself and sends back only a
/// closed state spelling.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DefenderQueryArgs {
    media_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DefenderResultArgs {
    media_id: String,
    state: String,
}

/// A catalog pick promoted to a "wanted" library item (ADR-0024). Display-only
/// fields; no path or credential ever enters this shape.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CatalogWantedAddArgs {
    title: String,
    year: Option<i64>,
    kind: String,
}

/// Host-internal probe result for a completed download. The Windows host runs
/// `ffprobe` (it owns the Docker CLI); `core` only records the closed state
/// spelling. No path or probe output crosses this boundary.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InspectArgs {
    media_id: String,
    state: String,
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
        .route(CATALOG_WANTED_ADD_PATH, post(catalog_wanted_add))
        .route(DEFENDER_STATE_PATH, post(defender_state))
        .route(DEFENDER_RESULT_PATH, post(defender_result))
}

/// Attach an operator-supplied source to a library item (R-01 / ADR-0021).
/// Host-internal: the renderer reaches it through a named command only. The
/// source is validated again here (trust boundary) and never returned to a DTO.
///
/// Attaching a source is not a passive record: the operator confirmed this
/// source *for this item*, so the acquisition must actually start. The source
/// is recorded (so the rolling buffer and cleanup can see it) **and** handed to
/// the downloader, with the download row persisted against this media id.
/// Without the acquisition, a confirmed magnet was inert — the item could
/// never become playable.
fn check_source_len(source: &str) -> Result<(), ApiError> {
    if source.len() > MAX_ITEM_SOURCE_LEN {
        Err(cleanup_invalid("source is too long."))
    } else {
        Ok(())
    }
}

async fn source_set(
    State(state): State<AppState>,
    ApiJson(args): ApiJson<SourceSetArgs>,
) -> Result<Json<Value>, ApiError> {
    if args.media_id.trim().is_empty() || args.source.trim().is_empty() {
        return Err(cleanup_invalid("mediaId and source are required."));
    }
    check_source_len(&args.source)?;
    state
        .service
        .set_item_source(&args.media_id, &args.source)
        .await
        .map_err(ApiError)?;
    // Start the acquisition for the item the source was confirmed for. The
    // source is bound to the item here, so the persisted download row carries
    // the media id and the playability gates can see the association.
    let item = acquire_for_media(&state, &args.media_id, &args.source).await?;
    Ok(Json(serde_json::json!({ "downloadId": item.id })))
}

/// Acquire a source on behalf of a media item and persist the download rows
/// against that item.
///
/// The qB client is synchronous with 10 s socket timeouts, so the blocking
/// call runs on the blocking pool: it must never stall a tokio worker (the
/// same reasoning as the prefetch scheduler).
///
/// The persisted rows are the ones the rest of the app reads: `download_items`
/// carries the media id, the container path and the verification state, so the
/// probe/Defender gates and the cleanup worker can both see the association.
async fn acquire_for_media(
    state: &AppState,
    media_id: &str,
    source: &str,
) -> Result<app_core::domain::DownloadItem, ApiError> {
    let acquisition = state.acquisition.clone();
    let media_id = media_id.to_string();
    let source = source.to_string();
    let source_for_api = source.clone();
    let item = tokio::task::spawn_blocking(move || {
        acquisition.acquire_for(&source_for_api, Some(&media_id))
    })
    .await
    .map_err(|_| {
        ApiError(AppError::new(
            AppErrorCode::BackendUnreachable,
            "The acquisition task did not complete.",
            true,
        ))
    })?
    .map_err(map_error)?;

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
            .download_add(&item, path, &source, SAVE_PATH)
            .await?;
    }
    // Bridge the orchestrator's inspection result into SQLite so the
    // playability gate can read it. Persist by download id (= torrent hash);
    // the gate reads it back through the media id bound above.
    persist_inspection(state, &item.id).await?;
    Ok(item)
}

/// Bridge the orchestrator's in-memory inspection result into SQLite. Called
/// after add/source-set and after every refresh so a torrent that becomes
/// ready during polling is gated by the same stored probe state.
async fn persist_inspection(state: &AppState, download_id: &str) -> Result<(), ApiError> {
    if let Some(inspection) = state.acquisition.inspection_state(download_id) {
        let detail = (inspection != InspectionState::Clean)
            .then(|| format!("probe: {}", inspection.as_str()));
        verify_repo()
            .await?
            .set_verification_state(download_id, inspection.as_str(), detail.as_deref())
            .await?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Catalog "wanted" item (ADR-0024). A pick becomes an unavailable media_items
// row with a stable id so the existing source_set -> playback chain has a
// mediaId to act on. All input is validated here at the host trust boundary.
// ---------------------------------------------------------------------------

/// Reject kinds outside the library schema and hostile/path-like titles before
/// anything reaches storage or a display surface. A colon or interior dots are
/// safe (real TMDB titles use both): the id is a content hash and the title is
/// bound as a DB value, never a path. Path separators and a leading dot remain
/// rejected.
fn validate_wanted(title: &str, kind: &str) -> Result<(), ApiError> {
    if !matches!(kind, "movie" | "series") {
        return Err(cleanup_invalid("kind must be movie or series."));
    }
    if title.trim().is_empty()
        || title.len() > 512
        || title.chars().any(char::is_control)
        || title.starts_with('.')
        || title.chars().any(|c| matches!(c, '/' | '\\'))
    {
        return Err(cleanup_invalid("A valid title is required."));
    }
    Ok(())
}

/// Stable, content-addressed id: `catalog-{kind}-{sha256(title|year)[..32]}`.
fn catalog_wanted_id(title: &str, year: Option<i64>, kind: &str) -> String {
    let digest = hex::encode(sha2::Sha256::digest(
        format!("{}|{}", title, year.unwrap_or(0)).as_bytes(),
    ));
    format!("catalog-{}-{}", kind, &digest[..32])
}

async fn catalog_wanted_add(
    State(_state): State<AppState>,
    ApiJson(args): ApiJson<CatalogWantedAddArgs>,
) -> Result<Json<Value>, ApiError> {
    validate_wanted(&args.title, &args.kind)?;
    let id = catalog_wanted_id(&args.title, args.year, &args.kind);
    // Same library.db the service uses; reuse the lazy host-internal repo.
    verify_repo()
        .await?
        .wanted_add(&args.title, args.year, &args.kind, &id)
        .await?;
    Ok(Json(serde_json::json!({ "mediaId": id })))
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

/// The host resolves media ids to host paths; this boundary accepts only a
/// short, path-free identifier. It never accepts a path or a scanner detail.
fn validate_defender_media_id(media_id: &str) -> Result<(), ApiError> {
    if media_id.trim().is_empty()
        || media_id.len() > 256
        || media_id.chars().any(char::is_control)
        || media_id.chars().any(|c| matches!(c, '/' | '\\' | ':'))
    {
        return Err(cleanup_invalid("A valid mediaId is required."));
    }
    Ok(())
}

/// Read the stored Defender gate for the Windows host. `state: null` means a
/// library import with no download row, which needs no scan.
async fn defender_state(
    State(_state): State<AppState>,
    ApiJson(args): ApiJson<DefenderQueryArgs>,
) -> Result<Json<Value>, ApiError> {
    validate_defender_media_id(&args.media_id)?;
    let state = verify_repo()
        .await?
        .defender_state_for_media(&args.media_id)
        .await?;
    encode(serde_json::json!({ "state": state }))
}

/// Record the result of a host-side Windows Defender scan. The state is a
/// closed enum spelling; no path, output text or arbitrary state is accepted.
async fn defender_result(
    State(_state): State<AppState>,
    ApiJson(args): ApiJson<DefenderResultArgs>,
) -> Result<Json<Value>, ApiError> {
    validate_defender_media_id(&args.media_id)?;
    if !matches!(
        args.state.as_str(),
        "clean" | "threat" | "unavailable" | "not_scanned"
    ) {
        return Err(cleanup_invalid("An unknown Defender state was rejected."));
    }
    verify_repo()
        .await?
        .set_defender_verification_for_media(&args.media_id, &args.state, None)
        .await?;
    Ok(Json(Value::Null))
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
    persist_inspection(&state, &item.id).await?;
    encode(item)
}

async fn status(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    // qB's client is synchronous and bounded; keep it off the tokio workers.
    // Without refresh the status route replayed the state captured at add
    // time forever, so a completed torrent was never Ready or inspected.
    let acquisition = state.acquisition.clone();
    let names_acquisition = state.acquisition.clone();
    let (items, file_names) = tokio::task::spawn_blocking(move || {
        let items = acquisition.refresh();
        // qB may not know member names at add time. Once ready, replace the
        // placeholder path with the real container paths so the playback
        // source lookup can resolve the file through the host mount map.
        let file_names = items
            .iter()
            .filter(|item| item.state == DownloadState::Ready)
            .filter_map(|item| {
                names_acquisition
                    .files(&item.torrent_hash)
                    .ok()
                    .filter(|files| !files.is_empty())
                    .map(|files| (item.id.clone(), files))
            })
            .collect::<Vec<_>>();
        (items, file_names)
    })
    .await
    .map_err(|_| {
        ApiError(AppError::new(
            AppErrorCode::BackendUnreachable,
            "The acquisition status task did not complete.",
            true,
        ))
    })?;
    for item in &items {
        state.service.download_update(item).await?;
        persist_inspection(&state, &item.id).await?;
        if item.state == DownloadState::Ready {
            if let Some(media_id) = &item.media_id {
                let repo = verify_repo().await?;
                if repo.gates_clean(media_id).await? {
                    repo.set_media_available(media_id).await?;
                }
            }
        }
    }
    for (id, files) in file_names {
        let paths = files
            .iter()
            .map(|name| format!("{SAVE_PATH}/{}", name.trim_start_matches('/')))
            .collect::<Vec<_>>();
        state.service.download_set_paths(&id, &paths).await?;
    }
    encode(items)
}

async fn remove(
    State(state): State<AppState>,
    ApiJson(args): ApiJson<RemoveArgs>,
) -> Result<Json<Value>, ApiError> {
    state.acquisition.remove(&args.id).map_err(map_error)?;
    // Removing only from the in-memory qB tracker makes the task disappear
    // until the next restart, when the persisted row is restored again.
    // Delete the persisted acquisition row in the same operation.
    state
        .service
        .download_remove(&args.id)
        .await
        .map_err(ApiError)?;
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
    State(state): State<AppState>,
    ApiJson(args): ApiJson<InspectArgs>,
) -> Result<Json<Value>, ApiError> {
    let media_id = args.media_id.trim();
    if media_id.is_empty() || media_id.contains('/') || media_id.contains('\\') {
        return Err(cleanup_invalid("mediaId is required."));
    }
    let state_text = match args.state.as_str() {
        "clean" | "suspicious" | "unavailable" => args.state.as_str(),
        _ => return Err(cleanup_invalid("state is not recognised.")),
    };
    let item = state
        .acquisition
        .status()
        .into_iter()
        .find(|item| item.media_id.as_deref() == Some(media_id))
        .ok_or_else(|| cleanup_invalid("No download is bound to this item."))?;
    let detail = (state_text != "clean").then(|| format!("probe: {state_text}"));
    verify_repo()
        .await?
        .set_verification_state(&item.id, state_text, detail.as_deref())
        .await
        .map_err(ApiError)?;
    Ok(Json(serde_json::json!({ "state": state_text })))
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
            CATALOG_WANTED_ADD_PATH,
        ] {
            assert!(path.starts_with(HOST_PREFIX), "host-only path: {path}");
        }
    }

    #[test]
    fn source_len_follows_the_shared_boundary() {
        let long = "magnet:?xt=".to_string() + &"a".repeat(6000);
        assert!(check_source_len(&long).is_ok());
        let too_long = "magnet:?xt=".to_string() + &"a".repeat(MAX_ITEM_SOURCE_LEN);
        assert!(check_source_len(&too_long).is_err());
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

    /// Post a signed JSON body exactly as the host does. The fake token is the
    /// shared secret the middleware checks; no live service or network is used.
    async fn post_signed(
        router: &axum::Router,
        token: &str,
        path: &str,
        body: serde_json::Value,
    ) -> (axum::http::StatusCode, serde_json::Value) {
        use axum::body::{to_bytes, Body};
        use axum::http::{header, Request};
        use std::time::{SystemTime, UNIX_EPOCH};
        use tower::ServiceExt;

        let text = body.to_string();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_secs()
            .to_string();
        let signature = crate::auth::sign(token, &timestamp, "POST", path, text.as_bytes());
        let request = Request::builder()
            .method("POST")
            .uri(path)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(crate::auth::HEADER_TIMESTAMP, &timestamp)
            .header(crate::auth::HEADER_SIGNATURE, &signature)
            .body(Body::from(text))
            .expect("build signed request");
        let response = router
            .clone()
            .oneshot(request)
            .await
            .expect("route responds");
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 1024 * 1024)
            .await
            .expect("read body");
        let wire = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        (status, wire)
    }

    #[tokio::test]
    async fn catalog_wanted_add_route_is_signed_stable_and_validated() {
        use app_core::application::LibraryService;

        // Point the lazy host repo at a scratch DB so the signed route writes
        // nowhere near real user data. This test is the only caller of
        // verify_repo() in this binary, so its OnceLock opens this DB.
        let dir = std::env::temp_dir().join(format!("mh-wanted-route-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::env::set_var("MEDIAHAVEN_DATA_DIR", &dir);
        let pool = connect_path(&dir.join("library.db"))
            .await
            .expect("connect and migrate");
        let service = LibraryService::new(SqliteLibraryRepository::new(pool.clone()));
        let router = crate::server::build_router(AppState::new(service, "test-token"));

        let body = serde_json::json!({ "title": "Arrival", "year": 2016, "kind": "movie" });
        let (status, wire) =
            post_signed(&router, "test-token", CATALOG_WANTED_ADD_PATH, body.clone()).await;
        assert_eq!(status, axum::http::StatusCode::OK);
        let expected = catalog_wanted_id("Arrival", Some(2016), "movie");
        assert_eq!(wire["mediaId"], serde_json::json!(expected));
        assert_eq!(
            expected, "catalog-movie-91f4d2cff0e9b19e24c33e21b4a7f800",
            "stable id uses the exact contract formula"
        );

        // Repeating the pick returns the same id and leaves one, file-less row
        // that is honestly reported as missing rather than available.
        let (status2, wire2) =
            post_signed(&router, "test-token", CATALOG_WANTED_ADD_PATH, body).await;
        assert_eq!(status2, axum::http::StatusCode::OK);
        assert_eq!(wire2["mediaId"], serde_json::json!(expected));
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM media_items WHERE id = ?")
            .bind(&expected)
            .fetch_one(&pool)
            .await
            .expect("count media");
        assert_eq!(count, 1);
        let avail: String =
            sqlx::query_scalar("SELECT availability_state FROM media_items WHERE id = ?")
                .bind(&expected)
                .fetch_one(&pool)
                .await
                .expect("read state");
        assert_eq!(avail, "missing-file");
        let files: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM media_files WHERE media_id = ?")
            .bind(&expected)
            .fetch_one(&pool)
            .await
            .expect("count files");
        assert_eq!(files, 0);

        // A colon-bearing title is a real catalog title (e.g. "Star Trek:
        // Discovery"). The id is a content hash and the title is bound as a DB
        // value, so ':' is safe and produces a stable id.
        let colon = "Star Trek: Discovery";
        let colon_body = serde_json::json!({ "title": colon, "year": 2017, "kind": "series" });
        let (colon_status, colon_wire) = post_signed(
            &router,
            "test-token",
            CATALOG_WANTED_ADD_PATH,
            colon_body.clone(),
        )
        .await;
        assert_eq!(
            colon_status,
            axum::http::StatusCode::OK,
            "colon in title must not be rejected"
        );
        assert_eq!(
            colon_wire["mediaId"],
            serde_json::json!(catalog_wanted_id(colon, Some(2017), "series"))
        );
        let (colon_status2, colon_wire2) =
            post_signed(&router, "test-token", CATALOG_WANTED_ADD_PATH, colon_body).await;
        assert_eq!(colon_status2, axum::http::StatusCode::OK);
        assert_eq!(colon_wire2["mediaId"], colon_wire["mediaId"]);

        // Interior dots are a real catalog pattern ("The Movie ... Final Cut")
        // and must be accepted; only separators/leading dot are path-like.
        let dots = "The Movie ... Final Cut";
        let dots_body = serde_json::json!({ "title": dots, "year": 2024, "kind": "movie" });
        let (dots_status, dots_wire) =
            post_signed(&router, "test-token", CATALOG_WANTED_ADD_PATH, dots_body).await;
        assert_eq!(dots_status, axum::http::StatusCode::OK);
        assert_eq!(
            dots_wire["mediaId"],
            serde_json::json!(catalog_wanted_id(dots, Some(2024), "movie"))
        );

        // The Defender gate travels through the same signed host boundary.
        // No path is accepted: the host resolves the id locally and stores a
        // closed state spelling that playback_start can enforce.
        sqlx::query(
            "INSERT INTO torrent_tasks \
             (hash, magnet_or_source, state, progress, save_path, created_at, updated_at) \
             VALUES ('h-def-gate', 'magnet:?xt=urn:btih:defgate', 'downloading', 0.5, \
             '/downloads', '', '')",
        )
        .execute(&pool)
        .await
        .expect("insert defender torrent task");
        sqlx::query(
            "INSERT INTO download_items \
             (id, torrent_hash, media_id, file_path, state, created_at) \
             VALUES ('d-def-gate', 'h-def-gate', ?, '/downloads/movie.mkv', 'downloading', '')",
        )
        .bind(&expected)
        .execute(&pool)
        .await
        .expect("insert defender download row");

        let (result_status, _) = post_signed(
            &router,
            "test-token",
            DEFENDER_RESULT_PATH,
            serde_json::json!({ "mediaId": expected, "state": "clean" }),
        )
        .await;
        assert_eq!(result_status, axum::http::StatusCode::OK);
        let (state_status, state_wire) = post_signed(
            &router,
            "test-token",
            DEFENDER_STATE_PATH,
            serde_json::json!({ "mediaId": expected }),
        )
        .await;
        assert_eq!(state_status, axum::http::StatusCode::OK);
        assert_eq!(state_wire["state"], serde_json::json!("clean"));

        let (bad_state_status, _) = post_signed(
            &router,
            "test-token",
            DEFENDER_RESULT_PATH,
            serde_json::json!({ "mediaId": expected, "state": "definitely-clean" }),
        )
        .await;
        assert_eq!(
            bad_state_status,
            axum::http::StatusCode::BAD_REQUEST,
            "only the closed Defender state set may be stored"
        );

        // The probe result travels through the same closed boundary: only the
        // three known states, and never a path-like media id, may be stored.
        for bad in [
            serde_json::json!({ "mediaId": expected, "state": "definitely-clean" }),
            serde_json::json!({ "mediaId": "../etc/passwd", "state": "clean" }),
            serde_json::json!({ "mediaId": "C:\\movies\\film", "state": "clean" }),
        ] {
            let (status, _) = post_signed(&router, "test-token", INSPECT_PATH, bad).await;
            assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
        }

        // Bad kind and hostile/path-like titles are refused at the boundary.
        for bad in [
            serde_json::json!({ "title": "Arrival", "year": 2016, "kind": "tv" }),
            serde_json::json!({ "title": "../etc/passwd", "year": 2016, "kind": "movie" }),
            serde_json::json!({ "title": "C:\\movies\\film", "year": 2016, "kind": "movie" }),
            serde_json::json!({ "title": ".hidden", "year": 2016, "kind": "movie" }),
            serde_json::json!({ "title": "", "year": 2016, "kind": "movie" }),
        ] {
            let (status, _) =
                post_signed(&router, "test-token", CATALOG_WANTED_ADD_PATH, bad).await;
            assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
        }

        std::env::remove_var("MEDIAHAVEN_DATA_DIR");
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
