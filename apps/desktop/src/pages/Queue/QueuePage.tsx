import { useCallback, useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";

import { ActionButton, buttonClass } from "@/components/ui/Controls";
import { Icon } from "@/components/ui/Icon";
import { EmptyState, PageHeader, Panel } from "@/components/ui/Surfaces";
import { artFor } from "@/data/catalog";
import { api, describeError, type QueueEntry } from "@/services/api";
import { useAppState } from "@/state/AppState";
import styles from "./queue.module.css";

const STATE_LABEL: Record<QueueEntry["state"], string> = {
  pending: "Waiting",
  active: "Playing",
  done: "Finished",
  failed: "Failed",
};

export function QueuePage() {
  const { data } = useAppState();
  const [entries, setEntries] = useState<QueueEntry[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [confirmId, setConfirmId] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState("");

  const load = useCallback(async () => {
    try {
      const screen = await api.queueList();
      setEntries(screen.entries);
      setError(null);
    } catch (cause) {
      setError(describeError(cause));
    }
  }, []);

  useEffect(() => {
    // Deferred so the first paint is the empty shell, not a cascading render.
    const timer = window.setTimeout(() => {
      void load();
    }, 0);
    return () => window.clearTimeout(timer);
  }, [load]);

  const candidates = useMemo(() => {
    const movies = data?.movies ?? [];
    const shows = data?.shows ?? [];
    return [
      ...movies.map((movie) => ({ id: movie.id, title: movie.title })),
      ...shows.map((show) => ({ id: show.id, title: show.title })),
    ];
  }, [data]);

  const run = useCallback(
    async (action: () => Promise<unknown>) => {
      setBusy(true);
      try {
        await action();
        await load();
        setError(null);
      } catch (cause) {
        setError(describeError(cause));
      } finally {
        setBusy(false);
      }
    },
    [load],
  );

  const add = () => {
    if (!selectedId || busy) return;
    void run(() => api.queueAdd(selectedId));
  };

  const move = (entry: QueueEntry, delta: number) => {
    const ids = entries.map((item) => item.id);
    const from = ids.indexOf(entry.id);
    const to = from + delta;
    if (from < 0 || to < 0 || to >= ids.length) return;
    const ordered = [...ids.slice(0, from), ...ids.slice(from + 1)];
    ordered.splice(to, 0, entry.id);
    void run(() => api.queueReorder(ordered));
  };

  const remove = (entry: QueueEntry) => {
    setConfirmId(null);
    void run(() => api.queueRemove(entry.id));
  };

  return (
    <div className={`page ${styles.page}`}>
      <PageHeader
        title="Queue"
        description="Your play order, kept in the local library database. The queue never advances on its own."
      />

      <div className={styles.body}>
        {error ? (
          <div className={styles.error} role="alert">
            <span>{error}</span>
            <ActionButton variant="glass" onClick={() => void load()}>
              Retry
            </ActionButton>
          </div>
        ) : null}

        <Panel tone="quiet">
          <div className={styles.addRow}>
            <label className={styles.addLabel} htmlFor="queue-add-title">
              Add a title
            </label>
            <select
              id="queue-add-title"
              className={styles.addSelect}
              value={selectedId}
              onChange={(event) => setSelectedId(event.target.value)}
              disabled={busy || candidates.length === 0}
              data-focusable="true"
            >
              <option value="">Choose from your library…</option>
              {candidates.map((candidate) => (
                <option key={candidate.id} value={candidate.id}>
                  {candidate.title}
                </option>
              ))}
            </select>
            <ActionButton
              variant="solid"
              icon="plus"
              onClick={add}
              disabled={busy || !selectedId}
            >
              Add to queue
            </ActionButton>
          </div>
          <p className={styles.hint}>
            Adding the same title again is harmless: the backend keeps one entry.
            No title is downloaded or removed by adding it here.
          </p>
          {candidates.length === 0 ? (
            <p className={styles.hint}>
              There is nothing in the library to add yet.{" "}
              <Link to="/library" data-focusable="true">
                Scan a library root
              </Link>
              .
            </p>
          ) : null}
        </Panel>

        <section className={styles.listSection} aria-labelledby="queue-heading">
          <h2 id="queue-heading" className={styles.listTitle}>
            Up next
          </h2>

          {entries.length === 0 ? (
            <EmptyState
              title="Your queue is empty"
              body="Pick a title above, or open any title in the library and add it to the queue. Nothing will play until you choose it."
              action={
                <Link to="/movies" className={buttonClass("solid")} data-focusable="true">
                  Browse movies
                </Link>
              }
            />
          ) : (
            <ol className={styles.list}>
              {entries.map((entry, index) => (
                <li key={entry.id} className={styles.item}>
                  <span className={styles.position} aria-hidden="true">
                    {index + 1}
                  </span>
                  <span className={styles.thumb}>
                    <img
                      src={artFor(entry.media.id, entry.media.title).still}
                      alt=""
                      loading="lazy"
                    />
                  </span>
                  <div className={styles.itemCopy}>
                    <span className={styles.itemTitle}>{entry.media.title}</span>
                    <span className={styles.itemMeta}>
                      {STATE_LABEL[entry.state]}
                      {entry.media.kind === "series" ? " · Series" : " · Movie"}
                    </span>
                  </div>

                  {confirmId === entry.id ? (
                    <div className={styles.confirm}>
                      <span className={styles.confirmText}>Remove this entry?</span>
                      <button
                        type="button"
                        className={styles.confirmRemove}
                        onClick={() => remove(entry)}
                        disabled={busy}
                        data-focusable="true"
                      >
                        Remove
                      </button>
                      <button
                        type="button"
                        className={styles.confirmCancel}
                        onClick={() => setConfirmId(null)}
                        data-focusable="true"
                      >
                        Cancel
                      </button>
                    </div>
                  ) : (
                    <div className={styles.controls}>
                      <button
                        type="button"
                        className={buttonClass("quiet", styles.iconButton)}
                        onClick={() => move(entry, -1)}
                        disabled={busy || index === 0}
                        aria-label={`Move ${entry.media.title} up`}
                        data-focusable="true"
                      >
                        <Icon name="chevronLeft" />
                      </button>
                      <button
                        type="button"
                        className={buttonClass("quiet", styles.iconButton)}
                        onClick={() => move(entry, 1)}
                        disabled={busy || index === entries.length - 1}
                        aria-label={`Move ${entry.media.title} down`}
                        data-focusable="true"
                      >
                        <Icon name="chevronRight" />
                      </button>
                      <button
                        type="button"
                        className={buttonClass("quiet", styles.iconButton)}
                        onClick={() => setConfirmId(entry.id)}
                        disabled={busy}
                        aria-label={`Remove ${entry.media.title} from queue`}
                        data-focusable="true"
                      >
                        <Icon name="trash" />
                      </button>
                    </div>
                  )}
                </li>
              ))}
            </ol>
          )}
        </section>

        <Panel tone="quiet">
          <p className={styles.note}>
            Nothing is downloading. This queue only decides which title is offered
            next; it never fetches a file, changes a library file, or starts playback
            by itself. Removing an entry removes the queue item only — no media file
            is touched.
          </p>
        </Panel>
      </div>
    </div>
  );
}
