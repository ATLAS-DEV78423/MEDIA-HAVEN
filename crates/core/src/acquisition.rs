//! Host-internal acquisition routes (ADR-0018).
//!
//! The qBittorrent WebUI is reached only from `core`, which is the sole
//! container on the `acquire` network. These routes carry the Windows host's
//! `download_*` commands to that client. They are deliberately NOT renderer
//! commands: they live in this module under `/host/` instead of `routes.rs`,
//! so the 24-command POST parity surface (renderer commands only) is unchanged.
//! They still pass the signed auth middleware exactly like `/readyz`.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use app_core::domain::{
    AppError, AppErrorCode, BrowseQuery, BrowseSort, CleanupEntry, DownloadState, MediaAvailability,
};
use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use media_acquisition::{
    missing_episodes, next_two_in_season, reconcile, AcquireError, CleanupCandidate,
    CleanupDecision, CleanupError, CleanupState, CleanupWorker, EpisodeRef, ManagedFile,
    PrefetchScheduler, QbClient, TorrentApi, DEFAULT_BUFFER_LIMIT_BYTES, REPLAY_GRACE, SAVE_PATH,
};
use media_persistence::sqlite::{CleanupRepository, ManagedFileRow};
use serde::Deserialize;
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

/// This build maps no user-supplied source to an upcoming episode: the app
/// bundles no trackers/indexers/catalogues (ADR-0018) and Task 4 accepts only
/// sources the operator supplies. Returning `None` degrades honestly rather
/// than inventing a source. Wiring through `acquire` is exercised in unit tests.
fn source_for(_media_id: &str) -> Option<String> {
    None
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
    if !missing.iter().any(|id| source_for(id).is_some()) {
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
        let source = source_for(id).ok_or(AcquireError::InvalidSource)?;
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
        ] {
            assert!(path.starts_with(HOST_PREFIX), "host-only path: {path}");
        }
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

    #[test]
    fn no_lawful_source_is_invented_for_prefetch() {
        assert!(source_for("s1e2").is_none());
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
}
