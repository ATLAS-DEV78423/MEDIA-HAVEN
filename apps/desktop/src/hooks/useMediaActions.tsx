import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import type { MediaSummary, PlaybackSession } from "@/types/media";
import { getCommands } from "@/services/commands";
import { MediaDetails } from "@/components/media/MediaDetails";
import { Player } from "@/components/player/Player";

interface MediaActionsValue {
  /** Opens the details overlay for a media item. */
  readonly openDetails: (media: MediaSummary) => void;
  /** Starts playback, optionally resuming from a position. */
  readonly play: (media: MediaSummary, startPositionSeconds?: number) => void;
  readonly addToQueue: (media: MediaSummary) => void;
  readonly toggleWatchlist: (media: MediaSummary) => void;
  readonly inWatchlist: (mediaId: string) => boolean;
  /** True when the player is open. */
  readonly playing: boolean;
}

const MediaActionsContext = createContext<MediaActionsValue | null>(null);

export function useMediaActions(): MediaActionsValue {
  const value = useContext(MediaActionsContext);
  if (!value) {
    throw new Error("useMediaActions must be used inside <MediaActionsProvider>.");
  }
  return value;
}

export function MediaActionsProvider({
  children,
  initialWatchlistIds = [],
}: {
  readonly children: ReactNode;
  readonly initialWatchlistIds?: readonly string[];
}) {
  const [detailsId, setDetailsId] = useState<string | null>(null);
  const [session, setSession] = useState<PlaybackSession | null>(null);
  const [watchlistIds, setWatchlistIds] = useState<ReadonlySet<string>>(
    () => new Set(initialWatchlistIds),
  );
  const [pendingPlay, setPendingPlay] = useState<{
    readonly media: MediaSummary;
    readonly startPositionSeconds: number;
  } | null>(null);

  const openDetails = useCallback((media: MediaSummary) => {
    setDetailsId(media.id);
  }, []);

  const play = useCallback(
    (media: MediaSummary, startPositionSeconds?: number) => {
      const position =
        startPositionSeconds ?? media.progress?.positionSeconds ?? 0;
      setPendingPlay({ media, startPositionSeconds: position });
    },
    [],
  );

  // Starting playback is asynchronous; the pending request is resolved in an
  // effect, never during render.
  useEffect(() => {
    if (!pendingPlay) return;
    let cancelled = false;
    void (async () => {
      const request = pendingPlay;
      try {
        const commands = await getCommands();
        const started = await commands.playback_start({
          mediaId: request.media.id,
          startPositionSeconds: request.startPositionSeconds,
        });
        if (!cancelled) setSession(started);
      } catch {
        // The player surface reports command failures; here we simply do not
        // open it, and the page keeps its current state.
      }
      if (!cancelled) setPendingPlay(null);
    })();
    return () => {
      cancelled = true;
    };
  }, [pendingPlay]);

  const addToQueue = useCallback((media: MediaSummary) => {
    void (async () => {
      const commands = await getCommands();
      await commands.queue_add(media.id);
    })();
  }, []);

  const toggleWatchlist = useCallback((media: MediaSummary) => {
    setWatchlistIds((current) => {
      const next = new Set(current);
      const removing = next.has(media.id);
      if (removing) next.delete(media.id);
      else next.add(media.id);
      void (async () => {
        const commands = await getCommands();
        if (removing) await commands.watchlist_remove(media.id);
        else await commands.watchlist_add(media.id);
      })();
      return next;
    });
  }, []);

  const inWatchlist = useCallback(
    (mediaId: string) => watchlistIds.has(mediaId),
    [watchlistIds],
  );

  const value = useMemo<MediaActionsValue>(
    () => ({
      openDetails,
      play,
      addToQueue,
      toggleWatchlist,
      inWatchlist,
      playing: session !== null,
    }),
    [openDetails, play, addToQueue, toggleWatchlist, inWatchlist, session],
  );

  return (
    <MediaActionsContext.Provider value={value}>
      {children}
      {detailsId ? (
        <MediaDetails
          mediaId={detailsId}
          onClose={() => setDetailsId(null)}
          onPlay={(media, startPositionSeconds) => {
            setDetailsId(null);
            play(media, startPositionSeconds);
          }}
          onQueue={addToQueue}
          onWatchlist={toggleWatchlist}
          inWatchlist={inWatchlist(detailsId)}
        />
      ) : null}
      {session ? (
        <Player session={session} onClose={() => setSession(null)} />
      ) : null}
    </MediaActionsContext.Provider>
  );
}
