use serde::{Deserialize, Serialize};

pub type MediaId = String;
pub type CollectionId = String;
pub type IsoTimestamp = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    Movie,
    Series,
    Episode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtworkVariant {
    Poster,
    Backdrop,
    Still,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkRef {
    pub variant: ArtworkVariant,
    pub url: Option<String>,
    pub alt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum MediaAvailability {
    Available,
    #[serde(rename = "missing-file")]
    #[serde(rename_all = "camelCase")]
    MissingFile {
        last_seen_at: Option<IsoTimestamp>,
    },
    Offline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackProgress {
    pub position_seconds: f64,
    pub duration_seconds: f64,
    pub updated_at: IsoTimestamp,
    pub completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaSummary {
    pub id: MediaId,
    pub kind: MediaKind,
    pub title: String,
    pub year: Option<i64>,
    pub synopsis: Option<String>,
    pub runtime_minutes: Option<i64>,
    pub genres: Vec<String>,
    pub artwork: Vec<ArtworkRef>,
    pub availability: MediaAvailability,
    pub progress: Option<PlaybackProgress>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MovieDetail {
    pub id: MediaId,
    pub kind: MediaKind,
    pub title: String,
    pub year: Option<i64>,
    pub synopsis: Option<String>,
    pub runtime_minutes: Option<i64>,
    pub genres: Vec<String>,
    pub artwork: Vec<ArtworkRef>,
    pub availability: MediaAvailability,
    pub progress: Option<PlaybackProgress>,
    pub director: Option<String>,
    pub cast: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeasonSummary {
    pub number: i64,
    pub title: String,
    pub episode_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesDetail {
    pub id: CollectionId,
    pub kind: MediaKind,
    pub title: String,
    pub year: Option<i64>,
    pub synopsis: Option<String>,
    pub genres: Vec<String>,
    pub artwork: Vec<ArtworkRef>,
    pub seasons: Vec<SeasonSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MediaDetail {
    Movie(MovieDetail),
    Series(SeriesDetail),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeSummary {
    pub id: MediaId,
    pub series_id: CollectionId,
    pub series_title: String,
    pub season_number: i64,
    pub episode_number: i64,
    pub title: String,
    pub runtime_minutes: Option<i64>,
    pub artwork: Vec<ArtworkRef>,
    pub availability: MediaAvailability,
    pub progress: Option<PlaybackProgress>,
    pub next_up: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShelfId {
    ContinueWatching,
    UpNext,
    RecentlyAddedMovies,
    InProgress,
    RecentlyAddedShows,
    Recommended,
    RecentlyPlayed,
    Watchlist,
    Genres,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaShelf {
    pub id: ShelfId,
    pub title: String,
    pub items: Vec<MediaSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeroActionKind {
    Play,
    Resume,
    Details,
    Queue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeroAction {
    pub kind: HeroActionKind,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeroFeature {
    pub media: MediaSummary,
    pub headline: String,
    pub actions: Vec<HeroAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeScreen {
    pub hero: Option<HeroFeature>,
    pub shelves: Vec<MediaShelf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LibraryRootStatus {
    Ok,
    Scanning,
    Offline,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryRoot {
    pub id: String,
    pub display_name: String,
    pub item_count: i64,
    pub status: LibraryRootStatus,
    pub last_scan_at: Option<IsoTimestamp>,
    pub error_message: Option<String>,
    pub path: Option<String>,
}

/// Metadata read from a local source (currently an `.nfo` sidecar) before any
/// provider is involved. Every field is optional so a sparse source never
/// blanks a value the library already holds.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataRecord {
    pub title: Option<String>,
    pub year: Option<i64>,
    pub synopsis: Option<String>,
    pub runtime_minutes: Option<i64>,
    pub genres: Vec<String>,
}

/// A media item discovered by the scanner, ready to be persisted.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaUpsert {
    pub id: MediaId,
    pub kind: MediaKind,
    pub title: String,
    pub root_id: Option<String>,
    pub path: Option<String>,
    pub size_bytes: Option<i64>,
    pub hash: Option<String>,
    /// Offline metadata found beside the file. `None` leaves stored values
    /// untouched on rescan.
    #[serde(default)]
    pub metadata: Option<MetadataRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryCounts {
    pub movies: i64,
    pub shows: i64,
    pub episodes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryScreen {
    pub roots: Vec<LibraryRoot>,
    pub counts: LibraryCounts,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchQuery {
    pub text: String,
    pub kinds: Vec<MediaKind>,
    pub genres: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResults {
    pub query: SearchQuery,
    pub items: Vec<MediaSummary>,
    pub total: i64,
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueEntryState {
    Pending,
    Active,
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueEntry {
    pub id: String,
    pub media: MediaSummary,
    pub position: i64,
    pub state: QueueEntryState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueScreen {
    pub entries: Vec<QueueEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchlistEntry {
    pub id: String,
    pub media: MediaSummary,
    pub position: i64,
    pub added_at: IsoTimestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchlistScreen {
    pub entries: Vec<WatchlistEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadState {
    Queued,
    Downloading,
    Paused,
    Verifying,
    Available,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTask {
    pub id: String,
    pub title: String,
    pub state: DownloadState,
    pub progress: f64,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadsScreen {
    pub enabled: bool,
    pub tasks: Vec<DownloadTask>,
    pub storage_used_bytes: i64,
    pub storage_limit_bytes: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackOption {
    pub id: String,
    pub label: String,
    pub language: Option<String>,
    pub selected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackSession {
    pub session_id: String,
    pub media: MediaSummary,
    pub start_position_seconds: f64,
    pub duration_seconds: f64,
    pub subtitle_tracks: Vec<TrackOption>,
    pub audio_tracks: Vec<TrackOption>,
    pub can_resume: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum RuntimeStatus {
    Unknown,
    Absent,
    Stopped,
    Starting,
    Ready,
    #[serde(rename = "unhealthy")]
    Unhealthy {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackSettings {
    pub preferred_language: String,
    pub subtitle_size: String,
    pub autoplay_next: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageSettings {
    pub root_display_name: String,
    pub limit_bytes: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionalServicesSettings {
    pub downloads_enabled: bool,
    pub metadata_provider: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsSettings {
    pub runtime_status: RuntimeStatus,
    pub backend_status: RuntimeStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub avatar_color: String,
    pub created_at: IsoTimestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub library_roots: Vec<LibraryRoot>,
    pub playback: PlaybackSettings,
    pub storage: StorageSettings,
    pub optional_services: OptionalServicesSettings,
    pub diagnostics: DiagnosticsSettings,
    #[serde(default)]
    pub profiles: Vec<Profile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlaybackSettingsPatch {
    pub preferred_language: Option<String>,
    pub subtitle_size: Option<String>,
    pub autoplay_next: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StorageSettingsPatch {
    pub root_display_name: Option<String>,
    pub limit_bytes: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsPatch {
    pub playback: Option<PlaybackSettingsPatch>,
    pub storage: Option<StorageSettingsPatch>,
    #[serde(default)]
    pub profiles: Option<Vec<Profile>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowseSort {
    Recent,
    Title,
    Year,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BrowseQuery {
    pub sort: BrowseSort,
    pub genre: Option<String>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlaybackStartRequest {
    pub media_id: MediaId,
    pub start_position_seconds: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlaybackCheckpointRequest {
    pub media_id: MediaId,
    pub position_seconds: f64,
    pub duration_seconds: Option<f64>,
}
