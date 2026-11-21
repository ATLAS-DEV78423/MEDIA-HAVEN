//! Acquisition orchestration: validation, duplicate prevention and the
//! add → persist → status flow (spec R-11, Task 4).
//!
//! The orchestrator is generic over a `TorrentApi` (qBittorrent in production,
//! a fake in tests) and a `DownloadStore`. It is deliberately synchronous and
//! path-free: no host path or credential is ever produced here.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use app_core::domain::{DownloadItem, DownloadState};

use crate::executables::has_executable_file;
use crate::inspection::{
    combine_inspection, inspect_completed, DockerProbeRunner, InspectionState, ProbeRunner,
};
use crate::qbittorrent::{QbClient, TorrentApi};

/// Container destination for user-supplied acquisitions (storage option B).
/// The host directory behind it is resolved by the A9 mount map, never here.
pub const SAVE_PATH: &str = "/downloads";

/// Closed cap on a source string. Magnets with several trackers fit comfortably;
/// anything larger is rejected rather than buffered.
const MAX_SOURCE_LEN: usize = 2048;
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
    #[error(
        "torrent contains an executable file or has no readable file list; nothing was downloaded"
    )]
    Rejected,
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
    /// Per-file probe results for completed downloads (host-internal only).
    inspection: Mutex<HashMap<String, Vec<(String, InspectionState)>>>,
}

impl<A: TorrentApi, S: DownloadStore> Orchestrator<A, S> {
    pub fn new(api: A, store: S) -> Self {
        Self {
            api,
            store,
            probe: Box::new(DockerProbeRunner),
            inspection: Mutex::new(HashMap::new()),
        }
    }

    /// Inject a probe for tests. Production uses `DockerProbeRunner` by default.
    pub fn with_probe_runner(mut self, runner: impl ProbeRunner + Send + Sync + 'static) -> Self {
        self.probe = Box::new(runner);
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
        if item.state != DownloadState::Ready {
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
        let source = validate_source(raw)?;
        if let Some(existing) = self.store.download_exists(&source) {
            return Ok(existing);
        }
        let hash = self
            .api
            .add(&source, SAVE_PATH)
            .map_err(|_| AcquireError::Backend)?;

        // Metadata for a freshly added magnet may lag; poll briefly, then fail
        // closed. The task is paused, so no file bytes are written meanwhile.
        let mut declared: Vec<String> = Vec::new();
        for _ in 0..5 {
            match self.api.files(&hash) {
                Ok(files) if !files.is_empty() => {
                    declared = files;
                    break;
                }
                _ => std::thread::sleep(std::time::Duration::from_millis(200)),
            }
        }
        if declared.is_empty() || has_executable_file(&declared) {
            // Task-only removal: no file bytes exist, so delete_files=false.
            let _ = self.api.remove(&hash, false);
            return Err(AcquireError::Rejected);
        }
        self.api.resume(&hash).map_err(|_| {
            let _ = self.api.remove(&hash, false);
            AcquireError::Backend
        })?;

        let status = self.api.status(&hash).map_err(|_| AcquireError::Backend)?;
        let item = DownloadItem {
            id: hash.clone(),
            torrent_hash: hash,
            media_id: None,
            state: map_state(&status.state),
            progress: status.progress,
        };
        self.store.download_insert(&item, &source);
        self.inspect_if_ready(&item);
        Ok(item)
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
    match state.to_ascii_lowercase().as_str() {
        "completed" | "uploading" | "seeding" | "forced up" | "stalled up" => DownloadState::Ready,
        "error" | "missingfiles" => DownloadState::Error,
        _ => DownloadState::Downloading,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use crate::qbittorrent::{QbError, TorrentStatus};

    use super::*;

    /// Fake with shareable call counters so a test can prove a rejected task
    /// was never resumed and was removed task-only before it is consumed.
    struct FakeApi {
        adds: Arc<AtomicUsize>,
        removed_files: Arc<AtomicUsize>,
        resumes: Arc<AtomicUsize>,
        task_removes: Arc<AtomicUsize>,
        files: Vec<String>,
        files_error: bool,
    }

    impl Default for FakeApi {
        fn default() -> Self {
            Self {
                adds: Arc::new(AtomicUsize::new(0)),
                removed_files: Arc::new(AtomicUsize::new(0)),
                resumes: Arc::new(AtomicUsize::new(0)),
                task_removes: Arc::new(AtomicUsize::new(0)),
                // A benign, non-empty list is the default so existing tests
                // that only want a successful acquire still pass the guard.
                files: vec!["show/e01.mkv".to_string()],
                files_error: false,
            }
        }
    }

    impl TorrentApi for FakeApi {
        fn add(&self, source: &str, _save_path: &str) -> Result<String, QbError> {
            self.adds.fetch_add(1, Ordering::SeqCst);
            Ok(format!("hash-{}", source.len()))
        }

        fn resume(&self, _hash: &str) -> Result<(), QbError> {
            self.resumes.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn status(&self, hash: &str) -> Result<TorrentStatus, QbError> {
            Ok(TorrentStatus {
                hash: hash.to_string(),
                state: "downloading".to_string(),
                progress: 0.25,
            })
        }

        fn files(&self, _hash: &str) -> Result<Vec<String>, QbError> {
            if self.files_error {
                Err(QbError::Api(500))
            } else {
                Ok(self.files.clone())
            }
        }

        fn remove(&self, _hash: &str, delete_files: bool) -> Result<(), QbError> {
            if delete_files {
                self.removed_files.fetch_add(1, Ordering::SeqCst);
            } else {
                self.task_removes.fetch_add(1, Ordering::SeqCst);
            }
            Ok(())
        }
    }

    const MAGNET: &str = "magnet:?xt=urn:btih:ABC123&dn=episode";

    #[test]
    fn executable_torrent_is_removed_without_resuming_or_storing() {
        let api = FakeApi {
            files: vec!["show/S01E01.mkv".into(), "installer.exe".into()],
            ..FakeApi::default()
        };
        let resumes = Arc::clone(&api.resumes);
        let task_removes = Arc::clone(&api.task_removes);
        let orchestrator = Orchestrator::new(api, InMemoryDownloadStore::default());

        let err = orchestrator.acquire(MAGNET).unwrap_err();

        assert!(matches!(err, AcquireError::Rejected));
        assert_eq!(
            resumes.load(Ordering::SeqCst),
            0,
            "never resume a rejected task"
        );
        assert_eq!(
            task_removes.load(Ordering::SeqCst),
            1,
            "remove the task with delete_files=false"
        );
        assert!(
            orchestrator.status().is_empty(),
            "nothing may be stored on rejection"
        );
    }

    #[test]
    fn clean_torrent_is_resumed_and_stored() {
        let api = FakeApi {
            files: vec!["show/S01E01.mkv".into()],
            ..FakeApi::default()
        };
        let resumes = Arc::clone(&api.resumes);
        let orchestrator = Orchestrator::new(api, InMemoryDownloadStore::default());

        let item = orchestrator.acquire(MAGNET).unwrap();

        assert!(!item.id.is_empty());
        assert_eq!(
            resumes.load(Ordering::SeqCst),
            1,
            "resume an all-media task once"
        );
        assert_eq!(orchestrator.status().len(), 1);
    }

    #[test]
    fn empty_or_unreadable_file_list_is_rejected() {
        for (files, files_error) in [(vec![], false), (vec!["show/e01.mkv".into()], true)] {
            let api = FakeApi {
                files,
                files_error,
                ..FakeApi::default()
            };
            let resumes = Arc::clone(&api.resumes);
            let task_removes = Arc::clone(&api.task_removes);
            let orchestrator = Orchestrator::new(api, InMemoryDownloadStore::default());

            assert!(matches!(
                orchestrator.acquire(MAGNET),
                Err(AcquireError::Rejected)
            ));
            assert_eq!(resumes.load(Ordering::SeqCst), 0);
            assert_eq!(task_removes.load(Ordering::SeqCst), 1);
        }
    }

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
    fn acquire_rejects_invalid_source_without_calling_add() {
        let api = FakeApi::default();
        let orchestrator = Orchestrator::new(api, InMemoryDownloadStore::default());

        assert_eq!(
            orchestrator.acquire("/etc/passwd").unwrap_err(),
            AcquireError::InvalidSource
        );
        assert!(orchestrator.status().is_empty());
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

    #[derive(Default)]
    struct ReadyApi {
        files: Vec<String>,
        /// Simulate files vanishing between the acquire guard and completion
        /// inspection, so the post-download fail-closed gate stays testable.
        empty_on_completion: bool,
        files_calls: Arc<AtomicUsize>,
    }

    impl TorrentApi for ReadyApi {
        fn add(&self, source: &str, _save_path: &str) -> Result<String, QbError> {
            Ok(format!("hash-{source}"))
        }
        fn resume(&self, _hash: &str) -> Result<(), QbError> {
            Ok(())
        }
        fn status(&self, hash: &str) -> Result<TorrentStatus, QbError> {
            Ok(TorrentStatus {
                hash: hash.to_string(),
                state: "completed".to_string(),
                progress: 1.0,
            })
        }
        fn files(&self, _hash: &str) -> Result<Vec<String>, QbError> {
            if self.empty_on_completion && self.files_calls.fetch_add(1, Ordering::SeqCst) > 0 {
                Ok(vec![])
            } else {
                Ok(self.files.clone())
            }
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
            ..ReadyApi::default()
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
                ..ReadyApi::default()
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
        // The pre-download guard needs a benign declared file to admit the
        // task; the post-download gate must still fail closed if that file is
        // gone by the time completion is inspected.
        let api = ReadyApi {
            files: vec!["e01.mkv".to_string()],
            empty_on_completion: true,
            ..ReadyApi::default()
        };
        let orchestrator = Orchestrator::new(api, InMemoryDownloadStore::default())
            .with_probe_runner(FakeProbe(InspectionState::Clean));
        let item = orchestrator.acquire(MAGNET).expect("acquire");
        assert_eq!(
            orchestrator.inspection_state(&item.id),
            Some(InspectionState::Unavailable),
            "no files to inspect is never Clean"
        );
    }
}
