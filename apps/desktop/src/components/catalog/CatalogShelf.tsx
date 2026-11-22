/* Display-only TMDB catalogue. Entries are not in the local library, so cards
 * carry no detail link, add, download or playback action. */

import { Link } from "react-router-dom";

import { TMDB_ATTRIBUTION_FRONTEND } from "@/components/cards/TmdbAttribution";
import { apiBase } from "@/services/errors";
import type { CatalogKind, CatalogScreen } from "@/services/types";
import styles from "./catalog.module.css";

export function CatalogShelf({
  kind,
  screen,
}: {
  kind: CatalogKind;
  screen: CatalogScreen | null;
}) {
  if (!screen) {
    return (
      <p className={styles.notice} role="status">
        Loading catalogue…
      </p>
    );
  }

  if (!screen.available) {
    return (
      <p className={styles.notice} role="status">
        Add a TMDB token in{" "}
        <Link to="/settings" className={styles.noticeLink} data-focusable="true">
          Settings
        </Link>{" "}
        to use Discover.
      </p>
    );
  }

  if (screen.entries.length === 0) {
    return (
      <>
        <p className={styles.notice} role="status">
          No catalogue titles available right now.
        </p>
        <p className={styles.attribution}>{TMDB_ATTRIBUTION_FRONTEND}</p>
      </>
    );
  }

  return (
    <section className={styles.section} aria-label={`${kind} catalogue`}>
      <div className={styles.grid}>
        {screen.entries.map((entry) => {
          const poster = entry.artwork[0]?.url;
          const details = [
            entry.year === null ? null : String(entry.year),
            entry.rating === null ? null : `${entry.rating.toFixed(1)} ★`,
          ]
            .filter((part): part is string => part !== null)
            .join(" · ");
          return (
            <article key={entry.id} className={styles.card}>
              <div className={styles.artWrap}>
                {poster ? (
                  <img
                    className={styles.art}
                    src={`${apiBase}${poster}`}
                    alt={entry.artwork[0]?.alt ?? ""}
                    loading="lazy"
                    decoding="async"
                    draggable={false}
                    onError={(event) => {
                      // A cached poster can be evicted; never break the page.
                      event.currentTarget.hidden = true;
                    }}
                  />
                ) : null}
              </div>
              <div className={styles.meta}>
                <span className={styles.title}>{entry.title}</span>
                {details ? <span className={styles.sub}>{details}</span> : null}
              </div>
            </article>
          );
        })}
      </div>
      <p className={styles.attribution}>{TMDB_ATTRIBUTION_FRONTEND}</p>
    </section>
  );
}
