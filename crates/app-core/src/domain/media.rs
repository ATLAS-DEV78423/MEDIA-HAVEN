use serde::{Deserialize, Serialize};

use super::errors::{AppError, AppErrorCode};

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

// Display-only discovery DTOs. No path, token, host or upstream URL is carried:
// posters are the same relative hashed `/artwork/<64-hex>` refs as the library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CatalogKindDto {
    Movie,
    Tv,
    Anime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntryDto {
    pub id: String,
    pub kind: CatalogKindDto,
    pub title: String,
    pub year: Option<i64>,
    pub synopsis: Option<String>,
    pub rating: Option<f64>,
    pub artwork: Vec<ArtworkRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogScreen {
    pub available: bool,
    pub entries: Vec<CatalogEntryDto>,
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
    /// Display-only, opt-in IMDb/OMDb data. `None` when unset or disabled.
    #[serde(default)]
    pub imdb_rating: Option<f64>,
    #[serde(default)]
    pub vote_count: Option<i64>,
    #[serde(default)]
    pub age_rating: Option<String>,
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
    /// Display-only, opt-in IMDb/OMDb data. `None` when unset or disabled.
    #[serde(default)]
    pub imdb_rating: Option<f64>,
    #[serde(default)]
    pub vote_count: Option<i64>,
    #[serde(default)]
    pub age_rating: Option<String>,
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
    /// Display-only, opt-in IMDb/OMDb data. `None` when unset or disabled.
    #[serde(default)]
    pub imdb_rating: Option<f64>,
    #[serde(default)]
    pub vote_count: Option<i64>,
    #[serde(default)]
    pub age_rating: Option<String>,
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
#[serde(rename_all = "camelCase")]
pub enum DownloadState {
    Downloading,
    Ready,
    Playing,
    Watched,
    CleanupPending,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadItem {
    pub id: String,
    pub torrent_hash: String,
    pub media_id: Option<String>,
    pub state: DownloadState,
    pub progress: f64,
}

// Paths stay host-internal: no renderer-facing DTO carries a filesystem path.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TorrentTask {
    pub hash: String,
    pub source: String,
    pub state: String,
    pub progress: f64,
    pub created_at: IsoTimestamp,
    pub updated_at: IsoTimestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMapping {
    pub torrent_hash: String,
    pub media_id: Option<String>,
    pub episode_no: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefetchEntry {
    pub media_id: MediaId,
    pub requested_for: MediaId,
    pub priority: i64,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupEntry {
    pub media_id: MediaId,
    pub eligible_at: IsoTimestamp,
    pub state: String,
    pub outcome: Option<String>,
}

// Legacy renderer wire values for the read-only Downloads screen; kept
// distinct from the acquisition DownloadState so the frontend contract holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyDownloadState {
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
    pub state: LegacyDownloadState,
    pub progress: f64,
    pub error_message: Option<String>,
}

/// G-8 buffer policy. Pure and shared by acquisition and the Downloads DTO so
/// the enqueue decision and the surfaced notice can never drift apart.
pub const DEFAULT_BUFFER_LIMIT_BYTES: u64 = 20 * 1024 * 1024 * 1024;
pub const BUFFER_PAUSED_NOTICE: &str = "buffer paused — storage full";
const BUFFER_PAUSE_PERCENT: u64 = 90;

/// Paused once retained bytes reach 90% of the cap. A zero cap fails closed.
pub fn is_buffer_paused(used_bytes: u64, limit_bytes: u64) -> bool {
    if limit_bytes == 0 {
        return true;
    }
    used_bytes.saturating_mul(100) >= limit_bytes.saturating_mul(BUFFER_PAUSE_PERCENT)
}

/// Renderer-facing notice for the same condition; a missing/zero configured
/// cap uses the 20 GiB default. Returns only a message string — never a path.
pub fn buffer_pause_notice(used_bytes: i64, limit_bytes: Option<i64>) -> Option<String> {
    let limit = limit_bytes
        .filter(|value| *value > 0)
        .map(|value| value as u64)
        .unwrap_or(DEFAULT_BUFFER_LIMIT_BYTES);
    if is_buffer_paused(used_bytes.max(0) as u64, limit) {
        Some(BUFFER_PAUSED_NOTICE.to_string())
    } else {
        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadsScreen {
    pub enabled: bool,
    pub tasks: Vec<DownloadTask>,
    pub storage_used_bytes: i64,
    pub storage_limit_bytes: Option<i64>,
    /// Host-authored status message only; no filesystem path is ever included.
    #[serde(default)]
    pub buffer_notice: Option<String>,
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

// R-01 / ADR-0021. Mirrors `media_acquisition::validate_source`; app-core
// cannot depend on that crate (it depends on app-core), so keep both in step.
/// Closed length cap for an operator-supplied source.
pub const MAX_ITEM_SOURCE_LEN: usize = 2048;
const ITEM_MAGNET_PREFIX: &str = "magnet:?xt=urn:btih:";
const ITEM_TORRENT_SUFFIX: &str = ".torrent";

/// Validate an operator-supplied source at the domain boundary. Accepted: a
/// `magnet:?xt=urn:btih:` URI or a **bare** `*.torrent` file name. Rejected:
/// empty, oversized, control characters and anything path-like. Error strings
/// never echo the input, so no path or credential reaches a renderer.
pub fn validate_item_source(raw: &str) -> Result<String, AppError> {
    let source = raw.trim();
    let invalid = || {
        AppError::new(
            AppErrorCode::InvalidRequest,
            "A valid source is required.",
            false,
        )
    };
    if source.is_empty() {
        return Err(AppError::new(
            AppErrorCode::InvalidRequest,
            "A source is required.",
            false,
        ));
    }
    if source.len() > MAX_ITEM_SOURCE_LEN || source.chars().any(char::is_control) {
        return Err(invalid());
    }
    let lower = source.to_ascii_lowercase();
    if lower.starts_with(ITEM_MAGNET_PREFIX) {
        return Ok(source.to_string());
    }
    if lower.ends_with(ITEM_TORRENT_SUFFIX) {
        if source.len() == ITEM_TORRENT_SUFFIX.len()
            || source.chars().any(|c| matches!(c, '/' | '\\' | ':'))
            || source.contains("..")
            || source.starts_with('.')
        {
            return Err(invalid());
        }
        return Ok(source.to_string());
    }
    Err(invalid())
}

/// Path-free display name stored beside a source. Magnets carry tracker URLs
/// that may be private, so only the bare `.torrent` name is ever surfaced.
pub fn item_source_name(source: &str) -> String {
    if source.to_ascii_lowercase().starts_with(ITEM_MAGNET_PREFIX) {
        "Magnet link".to_string()
    } else {
        source.to_string()
    }
}

/// Info hash from a magnet source, used to tie a download row by hash. `None`
/// for a bare `.torrent` name, which is matched by its stored source instead.
pub fn item_source_torrent_hash(source: &str) -> Option<&str> {
    const KEY: &str = "urn:btih:";
    let start = source.find(KEY)? + KEY.len();
    let hash = source[start..].split(['&', '#']).next()?;
    (!hash.is_empty() && hash.chars().all(|c| c.is_ascii_alphanumeric())).then_some(hash)
}

#[cfg(test)]
mod item_source_tests {
    use super::*;

    #[test]
    fn only_magnets_and_bare_torrent_names_are_accepted() {
        assert!(validate_item_source("").is_err());
        assert!(validate_item_source("   ").is_err());
        assert!(validate_item_source(&"m".repeat(MAX_ITEM_SOURCE_LEN + 1)).is_err());
        assert!(validate_item_source("not a source").is_err());
        assert!(validate_item_source("C:\\media\\ep.torrent").is_err());
        assert!(validate_item_source("/downloads/ep.torrent").is_err());
        assert!(validate_item_source("..\\ep.torrent").is_err());
        assert!(validate_item_source(".torrent").is_err());
        assert!(validate_item_source("magnet:?xt=urn:btih:ABC123&dn=ep").is_ok());
        assert_eq!(
            validate_item_source("  episode.torrent  ").expect("valid"),
            "episode.torrent"
        );
    }

    #[test]
    fn torrent_hash_is_read_from_a_magnet_without_echoing_a_path() {
        assert_eq!(
            item_source_torrent_hash("magnet:?xt=urn:btih:ABC123&dn=ep"),
            Some("ABC123")
        );
        assert_eq!(item_source_torrent_hash("ep.torrent"), None);
        assert_eq!(
            item_source_name("magnet:?xt=urn:btih:ABC123"),
            "Magnet link"
        );
        assert_eq!(item_source_name("ep.torrent"), "ep.torrent");
    }
}

// IMDb plan Task 1. Kept standalone so existing `AppSettings` / seed literals
// keep compiling; wire it into the settings document with the settings owners.
// Both flags default to false: no IMDb/OMDb egress until the operator opts in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImdbOptInSettings {
    #[serde(default)]
    pub imdb_enabled: bool,
    #[serde(default)]
    pub omdb_enabled: bool,
}

/// Backend validates by type: unknown fields are refused and no key is ever a
/// DTO field (the OMDb key comes from the env/secret store only).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImdbOptInPatch {
    pub imdb_enabled: Option<bool>,
    pub omdb_enabled: Option<bool>,
}

/// Opt-in text names exactly what is sent and to whom (plan global constraint).
pub const IMDB_OPT_IN_TEXT: &str = "Opt-in to IMDb: title/year or IMDb id is sent to the \
operator-supplied IMDb datasets or OMDb (for age ratings). Off by default; \
local NFO/TMDB data is unchanged.";

impl ImdbOptInSettings {
    pub fn set_imdb_enabled(&mut self, enabled: bool) {
        self.imdb_enabled = enabled;
    }

    pub fn set_omdb_enabled(&mut self, enabled: bool) {
        self.omdb_enabled = enabled;
    }

    /// Apply a validated patch; `None` leaves a flag as it was.
    pub fn apply(&mut self, patch: &ImdbOptInPatch) {
        if let Some(enabled) = patch.imdb_enabled {
            self.imdb_enabled = enabled;
        }
        if let Some(enabled) = patch.omdb_enabled {
            self.omdb_enabled = enabled;
        }
    }

    /// True only while the operator has explicitly enabled IMDb.
    pub fn imdb_allowed(&self) -> bool {
        self.imdb_enabled
    }

    /// True only while the operator has explicitly enabled OMDb.
    pub fn omdb_allowed(&self) -> bool {
        self.omdb_enabled
    }
}

#[cfg(test)]
mod imdb_opt_in_tests {
    use super::*;

    #[test]
    fn defaults_are_off_and_setters_are_explicit() {
        let mut settings = ImdbOptInSettings::default();
        assert!(!settings.imdb_allowed());
        assert!(!settings.omdb_allowed());
        settings.set_imdb_enabled(true);
        settings.set_omdb_enabled(true);
        assert!(settings.imdb_allowed());
        assert!(settings.omdb_allowed());
    }

    #[test]
    fn patch_leaves_unset_flags_alone_and_names_the_opt_in() {
        let mut settings = ImdbOptInSettings::default();
        settings.set_imdb_enabled(true);
        settings.apply(&ImdbOptInPatch {
            imdb_enabled: None,
            omdb_enabled: Some(true),
        });
        assert!(settings.imdb_allowed());
        assert!(settings.omdb_allowed());
        assert!(IMDB_OPT_IN_TEXT.contains("IMDb"));
        assert!(IMDB_OPT_IN_TEXT.contains("OMDb"));
    }
}
