import { useState } from "react";
import type { BrowseQuery, MediaSummary } from "@/types/media";
import { getCommands } from "@/services/commands";
import { useCommand } from "@/hooks/useCommand";
import { useMediaActions } from "@/hooks/useMediaActions";
import { PageHeader, EmptyState, ErrorState } from "@/components/feedback/States";
import { MediaCard } from "@/components/cards/MediaCard";

import { SkeletonCard } from "@/components/cards/ProgressBar";
import styles from "@/components/feedback/page.module.css";

const SORTS: readonly BrowseQuery["sort"][] = ["recent", "title", "year"];

export function MoviesPage() {
  const actions = useMediaActions();
  const [sort, setSort] = useState<BrowseQuery["sort"]>("recent");
  const [genre, setGenre] = useState<string | null>(null);

  const query: BrowseQuery = { sort, genre, limit: 200, offset: 0 };

  const movies = useCommand<readonly MediaSummary[]>(
    async () => (await getCommands()).library_movies(query),
    [sort, genre],
  );

  const genres = Array.from(
    new Set(movies.status === "ready" ? movies.data.flatMap((m) => m.genres) : []),
  ).sort();

  if (movies.status === "error") {
    return <ErrorState error={movies.error} onRetry={movies.reload} />;
  }

  return (
    <>
      <PageHeader
        title="Movies"
        subtitle={
          movies.status === "ready"
            ? `${movies.data.length.toLocaleString()} title${movies.data.length === 1 ? "" : "s"}`
            : "Loading…"
        }
        toolbar={
          <>
            <label className={styles.toolbar}>
              <span className={styles.subheading}>Sort</span>
              <select
                aria-label="Sort movies"
                value={sort}
                onChange={(event) => setSort(event.target.value as BrowseQuery["sort"])}
              >
                {SORTS.map((option) => (
                  <option key={option} value={option}>
                    {option === "recent" ? "Recently added" : option === "title" ? "Title" : "Year"}
                  </option>
                ))}
              </select>
            </label>
            {genres.length > 0 ? (
              <label className={styles.toolbar}>
                <span className={styles.subheading}>Genre</span>
                <select
                  aria-label="Filter by genre"
                  value={genre ?? ""}
                  onChange={(event) =>
                    setGenre(event.target.value === "" ? null : event.target.value)
                  }
                >
                  <option value="">All genres</option>
                  {genres.map((g) => (
                    <option key={g} value={g}>
                      {g}
                    </option>
                  ))}
                </select>
              </label>
            ) : null}
          </>
        }
      />

      {movies.status === "loading" ? (
        <div className={styles.grid} data-shelf aria-busy="true" aria-label="Loading movies">
          {Array.from({ length: 12 }, (_, i) => (
            <SkeletonCard key={i} />
          ))}
        </div>
      ) : null}

      {movies.status === "ready" && movies.data.length === 0 ? (
        <EmptyState
          title="No movies yet"
          body="Add a folder that holds your films. MEDIA_HAVEN reads them in place and never moves or changes them."
        />
      ) : null}

      {movies.status === "ready" && movies.data.length > 0 ? (
        <div className={styles.grid} data-shelf>
          {movies.data.map((media) => (
            <MediaCard key={media.id} media={media} onOpen={actions.openDetails} />
          ))}
        </div>
      ) : null}
    </>
  );
}
