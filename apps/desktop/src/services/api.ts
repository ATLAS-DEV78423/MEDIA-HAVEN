/* Renderer-facing API facade. Method names and return types are unchanged;
 * every method now routes through the named-command transport. */

import type {
  AppSettings,
  BrowseQuery,
  DownloadsScreen,
  EpisodeSummary,
  HomeScreen,
  LibraryRoot,
  LibraryScreen,
  MediaDetail,
  MediaSummary,
  PlaybackSession,
  ScanOutcome,
  SearchResults,
  SeriesDetail,
  SettingsPatch,
  WatchlistScreen,
} from "./types";
import { call } from "./transport";

export { apiBase, ApiFailure, describeError } from "./errors";

/* Queue DTOs. The Rust wire types live in app-core's domain/media.rs; they are
 * declared here rather than in types.ts because that file is owned by another
 * workstream in this change set. */
export type QueueEntryState = "pending" | "active" | "done" | "failed";

export interface QueueEntry {
  id: string;
  media: MediaSummary;
  position: number;
  state: QueueEntryState;
}

export interface QueueScreen {
  entries: QueueEntry[];
}

const NO_QUERY: BrowseQuery = { sort: "recent", genre: null, limit: 200, offset: 0 };

export const api = {
  ready: () => call<{ status: string }>("settings_get", {}),

  home: () => call<HomeScreen>("library_home", {}),

  movies: (query: BrowseQuery = NO_QUERY) =>
    call<MediaSummary[]>("library_movies", { query }),

  shows: (query: BrowseQuery = NO_QUERY) =>
    call<SeriesDetail[]>("library_shows", { query }),

  detail: (id: string) => call<MediaDetail>("library_detail", { id }),

  episodes: (seriesId: string, seasonNumber: number) =>
    call<EpisodeSummary[]>("library_episodes", {
      series_id: seriesId,
      season_number: seasonNumber,
    }),

  search: (text: string, genres: string[] = []) =>
    call<SearchResults>("library_search", {
      query: { text, kinds: ["movie", "series"], genres },
    }),

  roots: () => call<LibraryScreen>("library_roots", {}),

  rootAdd: (displayName: string, path: string) =>
    call<LibraryRoot>("library_root_add", { display_name: displayName, path }),

  rootRemove: (rootId: string) => call<null>("library_root_remove", { root_id: rootId }),

  scan: (rootId: string) => call<ScanOutcome>("library_scan", { root_id: rootId }),

  watchlist: () => call<WatchlistScreen>("library_watchlist", {}),

  watchlistAdd: (mediaId: string) => call<null>("watchlist_add", { media_id: mediaId }),

  watchlistRemove: (mediaId: string) =>
    call<null>("watchlist_remove", { media_id: mediaId }),

  queueList: () => call<QueueScreen>("queue_list", {}),

  queueAdd: (mediaId: string) => call<null>("queue_add", { media_id: mediaId }),

  queueRemove: (queueId: string) => call<null>("queue_remove", { queue_id: queueId }),

  queueReorder: (orderedIds: string[]) =>
    call<null>("queue_reorder", { ordered_ids: orderedIds }),

  downloads: () => call<DownloadsScreen>("downloads_list", {}),

  settings: () => call<AppSettings>("settings_get", {}),

  settingsSet: (patch: SettingsPatch) => call<AppSettings>("settings_set", { patch }),

  playbackStart: (mediaId: string) =>
    call<PlaybackSession>("playback_start", {
      request: { mediaId, startPositionSeconds: null },
    }),

  playbackStop: () => call<null>("playback_stop", {}),

  checkpoint: (mediaId: string, positionSeconds: number, durationSeconds: number) =>
    call<null>("playback_checkpoint", {
      request: { mediaId, positionSeconds, durationSeconds },
    }),
};
