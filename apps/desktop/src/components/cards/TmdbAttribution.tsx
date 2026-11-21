/* Legal credit for cached TMDB artwork. The class is the shared, token-only
 * attribution style from imdbRating.module.css, so this adds no colour/spacing
 * literal of its own (AGENTS.md / Development.md §7.3). */

import styles from "./imdbRating.module.css";

export const TMDB_ATTRIBUTION_FRONTEND =
  "This product uses the TMDB API but is not endorsed or certified by TMDB.";

export function TmdbAttribution() {
  return <span className={styles.attribution}>{TMDB_ATTRIBUTION_FRONTEND}</span>;
}
