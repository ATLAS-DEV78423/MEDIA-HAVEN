import type { ReactNode } from "react";

import styles from "./surfaces.module.css";

export function PageHeader({
  title,
  description,
  meta,
  children,
}: {
  title: string;
  description?: string;
  meta?: string;
  children?: ReactNode;
}) {
  return (
    <header className={styles.header}>
      <h1 className={styles.title}>{title}</h1>
      {description ? <p className={styles.description}>{description}</p> : null}
      {meta ? <p className={styles.meta}>{meta}</p> : null}
      {children ? <div className={styles.headerActions}>{children}</div> : null}
    </header>
  );
}

export function Panel({
  children,
  tone = "glass",
  className = "",
}: {
  children: ReactNode;
  tone?: "glass" | "quiet";
  className?: string;
}) {
  return (
    <section
      className={`${tone === "glass" ? styles.panel : styles.panelQuiet} ${className}`}
    >
      {children}
    </section>
  );
}

export function EmptyState({
  title,
  body,
  action,
}: {
  title: string;
  body: string;
  action?: ReactNode;
}) {
  return (
    <div className={styles.empty}>
      <h2 className={styles.emptyTitle}>{title}</h2>
      <p className={styles.emptyBody}>{body}</p>
      {action ? <div className={styles.emptyAction}>{action}</div> : null}
    </div>
  );
}

export function FieldRow({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: ReactNode;
}) {
  return (
    <div className={styles.fieldRow}>
      <div className={styles.fieldCopy}>
        <span className={styles.fieldLabel}>{label}</span>
        {hint ? <span className={styles.fieldHint}>{hint}</span> : null}
      </div>
      <div className={styles.fieldControl}>{children}</div>
    </div>
  );
}
