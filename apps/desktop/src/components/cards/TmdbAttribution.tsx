/* Credit for cached IMDb posters. The class is the shared, token-only
 * attribution style from imdbRating.module.css, so this adds no colour/spacing
 * literal of its own (AGENTS.md / Development.md §7.3). */

import styles from "./imdbRating.module.css";

export const TMDB_ATTRIBUTION_FRONTEND = "Posters: IMDb";

export function TmdbAttribution() {
  return <span className={styles.attribution}>{TMDB_ATTRIBUTION_FRONTEND}</span>;
}
