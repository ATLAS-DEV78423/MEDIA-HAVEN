//! Acquisition orchestration: validation, duplicate prevention and the
//! add → persist → status flow (spec R-11, Task 4).
//!
//! The orchestrator is generic over a `TorrentApi` (qBittorrent in production,
//! a fake in tests) and a `DownloadStore`. It is deliberately synchronous and
//! path-free: no host path or credential is ever produced here.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use app_core::domain::{DownloadItem, DownloadState};

use crate::inspection::{
    combine_inspection, inspect_completed, DockerProbeRunner, InspectionState, ProbeRunner,
};
use crate::qbittorrent::{QbClient, TorrentApi};

/// Container destination for user-supplied acquisitions (storage option B).
/// The host directory behind it is resolved by the A9 mount map, never here.
pub const SAVE_PATH: &str = "/downloads";

/// Closed cap on a source string. Real magnets with several trackers are
/// ~2.2k characters, so 2048 rejected valid operator sources; anything larger
/// than 8192 is still rejected rather than buffered.
const MAX_SOURCE_LEN: usize = 8192;
const MAGNET_PREFIX: &str = "magnet:?xt=urn:btih:";
const TORRENT_SUFFIX: &str = ".torrent";

/// Closed error set for the trust boundary and the orchestration flow.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AcquireError {
    #[error("a source is required")]
    EmptySource,
    #[error("the source is too long")]
    SourceTooLong,
    #[error("file paths are not accepted as a source")]
    PathRejected,
    #[error("only a magnet URI or a .torrent file name is accepted")]
    InvalidSource,
    #[error("the acquisition backend is unavailable")]
    Backend,
    #[error("the download was not found")]
    NotFound,
}

/// Validate an untrusted renderer string at the host trust boundary.
///
/// Accepted: a `magnet:?xt=urn:btih:` URI or a **bare** `*.torrent` file name.
/// Rejected: empty, oversized, control characters, and anything that looks like
/// a host path (separators, drive letters, `..` traversal, leading dot).
pub fn validate_source(raw: &str) -> Result<String, AcquireError> {
    let source = raw.trim();
    if source.is_empty() {
        return Err(AcquireError::EmptySource);
    }
    if source.len() > MAX_SOURCE_LEN {
        return Err(AcquireError::SourceTooLong);
    }
    if source.chars().any(char::is_control) {
        return Err(AcquireError::InvalidSource);
    }

    let lower = source.to_ascii_lowercase();
    if lower.starts_with(MAGNET_PREFIX) {
        // Magnets legitimately contain `://` in tracker parameters, so path
        // characters are not rejected here.
        return Ok(source.to_string());
    }
    if lower.ends_with(TORRENT_SUFFIX) {
        if source.len() == TORRENT_SUFFIX.len() {
            return Err(AcquireError::InvalidSource);
        }
        if source.chars().any(|c| matches!(c, '/' | '\\' | ':'))
            || source.contains("..")
            || source.starts_with('.')
        {
            return Err(AcquireError::PathRejected);
        }
        return Ok(source.to_string());
    }
    Err(AcquireError::InvalidSource)
}

/// The repository surface the orchestrator needs. `download_exists` resolves
/// the interface gap left by Task 1 (spec R-11 duplicate prevention).
pub trait DownloadStore {
    fn download_exists(&self, source: &str) -> Option<DownloadItem>;
    fn download_insert(&self, item: &DownloadItem, source: &str);
    fn download_status(&self, id: &str) -> Option<DownloadItem>;
    fn download_list(&self) -> Vec<DownloadItem>;
    /// Replace one tracked item's state/progress. Default no-op keeps other
    /// stores compiling; the in-memory store implements it so a status poll
    /// can reflect the engine's real progress.
    fn download_update(&self, _item: &DownloadItem) {}
    fn download_remove(&self, id: &str);
    /// Completion-path inspection result. Default no-op keeps other stores
    /// compiling; the in-memory store records it so the gate can read it.
    fn record_inspection(&self, _id: &str, _state: InspectionState) {}
    fn inspection_state(&self, _id: &str) -> Option<InspectionState> {
        None
    }
}

#[derive(Clone)]
struct Stored {
    item: DownloadItem,
    source: String,
}

/// Host-side in-memory store. Durable persistence lives in the backend's
/// SQLite repository; this keeps the named commands testable without a live
/// qBittorrent container (live acquisition is NOT RUN in this task).
#[derive(Default)]
pub struct InMemoryDownloadStore {
    stored: Mutex<Vec<Stored>>,
    inspection: Mutex<HashMap<String, InspectionState>>,
}

impl DownloadStore for InMemoryDownloadStore {
    fn download_exists(&self, source: &str) -> Option<DownloadItem> {
        self.stored
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .find(|entry| entry.source == source)
            .map(|entry| entry.item.clone())
    }

    fn download_insert(&self, item: &DownloadItem, source: &str) {
        let mut stored = self.stored.lock().unwrap_or_else(PoisonError::into_inner);
        if !stored.iter().any(|entry| entry.item.id == item.id) {
            stored.push(Stored {
                item: item.clone(),
                source: source.to_string(),
            });
        }
    }

    fn download_status(&self, id: &str) -> Option<DownloadItem> {
        self.stored
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .find(|entry| entry.item.id == id)
            .map(|entry| entry.item.clone())
    }

    fn download_list(&self) -> Vec<DownloadItem> {
        self.stored
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|entry| entry.item.clone())
            .collect()
    }

    fn download_update(&self, item: &DownloadItem) {
        let mut stored = self.stored.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(entry) = stored.iter_mut().find(|entry| entry.item.id == item.id) {
            entry.item.state = item.state;
            entry.item.progress = item.progress;
            entry.item.media_id = item.media_id.clone();
        }
    }

    fn download_remove(&self, id: &str) {
        self.stored
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|entry| entry.item.id != id);
        self.inspection
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(id);
    }

    fn record_inspection(&self, id: &str, state: InspectionState) {
        self.inspection
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(id.to_string(), state);
    }

    fn inspection_state(&self, id: &str) -> Option<InspectionState> {
        self.inspection
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(id)
            .copied()
    }
}

/// Drives acquisition through a fakeable `TorrentApi` and `DownloadStore`.
pub struct Orchestrator<A: TorrentApi, S: DownloadStore> {
    api: A,
    store: S,
    probe: Box<dyn ProbeRunner + Send + Sync>,
    /// Only test-injected probes may auto-inspect. The host owns the real gate.
    auto_inspect: bool,
    /// Per-file probe results for completed downloads (host-internal only).
    inspection: Mutex<HashMap<String, Vec<(String, InspectionState)>>>,
}

impl<A: TorrentApi, S: DownloadStore> Orchestrator<A, S> {
    pub fn new(api: A, store: S) -> Self {
        Self {
            api,
            store,
            probe: Box::new(DockerProbeRunner),
            auto_inspect: false,
            inspection: Mutex::new(HashMap::new()),
        }
    }

    /// Inject a probe for tests. Production uses `DockerProbeRunner` by default.
    pub fn with_probe_runner(mut self, runner: impl ProbeRunner + Send + Sync + 'static) -> Self {
        self.probe = Box::new(runner);
        self.auto_inspect = true;
        self
    }

    /// The inspection result recorded for a completed download id.
    pub fn inspection_state(&self, id: &str) -> Option<InspectionState> {
        self.store.inspection_state(id)
    }

    /// The per-file results, used by the host to record detail without paths
    /// entering any renderer DTO (container paths are host-internal only).
    pub fn inspection_results(&self, id: &str) -> Vec<(String, InspectionState)> {
        self.inspection
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(id)
            .cloned()
            .unwrap_or_default()
    }

    /// Completion-path trigger (Task 1 finding 1). qB reports `Ready` here; a
    /// finished download is inspected immediately instead of staying pending
    /// forever. Any failure is unplayable — never a silent Clean.
    fn inspect_if_ready(&self, item: &DownloadItem) {
        if !self.auto_inspect || item.state != DownloadState::Ready {
            return;
        }
        let paths: Vec<String> = self
            .api
            .files(&item.torrent_hash)
            .unwrap_or_default()
            .into_iter()
            .map(|name| format!("{SAVE_PATH}/{}", name.trim_start_matches('/')))
            .collect();
        let probe: &dyn ProbeRunner = self.probe.as_ref();
        let results = inspect_completed(probe, &paths);
        let combined = combine_inspection(&results);
        self.inspection
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(item.id.clone(), results);
        self.store.record_inspection(&item.id, combined);
    }

    /// Add a user-supplied source once. A repeat of the same source returns the
    /// stored item without calling `TorrentApi::add` again (spec R-11).
    pub fn acquire(&self, raw: &str) -> Result<DownloadItem, AcquireError> {
        self.acquire_for(raw, None)
    }

    /// Add a source and bind it to a library item.
    ///
    /// The media id is attached to the returned item so the caller can persist
    /// the download row against the item it was confirmed for (R-01). The
    /// in-memory store keeps the *unbound* item: the same source may later be
    /// confirmed for a second item, and the store's source→item map must stay
    /// one-to-one (spec R-11).
    pub fn acquire_for(
        &self,
        raw: &str,
        media_id: Option<&str>,
    ) -> Result<DownloadItem, AcquireError> {
        let source = validate_source(raw)?;
        if let Some(existing) = self.store.download_exists(&source) {
            return Ok(DownloadItem {
                media_id: media_id.map(str::to_string).or(existing.media_id),
                ..existing
            });
        }
        let hash = self
            .api
            .add(&source, SAVE_PATH)
            .map_err(|_| AcquireError::Backend)?;
        let status = self.api.status(&hash).map_err(|_| AcquireError::Backend)?;
        let item = DownloadItem {
            id: hash.clone(),
            torrent_hash: hash,
            media_id: media_id.map(str::to_string),
            state: map_state(&status.state),
            progress: status.progress,
        };
        self.store.download_insert(&item, &source);
        self.inspect_if_ready(&item);
        Ok(item)
    }

    /// Rebuild the in-memory tracker from a persisted row after a core restart.
    /// No qB call is made: the next status poll reconciles live state.
    pub fn restore(&self, item: DownloadItem, source: &str) {
        self.store.download_insert(&item, source);
    }

    /// Host-internal container file names for a torrent. Used by `core` to
    /// build `/downloads/...` mapping paths for persistence; never a DTO field.
    pub fn files(&self, hash: &str) -> Result<Vec<String>, AcquireError> {
        self.api.files(hash).map_err(|_| AcquireError::Backend)
    }

    /// All tracked downloads (path-free identities only).
    pub fn status(&self) -> Vec<DownloadItem> {
        self.store.download_list()
    }

    /// Re-read every tracked download's state from the engine and update the
    /// store. Without this a download's state and progress are frozen at the
    /// values captured when it was added — the UI shows "Downloading 0%" for
    /// ever and a completed torrent is never seen as ready.
    ///
    /// Returns the refreshed list. A per-item status failure leaves that item
    /// as stored: one unreachable torrent must not wipe the tracker's state.
    pub fn refresh(&self) -> Vec<DownloadItem> {
        for item in self.store.download_list() {
            if let Ok(status) = self.api.status(&item.torrent_hash) {
                let mut updated = item.clone();
                updated.state = map_state(&status.state);
                updated.progress = status.progress;
                if updated.state != item.state
                    || (updated.progress - item.progress).abs() > f64::EPSILON
                {
                    self.store.download_update(&updated);
                    if updated.state == DownloadState::Ready && item.state != DownloadState::Ready {
                        self.inspect_if_ready(&updated);
                    }
                }
            }
        }
        self.store.download_list()
    }

    /// One tracked download by id.
    pub fn download_status(&self, id: &str) -> Option<DownloadItem> {
        self.store.download_status(id)
    }

    /// Remove a task but keep its files: coordinated, app-managed deletion is
    /// Task 6's job, and source media is never deleted here.
    pub fn remove(&self, id: &str) -> Result<(), AcquireError> {
        let item = self
            .store
            .download_status(id)
            .ok_or(AcquireError::NotFound)?;
        self.api
            .remove(&item.torrent_hash, false)
            .map_err(|_| AcquireError::Backend)?;
        self.store.download_remove(id);
        Ok(())
    }
}

/// Concrete host/backend state: a lazily created qB client plus the store.
/// `QbClient::from_env()` resolves the container DNS name only where it exists
/// (inside `core` on the `acquire` network), so construction is deferred until
/// the first add and fails closed elsewhere.
pub struct AcquisitionState {
    orchestrator: Mutex<Option<Orchestrator<QbClient, InMemoryDownloadStore>>>,
}

impl AcquisitionState {
    pub fn new() -> Self {
        Self {
            orchestrator: Mutex::new(None),
        }
    }

    /// Initialise on first use, then run `work` against the orchestrator while
    /// the lock is held. `MutexGuard::map` is not stable on this toolchain.
    fn with<T>(
        &self,
        work: impl FnOnce(&Orchestrator<QbClient, InMemoryDownloadStore>) -> Result<T, AcquireError>,
    ) -> Result<T, AcquireError> {
        let mut guard = self
            .orchestrator
            .lock()
            .map_err(|_| AcquireError::Backend)?;
        if guard.is_none() {
            let client = QbClient::from_env().map_err(|_| AcquireError::Backend)?;
            *guard = Some(Orchestrator::new(client, InMemoryDownloadStore::default()));
        }
        let orchestrator = guard.as_ref().ok_or(AcquireError::Backend)?;
        work(orchestrator)
    }

    pub fn acquire(&self, raw: &str) -> Result<DownloadItem, AcquireError> {
        self.with(|orchestrator| orchestrator.acquire(raw))
    }

    /// Acquire a source bound to a library item (R-01). The item id lets the
    /// caller persist the download row against the item the operator confirmed
    /// it for, which is what makes the file playable for that item.
    pub fn acquire_for(
        &self,
        raw: &str,
        media_id: Option<&str>,
    ) -> Result<DownloadItem, AcquireError> {
        self.with(|orchestrator| orchestrator.acquire_for(raw, media_id))
    }

    /// Restore one persisted acquisition row. A restart must not make tracked
    /// downloads disappear from status/refresh until they are re-added.
    pub fn restore(&self, item: DownloadItem, source: &str) {
        let _ = self.with(|orchestrator| {
            orchestrator.restore(item.clone(), source);
            Ok(())
        });
    }

    /// Inspection result computed for `id` on the completion path. Used by core
    /// to persist it into SQLite so the playability gate can read it.
    pub fn inspection_state(&self, id: &str) -> Option<InspectionState> {
        self.with(|orchestrator| Ok(orchestrator.inspection_state(id)))
            .ok()
            .flatten()
    }

    /// Host-internal container paths for an acquired torrent.
    pub fn files(&self, hash: &str) -> Result<Vec<String>, AcquireError> {
        self.with(|orchestrator| orchestrator.files(hash))
    }

    /// Status never constructs a client: an untouched backend reports no rows.
    pub fn status(&self) -> Vec<DownloadItem> {
        self.orchestrator
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(|orchestrator| orchestrator.status()))
            .unwrap_or_default()
    }

    /// Re-read every tracked download's state from the engine. Returns the
    /// refreshed list. Errors are swallowed by the orchestrator's per-item
    /// handling: one unreachable torrent must not wipe the tracker's state.
    pub fn refresh(&self) -> Vec<DownloadItem> {
        self.orchestrator
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(|orchestrator| orchestrator.refresh()))
            .unwrap_or_default()
    }

    pub fn remove(&self, id: &str) -> Result<(), AcquireError> {
        let guard = self
            .orchestrator
            .lock()
            .map_err(|_| AcquireError::Backend)?;
        guard.as_ref().ok_or(AcquireError::NotFound)?.remove(id)
    }
}

impl Default for AcquisitionState {
    fn default() -> Self {
        Self::new()
    }
}

/// Map qBittorrent's own state strings to the closed renderer-facing state.
fn map_state(state: &str) -> DownloadState {
    // qB emits both "stalled up" in docs and "stalledUP" on the wire.
    match state.to_ascii_lowercase().replace(' ', "").as_str() {
        "completed" | "uploading" | "seeding" | "forcedup" | "stalledup" => DownloadState::Ready,
        "error" | "missingfiles" => DownloadState::Error,
        _ => DownloadState::Downloading,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;

    use crate::qbittorrent::{QbError, TorrentStatus};

    use super::*;

    #[derive(Default)]
    struct FakeApi {
        adds: AtomicUsize,
        removed_files: AtomicUsize,
    }

    impl TorrentApi for FakeApi {
        fn add(&self, source: &str, _save_path: &str) -> Result<String, QbError> {
            self.adds.fetch_add(1, Ordering::SeqCst);
            Ok(format!("hash-{}", source.len()))
        }

        fn status(&self, hash: &str) -> Result<TorrentStatus, QbError> {
            Ok(TorrentStatus {
                hash: hash.to_string(),
                state: "downloading".to_string(),
                progress: 0.25,
            })
        }

        fn files(&self, _hash: &str) -> Result<Vec<String>, QbError> {
            Ok(vec![])
        }

        fn remove(&self, _hash: &str, delete_files: bool) -> Result<(), QbError> {
            if delete_files {
                self.removed_files.fetch_add(1, Ordering::SeqCst);
            }
            Ok(())
        }
    }

    const MAGNET: &str = "magnet:?xt=urn:btih:ABC123&dn=episode";

    #[test]
    fn acquire_persists_an_item_and_duplicate_add_calls_add_once() {
        let api = FakeApi::default();
        let orchestrator = Orchestrator::new(api, InMemoryDownloadStore::default());

        let first = orchestrator.acquire(MAGNET).unwrap();
        let second = orchestrator.acquire(MAGNET).unwrap();

        assert_eq!(first.id, second.id);
        assert_eq!(first.torrent_hash, format!("hash-{}", MAGNET.len()));
        assert_eq!(first.state, DownloadState::Downloading);
        assert!((first.progress - 0.25).abs() < f64::EPSILON);
        assert_eq!(orchestrator.status().len(), 1);
        let stored = orchestrator
            .download_status(&first.id)
            .expect("the item is stored");
        assert_eq!(stored.id, first.id);
    }

    #[test]
    fn validation_rejects_every_unsafe_source_shape() {
        assert_eq!(validate_source(""), Err(AcquireError::EmptySource));
        assert_eq!(validate_source("   "), Err(AcquireError::EmptySource));
        assert_eq!(
            validate_source(&"m".repeat(MAX_SOURCE_LEN + 1)),
            Err(AcquireError::SourceTooLong)
        );
        assert_eq!(
            validate_source("not a magnet"),
            Err(AcquireError::InvalidSource)
        );
        assert_eq!(
            validate_source("magnet:?xt=urn:btih:ABC\n&tr=x"),
            Err(AcquireError::InvalidSource)
        );
        assert_eq!(
            validate_source("C:\\media\\episode.torrent"),
            Err(AcquireError::PathRejected)
        );
        assert_eq!(
            validate_source("/downloads/episode.torrent"),
            Err(AcquireError::PathRejected)
        );
        assert_eq!(
            validate_source("..\\episode.torrent"),
            Err(AcquireError::PathRejected)
        );
        assert_eq!(
            validate_source(".torrent"),
            Err(AcquireError::InvalidSource)
        );
        assert_eq!(
            validate_source("http://example.com/file.torrent"),
            Err(AcquireError::PathRejected)
        );
    }

    #[test]
    fn validation_accepts_magnets_and_bare_torrent_names() {
        assert_eq!(validate_source(MAGNET), Ok(MAGNET.to_string()));
        let long_magnet = format!("{}&tr={}", MAGNET, "x".repeat(2200));
        assert!(
            long_magnet.len() > 2048 && long_magnet.len() <= MAX_SOURCE_LEN,
            "the fixture must reproduce a real multi-tracker magnet"
        );
        assert_eq!(
            validate_source(&long_magnet),
            Ok(long_magnet.clone()),
            "a real-world magnet must not be rejected at the trust boundary"
        );
        assert_eq!(
            validate_source("  episode.torrent  "),
            Ok("episode.torrent".to_string())
        );
        assert_eq!(
            validate_source("S01E02.TORRENT"),
            Ok("S01E02.TORRENT".to_string())
        );
    }

    #[test]
    fn restored_persisted_rows_are_visible_without_a_backend_call() {
        let orchestrator = Orchestrator::new(FakeApi::default(), InMemoryDownloadStore::default());
        let item = DownloadItem {
            id: "restored".to_string(),
            torrent_hash: "restored".to_string(),
            media_id: Some("m1".to_string()),
            state: DownloadState::Ready,
            progress: 1.0,
        };
        orchestrator.restore(item.clone(), MAGNET);
        let restored = orchestrator.status();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].id, item.id);
        assert_eq!(restored[0].media_id, item.media_id);
        assert_eq!(restored[0].state, item.state);
    }

    #[test]
    fn acquire_rejects_invalid_source_without_calling_add() {
        let api = FakeApi::default();
        let orchestrator = Orchestrator::new(api, InMemoryDownloadStore::default());

        assert_eq!(
            orchestrator.acquire("/etc/passwd").unwrap_err(),
            AcquireError::InvalidSource
        );
        assert!(orchestrator.status().is_empty());
    }

    /// A torrent that finishes *after* it was added: the engine reports
    /// "downloading" at acquire time and "completed" on the next poll.
    struct CompletingApi {
        complete: Arc<AtomicBool>,
    }

    impl TorrentApi for CompletingApi {
        fn add(&self, _source: &str, _save_path: &str) -> Result<String, QbError> {
            Ok("hash-0".to_string())
        }
        fn status(&self, _hash: &str) -> Result<TorrentStatus, QbError> {
            let done = self.complete.load(Ordering::SeqCst);
            Ok(TorrentStatus {
                hash: "hash-0".to_string(),
                state: if done { "completed" } else { "downloading" }.to_string(),
                progress: if done { 1.0 } else { 0.25 },
            })
        }
        fn files(&self, _hash: &str) -> Result<Vec<String>, QbError> {
            Ok(vec![])
        }
        fn remove(&self, _hash: &str, _delete_files: bool) -> Result<(), QbError> {
            Ok(())
        }
    }

    /// A download that completes after acquire must be *seen*: without a
    /// refresh its state and progress are frozen at the acquire-time values
    /// forever, so a finished torrent stays "Downloading 25%" and is never
    /// inspected (never playable).
    #[test]
    fn refresh_follows_the_engine_state_without_writing_probe_state() {
        let complete = Arc::new(AtomicBool::new(false));
        let api = CompletingApi {
            complete: complete.clone(),
        };
        let orchestrator = Orchestrator::new(api, InMemoryDownloadStore::default());

        let item = orchestrator.acquire(MAGNET).unwrap();
        assert_eq!(item.state, DownloadState::Downloading);
        assert!(orchestrator.inspection_state(&item.id).is_none());

        // No completion yet: the refresh must keep the stored state.
        let refreshed = orchestrator.refresh();
        assert_eq!(refreshed[0].state, DownloadState::Downloading);
        assert!((refreshed[0].progress - 0.25).abs() < f64::EPSILON);
        assert!(orchestrator.inspection_state(&item.id).is_none());

        // The engine now reports the torrent complete. Refresh only polls
        // `status()`, so the fake must be toggled rather than adding again.
        complete.store(true, Ordering::SeqCst);
        let refreshed = orchestrator.refresh();
        assert_eq!(refreshed[0].state, DownloadState::Ready);
        assert!((refreshed[0].progress - 1.0).abs() < f64::EPSILON);
        // Ready does not write probe state. The Windows host owns that gate.
        assert!(orchestrator.inspection_state(&item.id).is_none());
    }

    #[test]
    fn map_state_accepts_qb_camel_case_completion_states() {
        for state in ["completed", "uploading", "seeding", "forcedUP", "stalledUP"] {
            assert_eq!(map_state(state), DownloadState::Ready, "{state}");
        }
        for state in ["stalledDL", "downloading", "metaDL"] {
            assert_eq!(map_state(state), DownloadState::Downloading, "{state}");
        }
    }

    #[test]
    fn production_orchestrator_never_auto_inspects_a_completed_download() {
        let complete = Arc::new(AtomicBool::new(true));
        let orchestrator = Orchestrator::new(
            CompletingApi {
                complete: complete.clone(),
            },
            InMemoryDownloadStore::default(),
        );
        let item = orchestrator.acquire(MAGNET).unwrap();
        assert_eq!(item.state, DownloadState::Ready);
        assert!(
            orchestrator.inspection_state(&item.id).is_none(),
            "the Windows host is the only writer of probe state"
        );
    }

    /// A source confirmed *for* an item must carry that item's id, so the
    /// persisted download row ties the file to the item (R-01). The store's
    /// source→item map stays one-to-one: the same source confirmed for a
    /// second item re-binds, it does not fork the tracked download.
    #[test]
    fn acquire_for_binds_the_media_id_and_survives_a_second_bind() {
        let api = FakeApi::default();
        let orchestrator = Orchestrator::new(api, InMemoryDownloadStore::default());

        let bound = orchestrator.acquire_for(MAGNET, Some("w1")).unwrap();
        assert_eq!(bound.media_id.as_deref(), Some("w1"));

        let rebound = orchestrator.acquire_for(MAGNET, Some("w2")).unwrap();
        assert_eq!(rebound.id, bound.id, "the same source is one download");
        assert_eq!(rebound.media_id.as_deref(), Some("w2"));
    }

    #[test]
    fn remove_keeps_files_and_forgets_only_the_named_task() {
        let api = FakeApi::default();
        let orchestrator = Orchestrator::new(api, InMemoryDownloadStore::default());
        let first = orchestrator.acquire(MAGNET).unwrap();
        let other = orchestrator
            .acquire("magnet:?xt=urn:btih:DIFFERENT")
            .unwrap();

        orchestrator.remove(&first.id).unwrap();

        assert_eq!(orchestrator.status().len(), 1);
        assert!(orchestrator.download_status(&first.id).is_none());
        let remaining = orchestrator
            .download_status(&other.id)
            .expect("the other item is kept");
        assert_eq!(remaining.id, other.id);
        assert_eq!(
            orchestrator.remove(&first.id).unwrap_err(),
            AcquireError::NotFound
        );
    }

    // --- Completion-path inspection trigger (Task 1 finding 1) --------------

    struct ReadyApi {
        files: Vec<String>,
    }

    impl TorrentApi for ReadyApi {
        fn add(&self, source: &str, _save_path: &str) -> Result<String, QbError> {
            Ok(format!("hash-{source}"))
        }
        fn status(&self, hash: &str) -> Result<TorrentStatus, QbError> {
            Ok(TorrentStatus {
                hash: hash.to_string(),
                state: "completed".to_string(),
                progress: 1.0,
            })
        }
        fn files(&self, _hash: &str) -> Result<Vec<String>, QbError> {
            Ok(self.files.clone())
        }
        fn remove(&self, _hash: &str, _delete_files: bool) -> Result<(), QbError> {
            Ok(())
        }
    }

    struct FakeProbe(InspectionState);

    impl ProbeRunner for FakeProbe {
        fn run(
            &self,
            _container_path: &str,
        ) -> Result<crate::inspection::ProbeOutput, crate::inspection::ProbeError> {
            use crate::inspection::{ProbeError, ProbeOutput};
            match self.0 {
                InspectionState::Clean => Ok(ProbeOutput {
                    success: true,
                    stdout: r#"{"format":{"format_name":"matroska"}}"#.to_string(),
                }),
                InspectionState::Suspicious => Ok(ProbeOutput {
                    success: false,
                    stdout: String::new(),
                }),
                _ => Err(ProbeError::Unavailable),
            }
        }
    }

    #[test]
    fn completed_download_is_inspected_automatically_on_acquire() {
        let api = ReadyApi {
            files: vec!["show/e01.mkv".to_string()],
        };
        let orchestrator = Orchestrator::new(api, InMemoryDownloadStore::default())
            .with_probe_runner(FakeProbe(InspectionState::Clean));
        let item = orchestrator.acquire(MAGNET).expect("acquire");

        assert_eq!(item.state, DownloadState::Ready);
        assert_eq!(
            orchestrator.inspection_state(&item.id),
            Some(InspectionState::Clean)
        );
        let results = orchestrator.inspection_results(&item.id);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "/downloads/show/e01.mkv");
        assert_eq!(results[0].1, InspectionState::Clean);
    }

    #[test]
    fn refused_and_unavailable_probes_fail_closed_on_completion() {
        for (probe_state, expected) in [
            (InspectionState::Suspicious, InspectionState::Suspicious),
            (InspectionState::Unavailable, InspectionState::Unavailable),
        ] {
            let api = ReadyApi {
                files: vec!["e01.mkv".to_string()],
            };
            let orchestrator = Orchestrator::new(api, InMemoryDownloadStore::default())
                .with_probe_runner(FakeProbe(probe_state));
            let item = orchestrator.acquire(MAGNET).expect("acquire");
            assert_eq!(orchestrator.inspection_state(&item.id), Some(expected));
            assert_ne!(
                orchestrator.inspection_state(&item.id),
                Some(InspectionState::Clean)
            );
        }
    }

    #[test]
    fn incomplete_download_is_not_inspected_yet() {
        let orchestrator = Orchestrator::new(FakeApi::default(), InMemoryDownloadStore::default())
            .with_probe_runner(FakeProbe(InspectionState::Clean));
        let item = orchestrator.acquire(MAGNET).expect("acquire");
        assert_eq!(item.state, DownloadState::Downloading);
        assert_eq!(orchestrator.inspection_state(&item.id), None);
    }

    #[test]
    fn completed_torrent_with_no_files_fails_closed() {
        let orchestrator =
            Orchestrator::new(ReadyApi { files: vec![] }, InMemoryDownloadStore::default())
                .with_probe_runner(FakeProbe(InspectionState::Clean));
        let item = orchestrator.acquire(MAGNET).expect("acquire");
        assert_eq!(
            orchestrator.inspection_state(&item.id),
            Some(InspectionState::Unavailable),
            "no files to inspect is never Clean"
        );
    }
}
