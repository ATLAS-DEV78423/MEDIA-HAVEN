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
    async fn watchlist_add(&self, media_id: &str) -> Result<(), AppError>;
    async fn watchlist_remove(&self, media_id: &str) -> Result<(), AppError>;
    async fn watchlist_reorder(&self, ordered_ids: &[String]) -> Result<(), AppError>;
    async fn queue_add(&self, media_id: &str) -> Result<(), AppError>;
    async fn queue_remove(&self, queue_id: &str) -> Result<(), AppError>;
    async fn queue_reorder(&self, ordered_ids: &[String]) -> Result<(), AppError>;
}
