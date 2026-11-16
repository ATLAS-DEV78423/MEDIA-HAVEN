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
    pub async fn episodes(&self, series_id: &str, season_number: i64) -> Result<Vec<EpisodeSummary>, AppError> {
        self.repo.episodes(series_id, season_number).await
    }
    pub async fn search(&self, query: &SearchQuery) -> Result<SearchResults, AppError> {
        self.repo.search(query).await
    }
    pub async fn roots(&self) -> Result<LibraryScreen, AppError> {
        self.repo.roots().await
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
    pub async fn playback_start(&self, request: &PlaybackStartRequest) -> Result<PlaybackSession, AppError> {
        self.repo.playback_start(request).await
    }
    pub async fn playback_stop(&self) -> Result<(), AppError> {
        self.repo.playback_stop().await
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
}