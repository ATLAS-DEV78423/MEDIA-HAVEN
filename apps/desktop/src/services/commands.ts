/**
 * The command layer.
 *
 * The frontend's capability surface is a set of *named operations*. There is
 * no generic passthrough: `invoke("http_request", …)` would hand the UI the
 * whole API, and ADR-0005 forbids it.
 *
 * Errors are normalised here into the closed `AppError` set, so no raw
 * backend error ever reaches a component.
 */

import type {
  AppError,
  AppErrorCode,
  BrowseQuery,
  Commands,
  MediaDetail,
  MediaId,
  MediaSummary,
  PlaybackSession,
  PlaybackStartRequest,
  SearchQuery,
  SearchResults,
  SeriesDetail,
  EpisodeSummary,
  HomeScreen,
  LibraryScreen,
  QueueScreen,
  WatchlistScreen,
  DownloadsScreen,
  AppSettings,
  CollectionId,
  SettingsPatch,
} from "@/types/media";
import { createTransport, type Transport } from "./transport";

const ERROR_MESSAGES: Record<AppErrorCode, string> = {
  runtime_unavailable:
    "The local backend runtime is not available. Start MEDIA_HAVEN from a machine where the runtime is installed.",
  runtime_unhealthy:
    "The local backend is running but not healthy. Try starting it again.",
  backend_unreachable:
    "The local backend could not be reached. It may still be starting.",
  credential_mismatch:
    "The app and the backend disagree on their shared credential. Re-provisioning is required.",
  library_root_offline:
    "This folder is offline or disconnected. Its media is still in your library.",
  library_root_unreadable:
    "This folder could not be read. Check that it still exists and is accessible.",
  media_unsupported:
    "This file uses a format or codec that is not supported on this machine.",
  media_missing: "This file is no longer where it was when it was indexed.",
  storage_full: "There is not enough free space to continue.",
  not_found: "That item is no longer in your library.",
  invalid_request: "That request could not be completed.",
  internal: "Something went wrong. The details are in the diagnostics.",
};

const RETRYABLE: ReadonlySet<AppErrorCode> = new Set<AppErrorCode>([
  "runtime_unhealthy",
  "backend_unreachable",
  "library_root_offline",
  "library_root_unreadable",
  "storage_full",
  "internal",
]);

export function toAppError(cause: unknown): AppError {
  const code: AppErrorCode =
    typeof cause === "object" &&
    cause !== null &&
    "code" in cause &&
    typeof (cause as { code: unknown }).code === "string" &&
    (cause as { code: string }).code in ERROR_MESSAGES
      ? ((cause as { code: string }).code as AppErrorCode)
      : "internal";

  return {
    code,
    message: ERROR_MESSAGES[code],
    retryable: RETRYABLE.has(code),
  };
}

export class CommandError extends Error {
  readonly appError: AppError;

  constructor(appError: AppError) {
    super(appError.message);
    this.name = "CommandError";
    this.appError = appError;
  }
}

function normalise(cause: unknown): never {
  if (cause instanceof CommandError) throw cause;
  throw new CommandError(toAppError(cause));
}

export function createCommands(transport: Transport): Commands {
  const call = async <T>(command: string, args?: Record<string, unknown>) => {
    try {
      return await transport.invoke<T>(command, args);
    } catch (cause) {
      normalise(cause);
    }
  };

  return {
    library_home: () => call<HomeScreen>("library_home"),
    library_movies: (query: BrowseQuery) =>
      call<readonly MediaSummary[]>("library_movies", { query }),
    library_shows: (query: BrowseQuery) =>
      call<readonly SeriesDetail[]>("library_shows", { query }),
    library_detail: (id: string) => call<MediaDetail>("library_detail", { id }),
    library_episodes: (seriesId: CollectionId, seasonNumber: number) =>
      call<readonly EpisodeSummary[]>("library_episodes", {
        seriesId,
        seasonNumber,
      }),
    library_search: (query: SearchQuery) =>
      call<SearchResults>("library_search", { query }),
    library_roots: () => call<LibraryScreen>("library_roots"),
    library_watchlist: () => call<WatchlistScreen>("library_watchlist"),
    queue_list: () => call<QueueScreen>("queue_list"),
    downloads_list: () => call<DownloadsScreen>("downloads_list"),
    settings_get: () => call<AppSettings>("settings_get"),
    settings_set: (patch: SettingsPatch) =>
      call<AppSettings>("settings_set", { patch }),
    playback_start: (request: PlaybackStartRequest) =>
      call<PlaybackSession>("playback_start", { request }),
    playback_stop: () => call<void>("playback_stop"),
    watchlist_add: (mediaId: MediaId) => call<void>("watchlist_add", { mediaId }),
    watchlist_remove: (mediaId: MediaId) =>
      call<void>("watchlist_remove", { mediaId }),
    watchlist_reorder: (orderedIds: readonly MediaId[]) =>
      call<void>("watchlist_reorder", { orderedIds }),
    queue_add: (mediaId: MediaId) => call<void>("queue_add", { mediaId }),
    queue_remove: (queueId: string) => call<void>("queue_remove", { queueId }),
    queue_reorder: (orderedIds: readonly string[]) =>
      call<void>("queue_reorder", { orderedIds }),
  };
}

/** The application-wide command instance. */
let commandsPromise: Promise<Commands> | null = null;

export function getCommands(): Promise<Commands> {
  if (!commandsPromise) {
    commandsPromise = createTransport().then(({ transport }) =>
      createCommands(transport),
    );
  }
  return commandsPromise;
}

/** Test seam: replace the command instance. */
export function setCommands(commands: Commands | null): void {
  commandsPromise = commands ? Promise.resolve(commands) : null;
}
