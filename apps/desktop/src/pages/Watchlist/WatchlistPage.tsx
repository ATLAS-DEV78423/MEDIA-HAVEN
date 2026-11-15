import { useState } from "react";
import type { WatchlistEntry } from "@/types/media";
import { getCommands } from "@/services/commands";
import { useCommand } from "@/hooks/useCommand";
import { useMediaActions } from "@/hooks/useMediaActions";
import { PageHeader, EmptyState, ErrorState } from "@/components/feedback/States";
import { MediaCard } from "@/components/cards/MediaCard";
import { SkeletonCard } from "@/components/cards/ProgressBar";
import styles from "@/components/feedback/page.module.css";

export function WatchlistPage() {
  const actions = useMediaActions();
  const watchlist = useCommand<WatchlistEntry[]>(
    async () => (await getCommands()).library_watchlist().then((s) => [...s.entries]),
    [],
  );

  const [moving, setMoving] = useState(false);

  const refresh = () => watchlist.reload();

  const remove = async (mediaId: string) => {
    const commands = await getCommands();
    await commands.watchlist_remove(mediaId);
    refresh();
  };

  const move = async (entryId: string, direction: -1 | 1) => {
    if (moving) return;
    const entries = watchlist.status === "ready" ? watchlist.data : [];
    const index = entries.findIndex((e) => e.id === entryId);
    const target = index + direction;
    if (index < 0 || target < 0 || target >= entries.length) return;
    setMoving(true);
    const reordered = [...entries];
    const [moved] = reordered.splice(index, 1);
    reordered.splice(target, 0, moved!);
    try {
      const commands = await getCommands();
      await commands.watchlist_reorder(reordered.map((e) => e.media.id));
      refresh();
    } finally {
      setMoving(false);
    }
  };

  if (watchlist.status === "error") {
    return <ErrorState error={watchlist.error} onRetry={watchlist.reload} />;
  }

  return (
    <>
      <PageHeader
        title="Watchlist"
        subtitle="Saved on this machine. Works offline."
      />

      {watchlist.status === "loading" ? (
        <div className={styles.grid} data-shelf aria-busy="true" aria-label="Loading watchlist">
          {Array.from({ length: 6 }, (_, i) => (
            <SkeletonCard key={i} />
          ))}
        </div>
      ) : null}

      {watchlist.status === "ready" && watchlist.data.length === 0 ? (
        <EmptyState
          title="Nothing saved yet"
          body="Add a film or episode to your watchlist from any card or details page. It is saved on this machine, works offline, and is separate from your play queue."
        />
      ) : null}

      {watchlist.status === "ready" && watchlist.data.length > 0 ? (
        <div className={styles.grid} data-shelf>
          {watchlist.data.map((entry, index) => (
            <div key={entry.id} style={{ position: "relative" }}>
              <MediaCard
                media={entry.media}
                onOpen={actions.openDetails}
              />
              <div
                style={{
                  display: "flex",
                  gap: "var(--space-2)",
                  marginTop: "var(--space-2)",
                }}
              >
                <button
                  type="button"
                  onClick={() => void move(entry.id, -1)}
                  disabled={moving || index === 0}
                  aria-label={`Move ${entry.media.title} up`}
                >
                  Up
                </button>
                <button
                  type="button"
                  onClick={() => void move(entry.id, 1)}
                  disabled={moving || index === watchlist.data.length - 1}
                  aria-label={`Move ${entry.media.title} down`}
                >
                  Down
                </button>
                <button
                  type="button"
                  onClick={() => void remove(entry.media.id)}
                  aria-label={`Remove ${entry.media.title} from watchlist`}
                >
                  Remove
                </button>
              </div>
            </div>
          ))}
        </div>
      ) : null}
    </>
  );
}
