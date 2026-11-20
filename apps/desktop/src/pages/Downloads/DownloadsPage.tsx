import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router-dom";

import { ProgressBar, buttonClass } from "@/components/ui/Controls";
import { EmptyState, PageHeader, Panel } from "@/components/ui/Surfaces";
import type { AcquisitionRow, AcquisitionState } from "@/services/types";
import { call } from "@/services/transport";
import { useAppState } from "@/state/AppState";
import { AddSourceDialog } from "./AddSourceDialog";
import styles from "./downloads.module.css";

const bytes = (value: number): string => {
  if (value <= 0) return "0 GB";
  const gigabytes = value / 1_000_000_000;
  if (gigabytes >= 1000) return `${(gigabytes / 1000).toFixed(1)} TB`;
  return `${gigabytes.toFixed(1)} GB`;
};

const stateLabel = (state: AcquisitionState): string => {
  switch (state) {
    case "ready":
      return "Ready";
    case "downloading":
      return "Downloading";
    case "playing":
      return "Playing";
    case "watched":
      return "Watched";
    case "cleanupPending":
      return "Cleanup pending";
    default:
      return "Error";
  }
};

/** Merge by id so a repeated source can never create a second row. */
const mergeRow = (rows: AcquisitionRow[], row: AcquisitionRow): AcquisitionRow[] =>
  rows.some((existing) => existing.id === row.id)
    ? rows.map((existing) => (existing.id === row.id ? row : existing))
    : [...rows, row];

export function DownloadsPage() {
  const { data } = useAppState();
  const [rows, setRows] = useState<AcquisitionRow[]>([]);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    call<AcquisitionRow[]>("download_status", {})
      .then((items) => {
        if (!active) return;
        setRows(items);
        setLoadError(null);
      })
      .catch(() => {
        if (active) setLoadError("Download status is unavailable.");
      });
    return () => {
      active = false;
    };
  }, []);

  const closeDialog = useCallback(() => setDialogOpen(false), []);
  const addRow = useCallback(
    (row: AcquisitionRow) => setRows((current) => mergeRow(current, row)),
    [],
  );

  if (!data) return null;

  const { downloads } = data;
  const limit = downloads.storageLimitBytes;
  const used = downloads.storageUsedBytes;
  // Host-authored G-8 notice; a message string only, never a path.
  const bufferNotice = downloads.bufferNotice;
  const ratio = limit && limit > 0 ? Math.min(1, used / limit) : null;

  const remove = async (id: string) => {
    try {
      await call("download_remove", { id });
      setRows((current) => current.filter((row) => row.id !== id));
    } catch {
      setLoadError("That download could not be removed.");
    }
  };

  return (
    <div className={`page ${styles.page}`}>
      <PageHeader
        title="Downloads"
        description="Acquire media you are authorized to download. Add a magnet link or a .torrent file you already have; the app never searches for sources."
      />

      {bufferNotice ? (
        <p className={styles.error} role="status">
          {bufferNotice}
        </p>
      ) : null}

      <div className={styles.body}>
        {!downloads.enabled ? (
          <EmptyState
            title="Downloads are switched off"
            body="The backend reports the download service as disabled, so there is no offline copy to manage. Your library files are untouched and still play locally."
            action={
              <Link to="/settings" className={buttonClass("solid")} data-focusable="true">
                Open settings
              </Link>
            }
          />
        ) : (
          <>
            <Panel>
              <div className={styles.storageHead}>
                <div>
                  <h2 className={styles.storageTitle}>This device</h2>
                  <p className={styles.storageSub}>
                    {bytes(used)} used{limit ? ` of ${bytes(limit)}` : " · no limit set"}
                  </p>
                </div>
                <span className={styles.storagePill}>
                  {rows.length} {rows.length === 1 ? "download" : "downloads"}
                </span>
              </div>
              {ratio !== null ? <ProgressBar value={ratio} large /> : null}
              <dl className={styles.stats}>
                <div>
                  <dt>Downloads</dt>
                  <dd>{bytes(used)}</dd>
                </div>
                <div>
                  <dt>Limit</dt>
                  <dd>{limit ? bytes(limit) : "None"}</dd>
                </div>
                <div>
                  <dt>Tasks</dt>
                  <dd>{rows.length}</dd>
                </div>
              </dl>
              <p className={styles.note}>
                Figures come from the local backend. Nothing is uploaded.
              </p>
            </Panel>

            <section className={styles.listSection} aria-labelledby="offline-title">
              <div className={styles.listHead}>
                <h2 id="offline-title" className={styles.listTitle}>
                  Acquisitions
                </h2>
                <button
                  type="button"
                  className={buttonClass("solid")}
                  data-focusable="true"
                  onClick={() => setDialogOpen(true)}
                >
                  Add source
                </button>
              </div>

              {loadError ? (
                <p className={styles.error} role="alert">
                  {loadError}
                </p>
              ) : null}

              {rows.length === 0 ? (
                <EmptyState
                  title="No downloads yet"
                  body="Add a magnet link or a .torrent file you already have to start an acquisition."
                />
              ) : (
                <ul className={styles.list} aria-label="Acquisitions">
                  {rows.map((row) => (
                    <li key={row.id} className={styles.item}>
                      <div className={styles.itemCopy}>
                        <span className={styles.itemTitle}>{row.id}</span>
                        <span className={styles.itemMeta}>
                          {stateLabel(row.state)} · {Math.round(row.progress * 100)}%
                        </span>
                      </div>
                      <span className={styles.state}>{stateLabel(row.state)}</span>
                      <button
                        type="button"
                        className={styles.remove}
                        data-focusable="true"
                        aria-label={`Remove download ${row.id}`}
                        onClick={() => void remove(row.id)}
                      >
                        Remove
                      </button>
                    </li>
                  ))}
                </ul>
              )}
            </section>
          </>
        )}

        <section className={styles.listSection} aria-labelledby="policy-title">
          <h2 id="policy-title" className={styles.listTitle}>
            How downloads work
          </h2>
          <Panel tone="quiet">
            <ul className={styles.policy}>
              <li>
                A download is stored in MEDIA HAVEN&apos;s own folder. Your
                library file stays where it is, untouched.
              </li>
              <li>
                Removing a download frees the task and keeps its files. Source
                media under your library roots is never deleted.
              </li>
              <li>
                Only sources you supply are acquired. Nothing is uploaded, and
                no catalogue or content service is contacted.
              </li>
            </ul>
          </Panel>
        </section>
      </div>

      {dialogOpen ? (
        <AddSourceDialog onClose={closeDialog} onAdded={addRow} />
      ) : null}
    </div>
  );
}
