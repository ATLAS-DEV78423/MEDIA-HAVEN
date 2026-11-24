import { Icon } from "@/components/ui/Icon";
import { artFor } from "@/data/catalog";
import type { CatalogItem } from "@/services/api";
import styles from "./discover.module.css";

/* Public image hosts. URLs are supplied only in the host-sanitised catalog
 * DTO; no endpoint, key or operator credential is part of an image URL. */
const TMDB_POSTER_BASE = "https://image.tmdb.org/t/p/w500";

/** One catalog title as a poster card. Play adds a wanted item and opens its
 *  Details page; this card never renders a source, local path or credential. */
export function CatalogCard({
  item,
  adding,
  onPlay,
}: {
  item: CatalogItem;
  adding: boolean;
  onPlay: () => void;
}) {
  const poster = item.posterUrl
    ? item.posterUrl
    : item.posterPath
      ? `${TMDB_POSTER_BASE}${item.posterPath}`
      : artFor(item.id, item.title).still;

  return (
    <button
      type="button"
      className={styles.card}
      onClick={onPlay}
      disabled={adding}
      aria-label={`Play ${item.title}${item.year ? `, ${item.year}` : ""}`}
      data-focusable="true"
    >
      <span className={styles.posterWrap}>
        <img
          className={styles.poster}
          src={poster}
          alt={`${item.title} poster`}
          loading="lazy"
          decoding="async"
          draggable={false}
        />
        <span className={styles.scrim} aria-hidden="true" />
        <span className={styles.kind}>
          {item.provider === "anilist" ? "Anime" : item.kind === "movie" ? "Movie" : "TV"}
        </span>
        <span className={styles.year}>{item.year ?? "Year unknown"}</span>
        <span className={styles.play} aria-hidden="true">
          <Icon name="play" />
        </span>
      </span>
      <span className={styles.meta}>
        <span className={styles.cardTitle}>{item.title}</span>
        {item.overview ? (
          <span className={styles.overview}>{item.overview}</span>
        ) : null}
        <span className={styles.cardAction}>{adding ? "Opening…" : "Play"}</span>
      </span>
    </button>
  );
}
