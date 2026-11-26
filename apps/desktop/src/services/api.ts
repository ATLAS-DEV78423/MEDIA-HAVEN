/* Renderer-facing API facade. Method names and return types are unchanged;
 * every method now routes through the named-command transport. */

import type {
  AcquisitionRow,
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

/* Display-only fields from the opt-in, offline IMDb/OMDb lookup. No dataset
 * path, API key or token is part of this wire shape. */
export interface ImdbLookup {
  imdbRating?: number | null | undefined;
  voteCount?: number | null | undefined;
  ageRating?: string | null | undefined;
}

/* Display-only discovery DTOs (ADR-0024). No endpoint, key, credential or path
 * crosses the bridge. A magnet appears only in a shortlist returned by a search. */
export interface ShortlistItem {
  title: string;
  sizeBytes: number;
  seeders: number;
  resolution: string;
  source: string;
  magnet: string;
}

export interface SourceSearchResult {
  items: ShortlistItem[];
  setup_required?: boolean;
}

/** Confirming a source now starts acquisition; the id is path-free. */
export interface SourceSetResult {
  downloadId: string;
}

export interface CatalogItem {
  id: string;
  title: string;
  year: number | null;
  kind: "movie" | "series";
  overview: string;
  posterPath: string | null;
  posterUrl?: string | null;
  provider?: "tmdb" | "anilist";
}

export interface CatalogResult {
  items: CatalogItem[];
  setup_required?: boolean;
}

const NO_QUERY: BrowseQuery = { sort: "recent", genre: null, limit: 200, offset: 0 };

export const api = {
  ready: () => call<{ status: string }>("settings_get", {}),

  home: () => call<HomeScreen>("library_home", {}),

  movies: (query: BrowseQuery = NO_QUERY) =>
    call<MediaSummary[]>("library_movies", { query }),

  shows: (query: BrowseQuery = NO_QUERY) =>
    call<SeriesDetail[]>("library_shows", { query }),

  detail: async (id: string): Promise<MediaDetail> => {
    const base = await call<MediaDetail>("library_detail", { id });
    try {
      const imdb = await call<ImdbLookup>("imdb_lookup", { media_id: id });
      return { ...base, ...imdb } as MediaDetail;
    } catch {
      // The offline opt-in lookup is additive: a failure (including opt-in off
      // on the backend) must never break the local detail view.
      return base;
    }
  },

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

  downloadStatus: () => call<AcquisitionRow[]>("download_status", {}),

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

  /** Attach an operator-supplied source to one item and start acquisition (R-01 / ADR-0021). */
  sourceSet: (mediaId: string, source: string) =>
    call<SourceSetResult>("source_set", { media_id: mediaId, source }),

  /** Ranked candidates from the operator's search endpoint (ADR-0024). */
  sourceSearch: (title: string, year: number | null) =>
    call<SourceSearchResult>("source_search", { title, year }),

  catalogBrowse: (kind: "movie" | "tv") =>
    call<CatalogResult>("catalog_browse", { kind }),

  catalogSearch: (query: string) =>
    call<CatalogResult>("catalog_search", { query }),

  catalogAnime: (query: string | null = null) =>
    call<CatalogResult>("catalog_anime", { query }),

  catalogWantedAdd: (title: string, year: number | null, kind: "movie" | "series") =>
    call<{ mediaId: string }>("catalog_wanted_add", { title, year, kind }),
};
