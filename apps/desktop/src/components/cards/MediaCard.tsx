import type { MediaSummary } from "@/types/media";
import { posterOf, backdropOf, formatRuntime, progressRatio } from "@/types/media";
import { Artwork } from "@/components/media/Artwork";
import { StatusPill } from "./ProgressBar";
import styles from "./cards.module.css";

export interface MediaCardProps {
  readonly media: MediaSummary;
  readonly onOpen: (media: MediaSummary) => void;
  /** Optional secondary action, revealed on the details surface. */
  readonly onPlay?: (media: MediaSummary) => void;
  readonly variant?: "poster" | "landscape";
  readonly showStatus?: boolean;
}

export function MediaCard({
  media,
  onOpen,
  variant = "poster",
  showStatus = true,
}: MediaCardProps) {
  const artwork = variant === "poster" ? posterOf(media) : backdropOf(media);
  const runtime = formatRuntime(media.runtimeMinutes);
  const missing = media.availability.state === "missing-file";
  const offline = media.availability.state === "offline";

  const metaParts = [media.year, runtime].filter(
    (part): part is number | string => part !== null,
  );

  return (
    <button
      type="button"
      className={`${styles.card} ${variant === "landscape" ? styles.wide : ""}`}
      data-shelf-item
      onClick={() => onOpen(media)}
      aria-label={`${media.title}${
        media.year ? `, ${media.year}` : ""
      }${missing ? ", file missing" : ""}${offline ? ", offline" : ""}`}
    >
      <Artwork
        artwork={artwork}
        shape={variant}
        alt={`${media.title}${variant === "poster" ? " poster" : " still"}`}
      />
      {media.progress && media.progress.positionSeconds > 0 ? (
        <div className={styles.progressOverlay}>
          <div
            className={styles.progressOverlayFill}
            style={{ width: `${progressRatio(media.progress) * 100}%` }}
          />
        </div>
      ) : null}
      <div className={styles.body}>
        <div className={styles.title}>{media.title}</div>
        <div className={styles.meta}>
          {missing ? (
            <span className={styles.missing}>File missing</span>
          ) : offline ? (
            <span className={styles.missing}>Offline</span>
          ) : (
            metaParts.join(" · ")
          )}
        </div>
        {showStatus && media.progress?.completed ? (
          <StatusPill tone="available">Watched</StatusPill>
        ) : null}
      </div>
    </button>
  );
}
