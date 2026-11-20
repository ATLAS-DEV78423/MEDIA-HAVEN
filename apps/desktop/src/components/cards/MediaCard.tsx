import { Link } from "react-router-dom";

import { ProgressBar } from "@/components/ui/Controls";
import { Icon } from "@/components/ui/Icon";
import type { Media } from "@/data/catalog";
import styles from "./card.module.css";
import { ImdbRating, type ImdbDisplayFields } from "./ImdbRating";

export function MediaCard({
  media,
  progress,
  label,
  variant = "still",
  badge,
  fill = false,
}: {
  media: Media & ImdbDisplayFields;
  progress?: number;
  label?: string;
  variant?: "still" | "wide";
  badge?: string;
  fill?: boolean;
}) {
  const fallback = [media.year ? String(media.year) : null, media.genres[0] ?? null]
    .filter((part): part is string => Boolean(part))
    .join(" · ");

  return (
    <Link
      to={`/details/${media.id}`}
      className={`${styles.card} ${variant === "wide" ? styles.wide : ""} ${
        fill ? styles.fill : ""
      }`}
      aria-label={`${media.title}${media.year ? `, ${media.year}` : ""}. ${media.genres.join(", ")}`}
      data-focusable="true"
    >
      <span className={styles.artWrap}>
        <img
          className={styles.art}
          src={media.art.still}
          alt=""
          loading="lazy"
          decoding="async"
          draggable={false}
        />
        <span className={styles.scrim} aria-hidden="true" />
        {badge ? <span className={styles.badge}>{badge}</span> : null}
        <span className={styles.play} aria-hidden="true">
          <Icon name="play" />
        </span>
        {progress !== undefined ? (
          <span className={styles.progressWrap}>
            <ProgressBar value={progress} />
          </span>
        ) : null}
      </span>

      <span className={styles.meta}>
        <span className={styles.title}>{media.title}</span>
        <span className={styles.sub}>{label ?? fallback}</span>
        <ImdbRating
          compact
          imdbRating={media.imdbRating}
          voteCount={media.voteCount}
          ageRating={media.ageRating}
        />
      </span>
    </Link>
  );
}
