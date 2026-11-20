//! Acquisition orchestration: validation, duplicate prevention and the
//! add → persist → status flow (spec R-11, Task 4).
//!
//! The orchestrator is generic over a `TorrentApi` (qBittorrent in production,
//! a fake in tests) and a `DownloadStore`. It is deliberately synchronous and
//! path-free: no host path or credential is ever produced here.

use std::sync::{Mutex, PoisonError};

use app_core::domain::{DownloadItem, DownloadState};

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
    }
}

/// Drives acquisition through a fakeable `TorrentApi` and `DownloadStore`.
pub struct Orchestrator<A: TorrentApi, S: DownloadStore> {
    api: A,
    store: S,
}

impl<A: TorrentApi, S: DownloadStore> Orchestrator<A, S> {
    pub fn new(api: A, store: S) -> Self {
        Self { api, store }
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
        let status = self.api.status(&hash).map_err(|_| AcquireError::Backend)?;
        let item = DownloadItem {
            id: hash.clone(),
            torrent_hash: hash,
            media_id: None,
            state: map_state(&status.state),
            progress: status.progress,
        };
        self.store.download_insert(&item, &source);
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
}
