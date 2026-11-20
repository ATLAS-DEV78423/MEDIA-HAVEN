use crate::domain::*;
use crate::ports::LibraryRepository;

pub struct LibraryService<R: LibraryRepository> {
    repo: R,
}

impl<R: LibraryRepository> LibraryService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }

    pub async fn home(&self) -> Result<HomeScreen, AppError> {
        self.repo.home().await
    }
    pub async fn movies(&self, query: &BrowseQuery) -> Result<Vec<MediaSummary>, AppError> {
        self.repo.movies(query).await
    }
    pub async fn shows(&self, query: &BrowseQuery) -> Result<Vec<SeriesDetail>, AppError> {
        self.repo.shows(query).await
    }
    pub async fn detail(&self, id: &str) -> Result<MediaDetail, AppError> {
        self.repo.detail(id).await
    }
    pub async fn episodes(
        &self,
        series_id: &str,
        season_number: i64,
    ) -> Result<Vec<EpisodeSummary>, AppError> {
        self.repo.episodes(series_id, season_number).await
    }
    pub async fn search(&self, query: &SearchQuery) -> Result<SearchResults, AppError> {
        self.repo.search(query).await
    }
    pub async fn roots(&self) -> Result<LibraryScreen, AppError> {
        self.repo.roots().await
    }
    pub async fn root_add(&self, display_name: &str, path: &str) -> Result<LibraryRoot, AppError> {
        self.repo.root_add(display_name, path).await
    }
    pub async fn root_remove(&self, id: &str) -> Result<(), AppError> {
        self.repo.root_remove(id).await
    }
    pub async fn recover_interrupted_scans(&self) -> Result<usize, AppError> {
        self.repo.recover_interrupted_scans().await
    }
    pub async fn root_update(
        &self,
        id: &str,
        item_count: i64,
        status: LibraryRootStatus,
        last_scan_at: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<(), AppError> {
        self.repo
            .root_update(id, item_count, status, last_scan_at, error_message)
            .await
    }
    pub async fn media_replace_for_root(
        &self,
        root_id: &str,
        items: &[MediaUpsert],
    ) -> Result<(), AppError> {
        self.repo.media_replace_for_root(root_id, items).await
    }
    pub async fn watchlist(&self) -> Result<WatchlistScreen, AppError> {
        self.repo.watchlist().await
    }
    pub async fn queue(&self) -> Result<QueueScreen, AppError> {
        self.repo.queue().await
    }
    pub async fn downloads(&self) -> Result<DownloadsScreen, AppError> {
        self.repo.downloads().await
    }
    pub async fn settings_get(&self) -> Result<AppSettings, AppError> {
        self.repo.settings_get().await
    }
    pub async fn settings_set(&self, patch: &SettingsPatch) -> Result<AppSettings, AppError> {
        self.repo.settings_set(patch).await
    }
    pub async fn playback_start(
        &self,
        request: &PlaybackStartRequest,
    ) -> Result<PlaybackSession, AppError> {
        self.repo.playback_start(request).await
    }
    pub async fn playback_stop(&self) -> Result<(), AppError> {
        self.repo.playback_stop().await
    }
    pub async fn playback_checkpoint(
        &self,
        request: &PlaybackCheckpointRequest,
    ) -> Result<(), AppError> {
        self.repo.playback_checkpoint(request).await
    }
    /// Host-internal passthrough (ADR-0015). The container path returned here
    /// is resolved in the host only; it is never placed in a renderer DTO.
    pub async fn media_source_path(&self, id: &str) -> Result<Option<String>, AppError> {
        self.repo.media_source_path(id).await
    }
    pub async fn watchlist_add(&self, media_id: &str) -> Result<(), AppError> {
        self.repo.watchlist_add(media_id).await
    }
    pub async fn watchlist_remove(&self, media_id: &str) -> Result<(), AppError> {
        self.repo.watchlist_remove(media_id).await
    }
    pub async fn watchlist_reorder(&self, ordered_ids: &[String]) -> Result<(), AppError> {
        self.repo.watchlist_reorder(ordered_ids).await
    }
    pub async fn queue_add(&self, media_id: &str) -> Result<(), AppError> {
        self.repo.queue_add(media_id).await
    }
    pub async fn queue_remove(&self, queue_id: &str) -> Result<(), AppError> {
        self.repo.queue_remove(queue_id).await
    }
    pub async fn queue_reorder(&self, ordered_ids: &[String]) -> Result<(), AppError> {
        self.repo.queue_reorder(ordered_ids).await
    }
    pub async fn download_add(
        &self,
        item: &DownloadItem,
        file_path: &str,
        source: &str,
        save_path: &str,
    ) -> Result<(), AppError> {
        self.repo
            .download_add(item, file_path, source, save_path)
            .await
    }
    pub async fn download_status(&self, id: &str) -> Result<Option<DownloadItem>, AppError> {
        self.repo.download_status(id).await
    }
    pub async fn download_remove(&self, id: &str) -> Result<(), AppError> {
        self.repo.download_remove(id).await
    }
    pub async fn prefetch_upsert(&self, entry: &PrefetchEntry) -> Result<(), AppError> {
        self.repo.prefetch_upsert(entry).await
    }
    pub async fn cleanup_eligible(&self) -> Result<Vec<CleanupEntry>, AppError> {
        self.repo.cleanup_eligible().await
    }
}
