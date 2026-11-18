import { Link } from "react-router-dom";

import { ProgressBar, buttonClass } from "@/components/ui/Controls";
import { EmptyState, PageHeader, Panel } from "@/components/ui/Surfaces";
import { artFor } from "@/data/catalog";
import { useAppState } from "@/state/AppState";
import styles from "./downloads.module.css";

const bytes = (value: number): string => {
  if (value <= 0) return "0 GB";
  const gigabytes = value / 1_000_000_000;
  if (gigabytes >= 1000) return `${(gigabytes / 1000).toFixed(1)} TB`;
  return `${gigabytes.toFixed(1)} GB`;
};

const stateLabel = (state: string): string => {
  switch (state) {
    case "available":
      return "On this device";
    case "downloading":
      return "Downloading";
    case "queued":
      return "Queued";
    case "paused":
      return "Paused";
    case "verifying":
      return "Verifying";
    default:
      return "Failed";
  }
};

export function DownloadsPage() {
  const { data } = useAppState();
  if (!data) return null;

  const { downloads } = data;
  const limit = downloads.storageLimitBytes;
  const used = downloads.storageUsedBytes;
  const ratio = limit && limit > 0 ? Math.min(1, used / limit) : null;

  return (
    <div className={`page ${styles.page}`}>
      <PageHeader
        title="Downloads"
        description="Titles kept on this machine so they play with the network down. A download is a second copy; the file in your library is never modified."
      />

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
                  {downloads.tasks.length}{" "}
                  {downloads.tasks.length === 1 ? "download" : "downloads"}
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
                  <dd>{downloads.tasks.length}</dd>
                </div>
              </dl>
              <p className={styles.note}>
                Figures come from the local backend. Nothing is uploaded.
              </p>
            </Panel>

            <section className={styles.listSection} aria-labelledby="offline-title">
              <h2 id="offline-title" className={styles.listTitle}>
                Available offline
              </h2>

              {downloads.tasks.length === 0 ? (
                <EmptyState
                  title="No downloads on this device"
                  body="The download service is enabled but the queue is empty."
                />
              ) : (
                <ul className={styles.list}>
                  {downloads.tasks.map((task) => (
                    <li key={task.id} className={styles.item}>
                      <span className={styles.thumb}>
                        <img src={artFor(task.id, task.title).still} alt="" loading="lazy" />
                      </span>

                      <div className={styles.itemCopy}>
                        <span className={styles.itemTitle}>{task.title}</span>
                        <span className={styles.itemMeta}>
                          {stateLabel(task.state)} · {Math.round(task.progress * 100)}%
                        </span>
                      </div>

                      <span className={styles.state}>{task.state}</span>
                      {task.errorMessage ? (
                        <span className={styles.itemMeta}>{task.errorMessage}</span>
                      ) : null}
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
                Removing a download frees that copy only. The title remains in
                the library if the source file is still present.
              </li>
              <li>
                Nothing is uploaded. Downloads never contact a catalogue or a
                content service.
              </li>
            </ul>
          </Panel>
        </section>
      </div>
    </div>
  );
}
