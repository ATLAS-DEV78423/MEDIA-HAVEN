import type { EpisodeSummary } from "@/types/media";
import { stillOf, formatRuntime, progressRatio } from "@/types/media";
import { Artwork } from "@/components/media/Artwork";
import { StatusPill } from "./ProgressBar";
import styles from "./cards.module.css";

export interface EpisodeCardProps {
  readonly episode: EpisodeSummary;
  readonly onOpen: (episode: EpisodeSummary) => void;
  readonly showSeries?: boolean;
}

export function EpisodeCard({
  episode,
  onOpen,
  showSeries = true,
}: EpisodeCardProps) {
  const runtime = formatRuntime(episode.runtimeMinutes);
  const missing = episode.availability.state === "missing-file";
  const watched = episode.progress?.completed === true;

  return (
    <button
      type="button"
      className={`${styles.card} ${styles.wide}`}
      data-shelf-item
      onClick={() => onOpen(episode)}
      aria-label={`${episode.seriesTitle}, season ${episode.seasonNumber} episode ${episode.episodeNumber}: ${episode.title}`}
    >
      <Artwork
        artwork={stillOf(episode)}
        shape="landscape"
        alt={`${episode.title} still`}
      />
      {episode.progress && episode.progress.positionSeconds > 0 ? (
        <div className={styles.progressOverlay}>
          <div
            className={styles.progressOverlayFill}
            style={{ width: `${progressRatio(episode.progress) * 100}%` }}
          />
        </div>
      ) : null}
      <div className={styles.body}>
        <div className={styles.title}>
          {episode.episodeNumber}. {episode.title}
        </div>
        <div className={styles.meta}>
          {showSeries
            ? `${episode.seriesTitle} · S${episode.seasonNumber}E${episode.episodeNumber}`
            : `S${episode.seasonNumber}E${episode.episodeNumber}`}
          {runtime ? ` · ${runtime}` : ""}
        </div>
        {missing ? <StatusPill tone="missing">Missing</StatusPill> : null}
        {watched ? <StatusPill tone="available">Watched</StatusPill> : null}
      </div>
    </button>
  );
}
