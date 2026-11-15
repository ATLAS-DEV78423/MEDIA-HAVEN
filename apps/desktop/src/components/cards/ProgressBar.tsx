import type { ReactNode } from "react";
import styles from "./cards.module.css";

interface ProgressBarProps {
  /** 0..1 */
  readonly value: number;
  readonly label?: string;
}

export function ProgressBar({ value, label }: ProgressBarProps) {
  const clamped = Math.min(1, Math.max(0, value));
  return (
    <div
      className={styles.progress}
      role="progressbar"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(clamped * 100)}
      aria-label={label ?? "Playback progress"}
    >
      <div className={styles.progressFill} style={{ width: `${clamped * 100}%` }} />
    </div>
  );
}

export type StatusTone = "neutral" | "available" | "missing" | "offline";

interface StatusPillProps {
  readonly tone: StatusTone;
  readonly children: ReactNode;
}

const TONE_CLASS: Record<StatusTone, string | undefined> = {
  neutral: undefined,
  available: styles.pillAvailable,
  missing: styles.pillMissing,
  offline: styles.pillOffline,
};

export function StatusPill({ tone, children }: StatusPillProps) {
  return (
    <span className={`${styles.pill} ${TONE_CLASS[tone] ?? ""}`}>
      <span className={styles.pillDot} aria-hidden="true" />
      {children}
    </span>
  );
}

export function SkeletonCard() {
  return (
    <div className={styles.skeleton} aria-hidden="true">
      <div className={styles.skeletonArt} />
      <div className={styles.skeletonLine} />
      <div className={styles.skeletonLineShort} />
    </div>
  );
}
