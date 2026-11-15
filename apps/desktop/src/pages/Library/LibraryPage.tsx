import type { LibraryScreen } from "@/types/media";
import { getCommands } from "@/services/commands";
import { useCommand } from "@/hooks/useCommand";
import { PageHeader, EmptyState, ErrorState, Banner } from "@/components/feedback/States";
import { StatusPill } from "@/components/cards/ProgressBar";
import styles from "./LibraryPage.module.css";
import page from "@/components/feedback/page.module.css";

export function LibraryPage() {
  const library = useCommand<LibraryScreen>(
    async () => (await getCommands()).library_roots(),
    [],
  );

  if (library.status === "error") {
    return <ErrorState error={library.error} onRetry={library.reload} />;
  }

  if (library.status === "loading") {
    return <PageHeader title="Library" subtitle="Loading…" />;
  }

  const data = library.data;
  const offlineRoots = data.roots.filter((root) => root.status !== "ok");

  return (
    <>
      <PageHeader
        title="Library"
        subtitle={`${data.counts.movies.toLocaleString()} movies · ${data.counts.shows.toLocaleString()} shows · ${data.counts.episodes.toLocaleString()} episodes`}
      />

      {offlineRoots.length > 0 ? (
        <Banner
          title="Some folders are offline"
          body="Their media is still in your library. MEDIA_HAVEN never deletes a file because a folder became unavailable."
        />
      ) : null}

      <div className={page.section}>
        <h2 className={page.sectionTitle}>Folders</h2>
        <div className={styles.rows}>
          {data.roots.map((root) => (
            <div className={styles.row} key={root.id}>
              <div>
                <div className={styles.rowLabel}>{root.displayName}</div>
                <div className={styles.rowHint}>
                  {root.itemCount.toLocaleString()} items
                  {root.lastScanAt
                    ? ` · last scan ${new Date(root.lastScanAt).toLocaleDateString()}`
                    : ""}
                </div>
                {root.errorMessage ? (
                  <div className={styles.rowMessage}>{root.errorMessage}</div>
                ) : null}
              </div>
              <div className={styles.rowControl}>
                <StatusPill
                  tone={
                    root.status === "ok"
                      ? "available"
                      : root.status === "offline"
                        ? "offline"
                        : "missing"
                  }
                >
                  {root.status === "ok"
                    ? "Available"
                    : root.status === "scanning"
                      ? "Scanning"
                      : root.status === "offline"
                        ? "Offline"
                        : "Error"}
                </StatusPill>
              </div>
            </div>
          ))}
        </div>
      </div>

      {data.roots.length === 0 ? (
        <EmptyState
          title="No folders added yet"
          body="Add a folder that holds your films and series. MEDIA_HAVEN reads them where they are — it does not move, rename or delete them."
        />
      ) : null}
    </>
  );
}
