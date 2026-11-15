import { useState } from "react";
import type { BrowseQuery, SeriesDetail } from "@/types/media";
import { getCommands } from "@/services/commands";
import { useCommand } from "@/hooks/useCommand";
import { useMediaActions } from "@/hooks/useMediaActions";
import { PageHeader, EmptyState, ErrorState } from "@/components/feedback/States";
import { MediaCard } from "@/components/cards/MediaCard";
import { SkeletonCard } from "@/components/cards/ProgressBar";
import styles from "@/components/feedback/page.module.css";

const SORTS: readonly BrowseQuery["sort"][] = ["recent", "title", "year"];

function toSummary(series: SeriesDetail) {
  return {
    id: series.id,
    kind: "series" as const,
    title: series.title,
    year: series.year,
    synopsis: series.synopsis,
    runtimeMinutes: null,
    genres: series.genres,
    artwork: series.artwork,
    availability: { state: "available" as const },
    progress: null,
  };
}

export function ShowsPage() {
  const actions = useMediaActions();
  const [sort, setSort] = useState<BrowseQuery["sort"]>("recent");
  const [genre, setGenre] = useState<string | null>(null);

  const query: BrowseQuery = { sort, genre, limit: 200, offset: 0 };

  const shows = useCommand<readonly SeriesDetail[]>(
    async () => (await getCommands()).library_shows(query),
    [sort, genre],
  );

  const genres = Array.from(
    new Set(shows.status === "ready" ? shows.data.flatMap((s) => s.genres) : []),
  ).sort();

  if (shows.status === "error") {
    return <ErrorState error={shows.error} onRetry={shows.reload} />;
  }

  return (
    <>
      <PageHeader
        title="TV Shows"
        subtitle={
          shows.status === "ready"
            ? `${shows.data.length.toLocaleString()} series`
            : "Loading…"
        }
        toolbar={
          <>
            <label className={styles.toolbar}>
              <span className={styles.subheading}>Sort</span>
              <select
                aria-label="Sort shows"
                value={sort}
                onChange={(event) => setSort(event.target.value as BrowseQuery["sort"])}
              >
                {SORTS.map((option) => (
                  <option key={option} value={option}>
                    {option === "recent"
                      ? "Recently added"
                      : option === "title"
                        ? "Title"
                        : "Year"}
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

      {shows.status === "loading" ? (
        <div className={styles.grid} data-shelf aria-busy="true" aria-label="Loading shows">
          {Array.from({ length: 12 }, (_, i) => (
            <SkeletonCard key={i} />
          ))}
        </div>
      ) : null}

      {shows.status === "ready" && shows.data.length === 0 ? (
        <EmptyState
          title="No shows yet"
          body="Add a folder that holds your series. MEDIA_HAVEN reads them in place and never moves or changes them."
        />
      ) : null}

      {shows.status === "ready" && shows.data.length > 0 ? (
        <div className={styles.grid} data-shelf>
          {shows.data.map((series) => (
            <MediaCard
              key={series.id}
              media={toSummary(series)}
              onOpen={() => actions.openDetails(toSummary(series))}
            />
          ))}
        </div>
      ) : null}
    </>
  );
}
