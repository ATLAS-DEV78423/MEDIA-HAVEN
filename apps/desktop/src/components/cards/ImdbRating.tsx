/* Display-only IMDb/OMDb fields. A missing or null value renders NOTHING —
 * the app never invents a rating when opt-in is off or the cache is empty.
 * The attribution constant mirrors `IMDB_ATTRIBUTION` in crates/metadata. */

import styles from "./imdbRating.module.css";

export const IMDB_ATTRIBUTION = "Ratings and age ratings: IMDb / OMDb";

export interface ImdbDisplayFields {
  // `undefined` is explicit: absent, null and unusable all render nothing.
  imdbRating?: number | null | undefined;
  voteCount?: number | null | undefined;
  ageRating?: string | null | undefined;
}

export function ImdbRating({
  imdbRating,
  voteCount,
  ageRating,
  compact = false,
}: ImdbDisplayFields & { compact?: boolean }) {
  const score =
    typeof imdbRating === "number" && Number.isFinite(imdbRating)
      ? imdbRating.toFixed(1)
      : null;
  const votes =
    typeof voteCount === "number" && Number.isFinite(voteCount)
      ? ` (${voteCount.toLocaleString()})`
      : "";
  const cert =
    typeof ageRating === "string" && ageRating.trim() ? ageRating.trim() : null;

  if (score === null && cert === null) return null;

  return (
    <span
      className={`${styles.root} ${compact ? styles.compact : ""}`}
      data-testid="imdb-rating"
    >
      {score !== null ? (
        <span className={styles.score}>
          <span aria-hidden="true">★</span> {score}
          {votes ? <span className={styles.votes}>{votes}</span> : null}
        </span>
      ) : null}
      {cert !== null ? <span className={styles.cert}>{cert}</span> : null}
      <span className={styles.attribution}>{IMDB_ATTRIBUTION}</span>
    </span>
  );
}
