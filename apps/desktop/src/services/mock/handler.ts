/**
 * The development mock transport.
 *
 * Serves the fixture library over the same `Transport` interface the Tauri
 * host will serve the real backend over. It implements no rules, touches no
 * filesystem and holds no database — it returns objects.
 *
 * Excluded from the production bundle: `createTransport` imports it
 * dynamically behind `import.meta.env.DEV`, which the production build
 * folds away.
 */

import type { Transport } from "@/services/transport";
import type {
  BrowseQuery,
  MediaId,
  MediaSummary,
  PlaybackSession,
  PlaybackStartRequest,
  SearchQuery,
  SearchResults,
  SettingsPatch,
} from "@/types/media";
import {
  MOCK_CONTINUE,
  MOCK_DOWNLOADS_SCREEN,
  MOCK_LIBRARY_SCREEN,
  MOCK_MOVIE_DETAILS,
  MOCK_MOVIES,
  MOCK_QUEUE_ENTRIES,
  MOCK_RECENTLY_ADDED_MOVIES,
  MOCK_RECENTLY_ADDED_SHOWS,
  MOCK_RECOMMENDED,
  MOCK_RECENTLY_PLAYED,
  MOCK_SERIES,
  MOCK_SETTINGS,
  MOCK_WATCHLIST_ENTRIES,
  MOCK_EPISODES,
  searchFixtures,
} from "./data";

/** Simulated latency, so loading states are real and visible in development. */
const LATENCY_MS = 120;

function delay<T>(value: T): Promise<T> {
  return new Promise((resolve) => {
    setTimeout(() => resolve(value), LATENCY_MS);
  });
}

function fail(code: string): never {
  throw Object.assign(new Error(code), { code });
}

function requireMovie(id: MediaId): MediaSummary {
  const found =
    MOCK_MOVIES.find((m) => m.id === id) ??
    MOCK_RECENTLY_ADDED_SHOWS.find((s) => s.id === id) ??
    MOCK_CONTINUE.find((m) => m.id === id);
  if (!found) fail("not_found");
  return found;
}

function sortMedia(items: readonly MediaSummary[], query: BrowseQuery) {
  const filtered = query.genre
    ? items.filter((m) => m.genres.includes(query.genre!))
    : items;
  const sorted = [...filtered];
  switch (query.sort) {
    case "title":
      sorted.sort((a, b) => a.title.localeCompare(b.title));
      break;
    case "year":
      sorted.sort((a, b) => (b.year ?? 0) - (a.year ?? 0));
      break;
    case "recent":
    default:
      break;
  }
  return sorted.slice(query.offset, query.offset + query.limit);
}

export function createMockTransport(): Transport {
  let watchlist = [...MOCK_WATCHLIST_ENTRIES];
  let queue = [...MOCK_QUEUE_ENTRIES];
  let settings = { ...MOCK_SETTINGS };

  const findMedia = (id: MediaId): MediaSummary | undefined =>
    MOCK_MOVIES.find((m) => m.id === id) ??
    MOCK_RECENTLY_ADDED_SHOWS.find((s) => s.id === id) ??
    MOCK_CONTINUE.find((m) => m.id === id) ??
    watchlist.find((w) => w.media.id === id)?.media;

  return {
    async invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
      switch (command) {
        case "library_home": {
          const hero = MOCK_CONTINUE[0];
          return delay({
            hero: hero
              ? {
                  media: hero,
                  headline: hero.title,
                  actions: [
                    { kind: "resume" as const, label: "Resume" },
                    { kind: "details" as const, label: "More Info" },
                    { kind: "queue" as const, label: "Add to Queue" },
                  ],
                }
              : null,
            shelves: [
              {
                id: "continue-watching" as const,
                title: "Continue Watching",
                items: MOCK_CONTINUE,
              },
              {
                id: "recently-added-movies" as const,
                title: "Recently Added Movies",
                items: MOCK_RECENTLY_ADDED_MOVIES,
              },
              {
                id: "recently-added-shows" as const,
                title: "Recently Added Shows",
                items: MOCK_RECENTLY_ADDED_SHOWS,
              },
              {
                id: "recommended" as const,
                title: "Recommended from Your Library",
                items: MOCK_RECOMMENDED,
              },
              {
                id: "recently-played" as const,
                title: "Recently Played",
                items: MOCK_RECENTLY_PLAYED,
              },
              {
                id: "watchlist" as const,
                title: "Watchlist",
                items: watchlist.map((w) => w.media),
              },
            ],
          } as T);
        }

        case "library_movies":
          return delay(
            sortMedia(MOCK_MOVIES, args?.query as BrowseQuery) as T,
          );

        case "library_shows": {
          const query = args?.query as BrowseQuery;
          const sorted = [...MOCK_SERIES].sort((a, b) =>
            query.sort === "title"
              ? a.title.localeCompare(b.title)
              : (b.year ?? 0) - (a.year ?? 0),
          );
          return delay(
            sorted
              .filter(
                (s) =>
                  !query.genre || s.genres.includes(query.genre),
              )
              .slice(query.offset, query.offset + query.limit) as T,
          );
        }

        case "library_detail": {
          const id = args?.id as string;
          const movie = MOCK_MOVIE_DETAILS.get(id);
          if (movie) return delay(movie as T);
          const series = MOCK_SERIES.find((s) => s.id === id);
          if (series) return delay(series as unknown as T);
          return fail("not_found");
        }

        case "library_episodes": {
          const seriesId = args?.seriesId as string;
          const seasonNumber = args?.seasonNumber as number;
          const episodes = MOCK_EPISODES.get(seriesId) ?? [];
          return delay(
            episodes.filter((e) => e.seasonNumber === seasonNumber) as T,
          );
        }

        case "library_search": {
          const query = args?.query as SearchQuery;
          const result = searchFixtures(query.text);
          const kinds = query.kinds;
          const genres = query.genres;
          const items =
            kinds.length === 0 && genres.length === 0
              ? result.items
              : result.items.filter(
                  (m) =>
                    (kinds.length === 0 || kinds.includes(m.kind)) &&
                    (genres.length === 0 ||
                      genres.some((g) => m.genres.includes(g))),
                );
          return delay({
            ...result,
            items,
            total: items.length,
          } as SearchResults as T);
        }

        case "library_roots":
          return delay(MOCK_LIBRARY_SCREEN as T);

        case "library_watchlist":
          return delay(
            { entries: watchlist.map((w, i) => ({ ...w, position: i })) } as T,
          );

        case "queue_list":
          return delay(
            { entries: queue.map((q, i) => ({ ...q, position: i })) } as T,
          );

        case "downloads_list":
          return delay(MOCK_DOWNLOADS_SCREEN as T);

        case "settings_get":
          return delay(MOCK_SETTINGS as T);

        case "settings_set": {
          const patch = args?.patch as SettingsPatch;
          settings = {
            ...settings,
            playback: { ...settings.playback, ...patch.playback },
            storage: { ...settings.storage, ...patch.storage },
          };
          return delay(settings as T);
        }

        case "playback_start": {
          const request = args?.request as PlaybackStartRequest;
          const media = findMedia(request.mediaId);
          if (!media) return fail("not_found");
          const durationSeconds = (media.runtimeMinutes ?? 48) * 60;
          const startPositionSeconds =
            request.startPositionSeconds ??
            media.progress?.positionSeconds ??
            0;
          const session: PlaybackSession = {
            sessionId: `sess-${media.id}`,
            media,
            startPositionSeconds,
            durationSeconds,
            canResume: startPositionSeconds > 0,
            subtitleTracks: [
              { id: "sub-0", label: "English", language: "en", selected: true },
              { id: "sub-1", label: "English (SDH)", language: "en", selected: false },
            ],
            audioTracks: [
              { id: "aud-0", label: "English 5.1", language: "en", selected: true },
              { id: "aud-1", label: "English Commentary", language: "en", selected: false },
            ],
          };
          return delay(session as T);
        }

        case "playback_stop":
          return delay(undefined as T);

        case "watchlist_add": {
          const mediaId = args?.mediaId as MediaId;
          const media = requireMovie(mediaId);
          if (!watchlist.some((w) => w.media.id === mediaId)) {
            watchlist = [
              ...watchlist,
              {
                id: `wl-${mediaId}`,
                media,
                position: watchlist.length,
                addedAt: new Date().toISOString(),
              },
            ];
          }
          return delay(undefined as T);
        }

        case "watchlist_remove": {
          const mediaId = args?.mediaId as MediaId;
          watchlist = watchlist.filter((w) => w.media.id !== mediaId);
          return delay(undefined as T);
        }

        case "watchlist_reorder": {
          const orderedIds = args?.orderedIds as readonly MediaId[];
          watchlist = orderedIds
            .map((id, index) => {
              const entry = watchlist.find((w) => w.media.id === id);
              return entry ? { ...entry, position: index } : undefined;
            })
            .filter((e): e is NonNullable<typeof e> => e !== undefined);
          return delay(undefined as T);
        }

        case "queue_add": {
          const mediaId = args?.mediaId as MediaId;
          const media = requireMovie(mediaId);
          if (!queue.some((q) => q.media.id === mediaId)) {
            queue = [
              ...queue,
              {
                id: `q-${mediaId}`,
                media,
                position: queue.length,
                state: "pending" as const,
              },
            ];
          }
          return delay(undefined as T);
        }

        case "queue_remove": {
          const queueId = args?.queueId as string;
          queue = queue.filter((q) => q.id !== queueId);
          return delay(undefined as T);
        }

        case "queue_reorder": {
          const orderedIds = args?.orderedIds as readonly string[];
          queue = orderedIds
            .map((id, index) => {
              const entry = queue.find((q) => q.id === id);
              return entry ? { ...entry, position: index } : undefined;
            })
            .filter((e): e is NonNullable<typeof e> => e !== undefined);
          return delay(undefined as T);
        }

        default:
          return fail("invalid_request");
      }
    },
  };
}
