import { useMemo, useState, type ReactNode } from "react";

import { MediaCard } from "@/components/cards/MediaCard";
import { Chip, Segmented } from "@/components/ui/Controls";
import { EmptyState, PageHeader } from "@/components/ui/Surfaces";
import type { Media } from "@/data/catalog";
import styles from "./browse.module.css";

type Sort = "recent" | "title" | "year";

const sortOptions: { value: Sort; label: string }[] = [
  { value: "recent", label: "Recent" },
  { value: "title", label: "A–Z" },
  { value: "year", label: "Year" },
];

const cardLabel = (media: Media): string => {
  if (media.kind === "series") {
    return media.runtime ? `Series · ${media.runtime}` : "Series";
  }
  if (media.year) return `${media.year} · ${media.minutes ?? 0} min`;
  return media.runtime || "Film";
};

export function BrowseView({
  title,
  description,
  items,
  emptyTitle,
  emptyBody,
  emptyAction,
}: {
  title: string;
  description: string;
  items: Media[];
  emptyTitle: string;
  emptyBody: string;
  emptyAction?: ReactNode;
}) {
  const [genre, setGenre] = useState("All");
  const [sort, setSort] = useState<Sort>("recent");

  const genres = useMemo(
    () =>
      ["All", ...new Set(items.flatMap((media) => media.genres))].slice(0, 7),
    [items],
  );

  const visible = useMemo(() => {
    const filtered =
      genre === "All"
        ? items
        : items.filter((media) => media.genres.includes(genre));
    if (sort === "title") {
      return [...filtered].sort((a, b) => a.title.localeCompare(b.title));
    }
    if (sort === "year") {
      return [...filtered].sort((a, b) => (b.year ?? 0) - (a.year ?? 0));
    }
    // The backend already orders by recency; keep that order.
    return filtered;
  }, [items, genre, sort]);

  return (
    <div className={`page ${styles.page}`}>
      <PageHeader
        title={title}
        description={description}
        meta={`${items.length} ${
          items.length === 1 ? "title" : "titles"
        } on this machine`}
      />

      {items.length === 0 ? (
        <EmptyState
          title={emptyTitle}
          body={emptyBody}
          {...(emptyAction ? { action: emptyAction } : {})}
        />
      ) : (
        <>
          <div className={styles.filters}>
            <div className={styles.chips} role="group" aria-label="Filter by genre">
              {genres.map((option) => (
                <Chip
                  key={option}
                  active={option === genre}
                  onClick={() => setGenre(option)}
                >
                  {option}
                </Chip>
              ))}
            </div>
            <div className={styles.sort}>
              <span className={styles.sortLabel}>Sort</span>
              <Segmented
                label="Sort titles"
                options={sortOptions}
                value={sort}
                onChange={setSort}
              />
            </div>
          </div>

          {visible.length === 0 ? (
            <EmptyState
              title="Nothing in this genre yet"
              body="Try another genre, or clear the filter to see everything in this category."
              action={
                <Chip active onClick={() => setGenre("All")}>
                  Show all
                </Chip>
              }
            />
          ) : (
            <div className={styles.grid}>
              {visible.map((media) => (
                <MediaCard
                  key={media.id}
                  media={media}
                  fill
                  label={cardLabel(media)}
                  {...(media.progress !== undefined
                    ? { progress: media.progress }
                    : {})}
                />
              ))}
            </div>
          )}
        </>
      )}
    </div>
  );
}
