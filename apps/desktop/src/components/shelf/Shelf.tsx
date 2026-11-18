import type { ReactNode } from "react";

import styles from "./shelf.module.css";

export function Shelf({
  title,
  caption,
  children,
  empty,
}: {
  title: string;
  caption?: string | undefined;
  children: ReactNode;
  empty?: string | undefined;
}) {
  const hasChildren = Array.isArray(children)
    ? children.length > 0
    : Boolean(children);

  return (
    <section className={styles.shelf}>
      <div className={styles.head}>
        <h2 className={styles.title}>{title}</h2>
        {caption ? <p className={styles.caption}>{caption}</p> : null}
      </div>
      {hasChildren ? (
        <div className={styles.track}>{children}</div>
      ) : empty ? (
        <p className={styles.empty}>{empty}</p>
      ) : null}
    </section>
  );
}
