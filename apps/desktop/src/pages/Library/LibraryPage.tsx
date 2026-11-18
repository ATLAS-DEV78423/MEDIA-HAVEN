import { Link } from "react-router-dom";

import { MediaCard } from "@/components/cards/MediaCard";
import { Shelf } from "@/components/shelf/Shelf";
import { buttonClass } from "@/components/ui/Controls";
import { CategoryTile } from "@/components/ui/CategoryTile";
import { Icon } from "@/components/ui/Icon";
import { EmptyState, PageHeader, Panel } from "@/components/ui/Surfaces";
import { seriesToMedia, toMedia } from "@/services/adapt";
import { useAppState } from "@/state/AppState";
import styles from "./library.module.css";

const statusLabel = (status: string): string => {
  switch (status) {
    case "ok":
      return "Available";
    case "scanning":
      return "Scanning";
    case "offline":
      return "Not reachable";
    default:
      return "Error";
  }
};

export function LibraryPage() {
  const { data } = useAppState();
  if (!data) return null;

  const { roots, home, movies, shows } = data;
  const recentShelf = home.shelves.find(
    (shelf) => shelf.id === "recently-added-movies",
  );
  const recent = (recentShelf?.items ?? []).slice(0, 6).map(toMedia);
  const totalTitles = roots.counts.movies + roots.counts.shows;

  return (
    <div className={`page ${styles.page}`}>
      <PageHeader
        title="Library"
        description="One catalogue of everything the backend has indexed. MEDIA HAVEN reads the folders you allow and never moves or renames a file."
        meta={`${totalTitles} titles · ${roots.roots.length} folders · ${roots.counts.episodes} episodes`}
      />

      <div className={styles.categories}>
        <CategoryTile
          to="/movies"
          label="Movies"
          count={`${roots.counts.movies} titles`}
          icon="film"
        />
        <CategoryTile
          to="/shows"
          label="TV Shows"
          count={`${roots.counts.shows} series`}
          icon="tv"
        />
        <CategoryTile
          to="/anime"
          label="Anime"
          count="Genre shelf"
          icon="grid"
        />
        <CategoryTile
          to="/downloads"
          label="Downloads"
          count={data.downloads.enabled ? "Enabled" : "Off"}
          icon="download"
        />
      </div>

      <section className={styles.section}>
        <div className={styles.sectionHead}>
          <h2 className={styles.sectionTitle}>Folders</h2>
          <Link to="/setup" className={buttonClass("quiet")} data-focusable="true">
            Add or rescan
          </Link>
        </div>

        {roots.roots.length === 0 ? (
          <EmptyState
            title="No folders are indexed"
            body="The backend has no library roots yet. Add the folder that holds your films or series and run a scan."
            action={
              <Link to="/setup" className={buttonClass("solid")} data-focusable="true">
                Add a media folder
              </Link>
            }
          />
        ) : (
          <Panel>
            <ul className={styles.roots}>
              {roots.roots.map((root) => (
                <li key={root.id} className={styles.root}>
                  <span className={styles.rootIcon}>
                    <Icon name="folder" />
                  </span>
                  <span className={styles.rootCopy}>
                    <span className={styles.rootLabel}>{root.displayName}</span>
                    <span className={styles.rootPath}>
                      {root.path ?? "No path recorded for this root"}
                    </span>
                  </span>
                  <span className={styles.rootCount}>{root.itemCount} items</span>
                  <span
                    className={`${styles.status} ${
                      root.status === "ok" ? styles.statusOk : styles.statusOff
                    }`}
                  >
                    {statusLabel(root.status)}
                  </span>
                </li>
              ))}
            </ul>
          </Panel>
        )}
      </section>

      {recent.length > 0 ? (
        <div className={styles.recent}>
          <Shelf title="Recently Added">
            {recent.map((media) => (
              <MediaCard key={media.id} media={media} />
            ))}
          </Shelf>
        </div>
      ) : shows.length > 0 || movies.length > 0 ? (
        <div className={styles.recent}>
          <Shelf title="In this library">
            {[...movies.map(toMedia), ...shows.map(seriesToMedia)]
              .slice(0, 6)
              .map((media) => (
              <MediaCard key={media.id} media={media} />
            ))}
          </Shelf>
        </div>
      ) : null}
    </div>
  );
}
