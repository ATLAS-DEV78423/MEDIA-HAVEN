/**
 * MEDIA_HAVEN frontend contract.
 *
 * This is the single, frozen description of everything the UI may see and
 * call. It is deliberately independent of where the data comes from: the same
 * types are served by the development mock today and by the containerised
 * backend later (ADR-0009, roadmap P4-06).
 *
 * Rules this file exists to enforce:
 *  - No DTO exposes a raw filesystem path the UI could act on.
 *  - No command is a generic passthrough. The surface is named operations.
 *  - Errors are a closed, typed set with user-visible messages.
 *  - Nothing here imports from the domain layer, the transport, or React.
 */

/* ------------------------------------------------------------------ *
 * Identifiers
 * ------------------------------------------------------------------ */

export type MediaId = string;
export type CollectionId = string;
export type IsoTimestamp = string;

/* ------------------------------------------------------------------ *
 * Core media
 * ------------------------------------------------------------------ */

export type MediaKind = "movie" | "series" | "episode";

export type ArtworkVariant = "poster" | "backdrop" | "still";

export interface ArtworkRef {
  readonly variant: ArtworkVariant;
  /** null means "no artwork for this variant" — never a broken image. */
  readonly url: string | null;
  /** Alt text for the artwork. Never empty for a meaningful image. */
  readonly alt: string;
}

/**
 * Whether the media can actually be played right now. Distinguishing
 * "available" from "missing local file" is a first-class distinction —
 * the UI must never imply a title is playable when it is not.
 */
export type MediaAvailability =
  | { readonly state: "available" }
  | { readonly state: "missing-file"; readonly lastSeenAt: IsoTimestamp | null }
  | { readonly state: "offline" };

export interface PlaybackProgress {
  readonly positionSeconds: number;
  readonly durationSeconds: number;
  readonly updatedAt: IsoTimestamp;
  readonly completed: boolean;
}

export interface MediaSummary {
  readonly id: MediaId;
  readonly kind: MediaKind;
  readonly title: string;
  readonly year: number | null;
  readonly synopsis: string | null;
  readonly runtimeMinutes: number | null;
  readonly genres: readonly string[];
  readonly artwork: readonly ArtworkRef[];
  readonly availability: MediaAvailability;
  readonly progress: PlaybackProgress | null;
}

export interface MovieDetail extends MediaSummary {
  readonly kind: "movie";
  readonly director: string | null;
  readonly cast: readonly string[];
}

export interface SeasonSummary {
  readonly number: number;
  readonly title: string;
  readonly episodeCount: number;
}

export interface SeriesDetail {
  readonly id: CollectionId;
  readonly kind: "series";
  readonly title: string;
  readonly year: number | null;
  readonly synopsis: string | null;
  readonly genres: readonly string[];
  readonly artwork: readonly ArtworkRef[];
  readonly seasons: readonly SeasonSummary[];
}

export type MediaDetail = MovieDetail | SeriesDetail;

export interface EpisodeSummary {
  readonly id: MediaId;
  readonly seriesId: CollectionId;
  readonly seriesTitle: string;
  readonly seasonNumber: number;
  readonly episodeNumber: number;
  readonly title: string;
  readonly runtimeMinutes: number | null;
  readonly artwork: readonly ArtworkRef[];
  readonly availability: MediaAvailability;
  readonly progress: PlaybackProgress | null;
  readonly nextUp: boolean;
}

/* ------------------------------------------------------------------ *
 * Shelves and the home screen
 * ------------------------------------------------------------------ */

export type ShelfId =
  | "continue-watching"
  | "up-next"
  | "recently-added-movies"
  | "in-progress"
  | "recently-added-shows"
  | "recommended"
  | "recently-played"
  | "watchlist"
  | "genres";

export interface MediaShelf {
  readonly id: ShelfId;
  readonly title: string;
  readonly items: readonly MediaSummary[];
}

export type HeroActionKind = "play" | "resume" | "details" | "queue";

export interface HeroAction {
  readonly kind: HeroActionKind;
  readonly label: string;
}

export interface HeroFeature {
  readonly media: MediaSummary;
  readonly headline: string;
  readonly actions: readonly HeroAction[];
}

export interface HomeScreen {
  readonly hero: HeroFeature | null;
  readonly shelves: readonly MediaShelf[];
}

/* ------------------------------------------------------------------ *
 * Library
 * ------------------------------------------------------------------ */

/**
 * A library root is identified by a display name and an id — never by its
 * path. The path stays on the backend.
 */
export interface LibraryRoot {
  readonly id: string;
  readonly displayName: string;
  readonly itemCount: number;
  readonly status: "ok" | "scanning" | "offline" | "error";
  readonly lastScanAt: IsoTimestamp | null;
  readonly errorMessage: string | null;
}

export interface LibraryScreen {
  readonly roots: readonly LibraryRoot[];
  readonly counts: {
    readonly movies: number;
    readonly shows: number;
    readonly episodes: number;
  };
}

/* ------------------------------------------------------------------ *
 * Search
 * ------------------------------------------------------------------ */

export interface SearchQuery {
  readonly text: string;
  readonly kinds: readonly MediaKind[];
  readonly genres: readonly string[];
}

export interface SearchResults {
  readonly query: SearchQuery;
  readonly items: readonly MediaSummary[];
  readonly total: number;
  /** True when the result set was capped for display. */
  readonly truncated: boolean;
}

/* ------------------------------------------------------------------ *
 * Queue and watchlist
 * ------------------------------------------------------------------ */

export type QueueEntryState = "pending" | "active" | "done" | "failed";

export interface QueueEntry {
  readonly id: string;
  readonly media: MediaSummary;
  readonly position: number;
  readonly state: QueueEntryState;
}

export interface QueueScreen {
  readonly entries: readonly QueueEntry[];
}

export interface WatchlistEntry {
  readonly id: string;
  readonly media: MediaSummary;
  readonly position: number;
  readonly addedAt: IsoTimestamp;
}

export interface WatchlistScreen {
  readonly entries: readonly WatchlistEntry[];
}

/* ------------------------------------------------------------------ *
 * Downloads (optional feature — never enabled by default)
 * ------------------------------------------------------------------ */

export type DownloadState =
  | "queued"
  | "downloading"
  | "paused"
  | "verifying"
  | "available"
  | "failed";

export interface DownloadTask {
  readonly id: string;
  readonly title: string;
  readonly state: DownloadState;
  /** 0..1 */
  readonly progress: number;
  readonly errorMessage: string | null;
}

export interface DownloadsScreen {
  /** False when the optional provider is not configured. */
  readonly enabled: boolean;
  readonly tasks: readonly DownloadTask[];
  readonly storageUsedBytes: number;
  readonly storageLimitBytes: number | null;
}

/* ------------------------------------------------------------------ *
 * Playback
 * ------------------------------------------------------------------ */

export interface TrackOption {
  readonly id: string;
  readonly label: string;
  readonly language: string | null;
  readonly selected: boolean;
}

export interface PlaybackSession {
  readonly sessionId: string;
  readonly media: MediaSummary;
  readonly startPositionSeconds: number;
  readonly durationSeconds: number;
  readonly subtitleTracks: readonly TrackOption[];
  readonly audioTracks: readonly TrackOption[];
  readonly canResume: boolean;
}

/* ------------------------------------------------------------------ *
 * Settings
 * ------------------------------------------------------------------ */

export type RuntimeStatus =
  | { readonly state: "unknown" }
  | { readonly state: "absent" }
  | { readonly state: "stopped" }
  | { readonly state: "starting" }
  | { readonly state: "ready" }
  | { readonly state: "unhealthy"; readonly message: string };

export interface SettingsPatch {
  readonly playback?: Partial<AppSettings["playback"]>;
  readonly storage?: Partial<AppSettings["storage"]>;
}

export interface AppSettings {
  readonly libraryRoots: readonly LibraryRoot[];
  readonly playback: {
    readonly preferredLanguage: string;
    readonly subtitleSize: "small" | "medium" | "large";
    readonly autoplayNext: boolean;
  };
  readonly storage: {
    readonly rootDisplayName: string;
    readonly limitBytes: number | null;
  };
  readonly optionalServices: {
    readonly downloadsEnabled: boolean;
    readonly metadataProvider: string | null;
  };
  readonly diagnostics: {
    readonly runtimeStatus: RuntimeStatus;
    readonly backendStatus: RuntimeStatus;
  };
}

/* ------------------------------------------------------------------ *
 * Errors — a closed set
 * ------------------------------------------------------------------ */

export type AppErrorCode =
  | "runtime_unavailable"
  | "runtime_unhealthy"
  | "backend_unreachable"
  | "credential_mismatch"
  | "library_root_offline"
  | "library_root_unreadable"
  | "media_unsupported"
  | "media_missing"
  | "storage_full"
  | "not_found"
  | "invalid_request"
  | "internal";

export interface AppError {
  readonly code: AppErrorCode;
  /** Already plain and user-visible. The UI renders this verbatim. */
  readonly message: string;
  readonly retryable: boolean;
}

/* ------------------------------------------------------------------ *
 * The command surface
 *
 * Named operations only. There is no generic "call the backend" command;
 * ADR-0005 forbids it and this interface is where that rule lives.
 * ------------------------------------------------------------------ */

export interface BrowseQuery {
  readonly sort: "recent" | "title" | "year";
  readonly genre: string | null;
  readonly limit: number;
  readonly offset: number;
}

export interface PlaybackStartRequest {
  readonly mediaId: MediaId;
  readonly startPositionSeconds: number | null;
}

export interface Commands {
  readonly library_home: () => Promise<HomeScreen>;
  readonly library_movies: (query: BrowseQuery) => Promise<readonly MediaSummary[]>;
  readonly library_shows: (query: BrowseQuery) => Promise<readonly SeriesDetail[]>;
  readonly library_detail: (id: string) => Promise<MediaDetail>;
  readonly library_episodes: (
    seriesId: CollectionId,
    seasonNumber: number,
  ) => Promise<readonly EpisodeSummary[]>;
  readonly library_search: (query: SearchQuery) => Promise<SearchResults>;
  readonly library_roots: () => Promise<LibraryScreen>;
  readonly library_watchlist: () => Promise<WatchlistScreen>;
  readonly queue_list: () => Promise<QueueScreen>;
  readonly downloads_list: () => Promise<DownloadsScreen>;
  readonly settings_get: () => Promise<AppSettings>;
  readonly settings_set: (patch: SettingsPatch) => Promise<AppSettings>;
  readonly playback_start: (request: PlaybackStartRequest) => Promise<PlaybackSession>;
  readonly playback_stop: () => Promise<void>;
  readonly watchlist_add: (mediaId: MediaId) => Promise<void>;
  readonly watchlist_remove: (mediaId: MediaId) => Promise<void>;
  readonly watchlist_reorder: (orderedIds: readonly MediaId[]) => Promise<void>;
  readonly queue_add: (mediaId: MediaId) => Promise<void>;
  readonly queue_remove: (queueId: string) => Promise<void>;
  readonly queue_reorder: (orderedIds: readonly string[]) => Promise<void>;
}

/* ------------------------------------------------------------------ *
 * Helpers used across the UI
 * ------------------------------------------------------------------ */

export function posterOf(media: {
  readonly artwork: readonly ArtworkRef[];
}): ArtworkRef | null {
  return media.artwork.find((a) => a.variant === "poster") ?? null;
}

export function backdropOf(media: {
  readonly artwork: readonly ArtworkRef[];
}): ArtworkRef | null {
  return media.artwork.find((a) => a.variant === "backdrop") ?? null;
}

export function stillOf(media: {
  readonly artwork: readonly ArtworkRef[];
}): ArtworkRef | null {
  return media.artwork.find((a) => a.variant === "still") ?? null;
}

export function isPlayable(media: MediaSummary): boolean {
  return media.availability.state === "available";
}

export function formatRuntime(minutes: number | null): string | null {
  if (minutes === null) return null;
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  if (hours === 0) return `${rest}m`;
  if (rest === 0) return `${hours}h`;
  return `${hours}h ${rest}m`;
}

export function formatTimestamp(seconds: number): string {
  const safe = Math.max(0, Math.floor(seconds));
  const hours = Math.floor(safe / 3600);
  const minutes = Math.floor((safe % 3600) / 60);
  const rest = safe % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return hours > 0
    ? `${hours}:${pad(minutes)}:${pad(rest)}`
    : `${minutes}:${pad(rest)}`;
}

export function progressRatio(progress: PlaybackProgress | null): number {
  if (!progress || progress.durationSeconds <= 0) return 0;
  return Math.min(1, Math.max(0, progress.positionSeconds / progress.durationSeconds));
}
