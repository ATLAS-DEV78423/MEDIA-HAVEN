// GENERATED from `crates/contract` — do not edit by hand.
// The command-name union reflects `mediahaven_contract::COMMANDS` and the DTOs
// mirror the Rust wire types (currently hand-mirrored in `services/types.ts`).
// Automated generation is a follow-up task that requires a codegen ADR; until
// then keep this file in sync with `crates/contract` and `services/types.ts`.

/** One named renderer command. No generic passthrough exists by design. */
export type CommandName =
  | "library_home"
  | "library_movies"
  | "library_shows"
  | "library_detail"
  | "library_episodes"
  | "library_search"
  | "library_roots"
  | "library_root_add"
  | "library_root_remove"
  | "library_scan"
  | "library_watchlist"
  | "watchlist_add"
  | "watchlist_remove"
  | "watchlist_reorder"
  | "queue_list"
  | "queue_add"
  | "queue_remove"
  | "queue_reorder"
  | "downloads_list"
  | "settings_get"
  | "settings_set"
  | "playback_start"
  | "playback_stop"
  | "playback_checkpoint";

/* Response shapes of the `mediahaven-core` HTTP API.
   Field names mirror the Rust DTOs exactly (serde camelCase). */

export type BackendKind = "movie" | "series" | "episode";

export interface ArtworkRef {
  variant: "poster" | "backdrop" | "still";
  url: string | null;
  alt: string;
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
