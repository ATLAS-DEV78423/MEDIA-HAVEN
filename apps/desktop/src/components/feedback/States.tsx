import type { ReactNode } from "react";
import type { AppError } from "@/types/media";
import styles from "./page.module.css";

export interface PageHeaderProps {
  readonly title: string;
  readonly subtitle?: string;
  readonly toolbar?: ReactNode;
}

export function PageHeader({ title, subtitle, toolbar }: PageHeaderProps) {
  return (
    <header className={styles.header}>
      <div>
        <h1 className={styles.heading}>{title}</h1>
        {subtitle ? <p className={styles.subheading}>{subtitle}</p> : null}
      </div>
      {toolbar ? <div className={styles.toolbar}>{toolbar}</div> : null}
    </header>
  );
}

interface EmptyStateProps {
  readonly title: string;
  readonly body: string;
  readonly action?: ReactNode;
  readonly glyph?: ReactNode;
}

export function EmptyState({
  title,
  body,
  action,
  glyph,
}: EmptyStateProps) {
  return (
    <div className={styles.state} role="status">
      {glyph ? <div className={styles.stateGlyph}>{glyph}</div> : null}
      <p className={styles.stateTitle}>{title}</p>
      <p className={styles.stateBody}>{body}</p>
      {action ? <div className={styles.stateAction}>{action}</div> : null}
    </div>
  );
}

interface ErrorStateProps {
  readonly error: AppError;
  readonly onRetry?: () => void;
}

export function ErrorState({ error, onRetry }: ErrorStateProps) {
  return (
    <div className={styles.state} role="alert">
      <svg
        className={styles.stateGlyph}
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinecap="round"
        aria-hidden="true"
        focusable="false"
      >
        <path d="M12 4 2.5 20h19z" />
        <path d="M12 10v4.5M12 17.2v.1" />
      </svg>
      <p className={styles.stateTitle}>Something went wrong</p>
      <p className={styles.stateBody}>{error.message}</p>
      {error.retryable && onRetry ? (
        <div className={styles.stateAction}>
          <button type="button" className={styles.bannerAction} onClick={onRetry}>
            Try again
          </button>
        </div>
      ) : null}
    </div>
  );
}

export interface BannerProps {
  readonly title: string;
  readonly body: string;
  readonly tone?: "neutral" | "warning";
  readonly action?: ReactNode;
}

export function Banner({ title, body, tone = "neutral", action }: BannerProps) {
  return (
    <div
      className={`${styles.banner} ${tone === "warning" ? styles.bannerWarning : ""}`}
      role="status"
    >
      <svg
        className={styles.bannerIcon}
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.6"
        strokeLinecap="round"
        aria-hidden="true"
        focusable="false"
      >
        <circle cx="12" cy="12" r="9" />
        <path d="M12 7.5v5.5M12 16v.1" />
      </svg>
      <div className={styles.bannerBody}>
        <span className={styles.bannerTitle}>{title}</span>
        <span>{body}</span>
        {action}
      </div>
    </div>
  );
}
