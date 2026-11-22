/* Credit for the artwork providers. Either TMDB or IMDb may supply a poster,
 * so both exact attribution lines render in one caption. The class is the
 * shared, token-only attribution style from imdbRating.module.css, so this
 * adds no colour/spacing literal of its own (AGENTS.md / Development.md §7.3). */

import styles from "./imdbRating.module.css";

export const TMDB_ATTRIBUTION_FRONTEND =
  "This product uses the TMDB API but is not endorsed or certified by TMDB.";
export const IMDB_POSTER_ATTRIBUTION = "Posters: IMDb";

export function TmdbAttribution() {
  return (
    <span className={styles.attribution}>
      <span>{TMDB_ATTRIBUTION_FRONTEND}</span>
      <br />
      <span>{IMDB_POSTER_ATTRIBUTION}</span>
    </span>
  );
}
