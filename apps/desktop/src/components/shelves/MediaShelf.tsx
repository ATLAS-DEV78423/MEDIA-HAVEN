import type { ReactNode } from "react";
import styles from "./shelf.module.css";

export interface MediaShelfProps {
  readonly title: string;
  readonly count?: number;
  readonly children: ReactNode;
  /** Actions rendered at the right of the shelf header. */
  readonly action?: ReactNode;
}

/**
 * A horizontal shelf of media.
 *
 * The `data-shelf` attribute is what the focus manager uses to move between
 * shelves with the vertical arrow keys, and to preserve the column.
 */
export function MediaShelf({ title, count, children, action }: MediaShelfProps) {
  return (
    <section className={styles.shelf} aria-labelledby={undefined}>
      <div className={styles.header}>
        <h2 className={styles.title}>{title}</h2>
        {action ??
          (count !== undefined ? (
            <span className={styles.count}>{count}</span>
          ) : null)}
      </div>
      <div className={styles.track} data-shelf>
        {children}
      </div>
    </section>
  );
}
