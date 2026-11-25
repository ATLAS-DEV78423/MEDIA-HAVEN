use crate::domain::*;
use async_trait::async_trait;

#[async_trait]
pub trait LibraryRepository: Send + Sync {
    async fn home(&self) -> Result<HomeScreen, AppError>;
    async fn movies(&self, query: &BrowseQuery) -> Result<Vec<MediaSummary>, AppError>;
    async fn shows(&self, query: &BrowseQuery) -> Result<Vec<SeriesDetail>, AppError>;
    async fn detail(&self, id: &str) -> Result<MediaDetail, AppError>;
    async fn episodes(
        &self,
        series_id: &str,
        season_number: i64,
    ) -> Result<Vec<EpisodeSummary>, AppError>;
    async fn search(&self, query: &SearchQuery) -> Result<SearchResults, AppError>;
    async fn roots(&self) -> Result<LibraryScreen, AppError>;
    async fn root_add(&self, display_name: &str, path: &str) -> Result<LibraryRoot, AppError>;
    async fn root_remove(&self, id: &str) -> Result<(), AppError>;
    async fn root_update(
        &self,
        id: &str,
        item_count: i64,
        status: LibraryRootStatus,
        last_scan_at: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<(), AppError>;
    /// Mark roots left in `scanning` by a killed process as `error`.
    /// Returns the number of roots recovered. Never deletes rows or media.
    async fn recover_interrupted_scans(&self) -> Result<usize, AppError>;
    async fn media_replace_for_root(
        &self,
        root_id: &str,
        items: &[MediaUpsert],
    ) -> Result<(), AppError>;
    async fn watchlist(&self) -> Result<WatchlistScreen, AppError>;
    async fn queue(&self) -> Result<QueueScreen, AppError>;
    async fn downloads(&self) -> Result<DownloadsScreen, AppError>;
    async fn settings_get(&self) -> Result<AppSettings, AppError>;
    async fn settings_set(&self, patch: &SettingsPatch) -> Result<AppSettings, AppError>;
    async fn playback_start(
        &self,
        request: &PlaybackStartRequest,
    ) -> Result<PlaybackSession, AppError>;
    async fn playback_stop(&self) -> Result<(), AppError>;
    async fn playback_checkpoint(
        &self,
        request: &PlaybackCheckpointRequest,
    ) -> Result<(), AppError>;
    /// Host-internal container-path identity for a media id (ADR-0015).
    /// Never a renderer DTO: the path stays inside the host and is handed only
    /// to the playback engine after host-side resolution.
    async fn media_source_path(&self, id: &str) -> Result<Option<String>, AppError>;
    async fn watchlist_add(&self, media_id: &str) -> Result<(), AppError>;
    async fn watchlist_remove(&self, media_id: &str) -> Result<(), AppError>;
    async fn watchlist_reorder(&self, ordered_ids: &[String]) -> Result<(), AppError>;
    async fn queue_add(&self, media_id: &str) -> Result<(), AppError>;
    async fn queue_remove(&self, queue_id: &str) -> Result<(), AppError>;
    async fn queue_reorder(&self, ordered_ids: &[String]) -> Result<(), AppError>;

    // Acquisition records. Paths are host-internal arguments, never DTO fields.
    async fn download_add(
        &self,
        item: &DownloadItem,
        file_path: &str,
        source: &str,
        save_path: &str,
    ) -> Result<(), AppError>;
    async fn download_status(&self, id: &str) -> Result<Option<DownloadItem>, AppError>;
    /// All persisted acquisition rows with their original operator source.
    /// Used to rehydrate the in-memory tracker after a core restart.
    async fn acquisition_rows(&self) -> Result<Vec<(DownloadItem, String)>, AppError>;
    /// Persist engine-reported progress/state. Playback lifecycle states are
    /// owned by the cleanup worker, never by this transport-level refresh.
    async fn download_update(&self, item: &DownloadItem) -> Result<(), AppError>;
    /// Replace placeholder/auto-created file paths once the engine reports the
    /// real names. Manual episode mappings are never removed.
    async fn download_set_paths(&self, id: &str, paths: &[String]) -> Result<(), AppError>;
    async fn download_remove(&self, id: &str) -> Result<(), AppError>;
    async fn prefetch_upsert(&self, entry: &PrefetchEntry) -> Result<(), AppError>;
    async fn cleanup_eligible(&self) -> Result<Vec<CleanupEntry>, AppError>;

    // R-01 per-item sources (ADR-0021). Operator-supplied only; no path in a
    // renderer DTO and no source is discovered or invented.
    async fn source_for_media(&self, media_id: &str) -> Result<Option<String>, AppError>;
    async fn set_item_source(&self, media_id: &str, source: &str) -> Result<(), AppError>;

    // Catalog "wanted" item (ADR-0024). Stable id supplied by the caller; no
    // media_files row is created and the row starts unavailable.
    async fn wanted_add(
        &self,
        title: &str,
        year: Option<i64>,
        kind: &str,
        id: &str,
    ) -> Result<(), AppError>;
}
