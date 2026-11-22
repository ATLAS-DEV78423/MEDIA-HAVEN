/* Response shapes of the `mediahaven-core` HTTP API.
   Field names mirror the Rust DTOs exactly (serde camelCase). */

export type BackendKind = "movie" | "series" | "episode";

export interface ArtworkRef {
  variant: "poster" | "backdrop" | "still";
  url: string | null;
  alt: string;
}

/* Display-only TMDB discovery catalogue (camelCase wire shape). No token, host
 * or upstream URL is carried — posters are the same relative hashed refs as
 * the library's. */
export type CatalogKind = "movie" | "tv" | "anime";

export interface CatalogEntry {
  id: string;
  kind: CatalogKind;
  title: string;
  year: number | null;
  synopsis: string | null;
  rating: number | null;
  artwork: ArtworkRef[];
}

export interface CatalogScreen {
  available: boolean;
  entries: CatalogEntry[];
}

export interface MediaAvailability {
  state: "available" | "missing-file" | "offline";
  lastSeenAt?: string | null;
}

export interface PlaybackProgress {
  positionSeconds: number;
  durationSeconds: number;
  updatedAt: string;
  completed: boolean;
}

export interface MediaSummary {
  id: string;
  kind: BackendKind;
  title: string;
  year: number | null;
  synopsis: string | null;
  runtimeMinutes: number | null;
  genres: string[];
  artwork: ArtworkRef[];
  availability: MediaAvailability;
  progress: PlaybackProgress | null;
  /** Display-only IMDb data. Absent/null until opted in and cached. */
  imdbRating?: number | null;
  voteCount?: number | null;
  ageRating?: string | null;
}

export interface MovieDetail {
  id: string;
  kind: "movie";
  title: string;
  year: number | null;
  synopsis: string | null;
  runtimeMinutes: number | null;
  genres: string[];
  artwork: ArtworkRef[];
  availability: MediaAvailability;
  progress: PlaybackProgress | null;
  director: string | null;
  cast: string[];
  /** Display-only IMDb data. Absent/null until opted in and cached. */
  imdbRating?: number | null;
  voteCount?: number | null;
  ageRating?: string | null;
}

export interface SeasonSummary {
  number: number;
  title: string;
  episodeCount: number;
}

export interface SeriesDetail {
  id: string;
  kind: "series";
  title: string;
  year: number | null;
  synopsis: string | null;
  genres: string[];
  artwork: ArtworkRef[];
  seasons: SeasonSummary[];
  /** Display-only IMDb data. Absent/null until opted in and cached. */
  imdbRating?: number | null;
  voteCount?: number | null;
  ageRating?: string | null;
}

export type MediaDetail = MovieDetail | SeriesDetail;

export interface EpisodeSummary {
  id: string;
  seriesId: string;
  seriesTitle: string;
  seasonNumber: number;
  episodeNumber: number;
  title: string;
  runtimeMinutes: number | null;
  artwork: ArtworkRef[];
  availability: MediaAvailability;
  progress: PlaybackProgress | null;
  nextUp: boolean;
}

export interface MediaShelf {
  id: string;
  title: string;
  items: MediaSummary[];
}

export interface HeroAction {
  kind: "play" | "resume" | "details" | "queue";
  label: string;
}

export interface HeroFeature {
  media: MediaSummary;
  headline: string;
  actions: HeroAction[];
}

export interface HomeScreen {
  hero: HeroFeature | null;
  shelves: MediaShelf[];
}

export interface LibraryRoot {
  id: string;
  displayName: string;
  itemCount: number;
  status: "ok" | "scanning" | "offline" | "error";
  lastScanAt: string | null;
  errorMessage: string | null;
  path: string | null;
}

export interface LibraryCounts {
  movies: number;
  shows: number;
  episodes: number;
}

export interface LibraryScreen {
  roots: LibraryRoot[];
  counts: LibraryCounts;
}

export interface SearchQuery {
  text: string;
  kinds: BackendKind[];
  genres: string[];
}

export interface SearchResults {
  query: SearchQuery;
  items: MediaSummary[];
  total: number;
  truncated: boolean;
}

export interface WatchlistEntry {
  id: string;
  media: MediaSummary;
  position: number;
  addedAt: string;
}

export interface WatchlistScreen {
  entries: WatchlistEntry[];
}

export interface DownloadTask {
  id: string;
  title: string;
  state: "queued" | "downloading" | "paused" | "verifying" | "available" | "failed";
  progress: number;
  errorMessage: string | null;
}

export interface DownloadsScreen {
  enabled: boolean;
  tasks: DownloadTask[];
  storageUsedBytes: number;
  storageLimitBytes: number | null;
  /** Host-authored buffer status message (no path). Null while the buffer runs. */
  bufferNotice: string | null;
}

/* Acquisition rows (Rust `DownloadItem`, camelCase, path-free). */

/** Display-only verification state. Never carries a path or a scanner detail. */
export type MediaStatus = "scanning" | "blocked" | "ready" | "unavailable";

export type AcquisitionState =
  | "downloading"
  | "ready"
  | "playing"
  | "watched"
  | "cleanupPending"
  | "error";

export interface AcquisitionRow {
  id: string;
  torrentHash: string;
  mediaId: string | null;
  state: AcquisitionState;
  progress: number;
  /** Honest, path-free verification state; absent until the gates report. */
  mediaStatus?: MediaStatus;
}

export interface Profile {
  id: string;
  name: string;
  avatarColor: string;
  createdAt: string;
}

export type RuntimeStatus =
  | { state: "unknown" | "absent" | "stopped" | "starting" | "ready" }
  | { state: "unhealthy"; message: string };

export interface AppSettings {
  libraryRoots: LibraryRoot[];
  playback: {
    preferredLanguage: string;
    subtitleSize: string;
    autoplayNext: boolean;
  };
  storage: {
    rootDisplayName: string;
    limitBytes: number | null;
  };
  optionalServices: {
    downloadsEnabled: boolean;
    metadataProvider: string | null;
  };
  diagnostics: {
    runtimeStatus: RuntimeStatus;
    backendStatus: RuntimeStatus;
  };
  profiles: Profile[];
}

export interface SettingsPatch {
  playback?: {
    preferredLanguage?: string;
    subtitleSize?: string;
    autoplayNext?: boolean;
  };
  storage?: {
    rootDisplayName?: string;
    limitBytes?: number;
  };
  profiles?: Profile[];
}

export interface TrackOption {
  id: string;
  label: string;
  language: string | null;
  selected: boolean;
}

export interface PlaybackSession {
  sessionId: string;
  media: MediaSummary;
  startPositionSeconds: number;
  durationSeconds: number;
  subtitleTracks: TrackOption[];
  audioTracks: TrackOption[];
  canResume: boolean;
}

export interface ScanOutcome {
  scanned: number;
  cancelled: boolean;
  errors: string[];
  duplicates: { hash: string; paths: string[] }[];
}

export interface BrowseQuery {
  sort: "recent" | "title" | "year";
  genre: string | null;
  limit: number;
  offset: number;
}

/** Named `source_set` command payload. Operator-supplied, never a path. */
export interface SourceSetRequest {
  media_id: string;
  source: string;
}
