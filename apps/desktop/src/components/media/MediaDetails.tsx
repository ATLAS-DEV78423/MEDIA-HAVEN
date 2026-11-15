import { useEffect, useRef, useState } from "react";
import type {
  EpisodeSummary,
  MediaDetail,
  MediaSummary,
  SeriesDetail,
} from "@/types/media";
import { backdropOf, formatRuntime } from "@/types/media";
import { getCommands } from "@/services/commands";
import { useCommand } from "@/hooks/useCommand";
import { useOverlay } from "@/components/navigation/FocusManager";
import { Artwork } from "@/components/media/Artwork";
import { StatusPill } from "@/components/cards/ProgressBar";
import { ErrorState } from "@/components/feedback/States";
import styles from "./details.module.css";

export interface MediaDetailsProps {
  readonly mediaId: string;
  readonly onClose: () => void;
  readonly onPlay: (media: MediaSummary, startPositionSeconds: number) => void;
  readonly onQueue: (media: MediaSummary) => void;
  readonly onWatchlist: (media: MediaSummary) => void;
  readonly inWatchlist: boolean;
}

function toSummary(detail: MediaDetail): MediaSummary {
  if (detail.kind === "movie") return detail;
  return {
    id: detail.id,
    kind: "series",
    title: detail.title,
    year: detail.year,
    synopsis: detail.synopsis,
    runtimeMinutes: null,
    genres: detail.genres,
    artwork: detail.artwork,
    availability: { state: "available" },
    progress: null,
  };
}

function CloseIcon() {
  return (
    <svg
      viewBox="0 0 24 24"
      width="22"
      height="22"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      aria-hidden="true"
      focusable="false"
    >
      <path d="m6 6 12 12M18 6 6 18" />
    </svg>
  );
}

function SeasonPicker({
  series,
  active,
  onSelect,
}: {
  readonly series: SeriesDetail;
  readonly active: number;
  readonly onSelect: (season: number) => void;
}) {
  return (
    <div className={styles.seasonRow} role="tablist" aria-label="Seasons">
      {series.seasons.map((season) => (
        <button
          key={season.number}
          type="button"
          role="tab"
          aria-selected={season.number === active}
          className={`${styles.seasonButton} ${
            season.number === active ? styles.seasonButtonActive : ""
          }`}
          onClick={() => onSelect(season.number)}
        >
          Season {season.number}
        </button>
      ))}
    </div>
  );
}

function EpisodeList({
  episodes,
  onPlay,
}: {
  readonly episodes: readonly EpisodeSummary[];
  readonly onPlay: (episode: EpisodeSummary) => void;
}) {
  return (
    <div className={styles.episodeList}>
      {episodes.map((episode) => (
        <button
          key={episode.id}
          type="button"
          className={styles.episodeRow}
          onClick={() => onPlay(episode)}
        >
          <span className={styles.episodeNumber} aria-hidden="true">
            {episode.episodeNumber}
          </span>
          <span>
            <span className={styles.episodeTitle}>{episode.title}</span>
            <span className={styles.episodeMeta}>
              {episode.availability.state === "missing-file"
                ? "File missing"
                : episode.progress?.completed
                  ? "Watched"
                  : "Not watched"}
            </span>
          </span>
          <span className={styles.episodeRuntime}>
            {formatRuntime(episode.runtimeMinutes) ?? "—"}
          </span>
        </button>
      ))}
    </div>
  );
}

export function MediaDetails({
  mediaId,
  onClose,
  onPlay,
  onQueue,
  onWatchlist,
  inWatchlist,
}: MediaDetailsProps) {
  const [season, setSeason] = useState(1);

  useOverlay(onClose);

  const detail = useCommand<MediaDetail>(
    async () => (await getCommands()).library_detail(mediaId),
    [mediaId],
  );

  const isSeries = detail.status === "ready" && detail.data.kind === "series";

  const episodes = useCommand<readonly EpisodeSummary[]>(
    async () =>
      isSeries
        ? (await getCommands()).library_episodes(mediaId, season)
        : [],
    [mediaId, season, isSeries],
  );

  useEffect(() => {
    if (detail.status !== "ready") return;
    const data = detail.data;
    if (data.kind !== "series") return;
    const first = data.seasons[0]?.number ?? 1;
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setSeason((current) =>
      data.seasons.some((s) => s.number === current) ? current : first,
    );
  }, [detail]);

  const closeRef = useRef(onClose);
  useEffect(() => {
    closeRef.current = onClose;
  });

  return (
    <div
      className={styles.scrim}
      role="dialog"
      aria-modal="true"
      aria-label="Media details"
      onClick={(event) => {
        if (event.target === event.currentTarget) closeRef.current();
      }}
    >
      <div className={styles.sheet}>
        <button
          type="button"
          className={styles.close}
          onClick={onClose}
          aria-label="Close details"
        >
          <CloseIcon />
        </button>

        {detail.status === "loading" ? (
          <div className={styles.body}>
            <div className={styles.title}>Loading…</div>
          </div>
        ) : null}

        {detail.status === "error" ? (
          <ErrorState error={detail.error} onRetry={detail.reload} />
        ) : null}

        {detail.status === "ready" ? (
          <>
            <div className={styles.backdrop}>
              <Artwork
                artwork={backdropOf(detail.data)}
                shape="landscape"
                alt=""
                className={styles.backdropImage}
              />
              <div className={styles.backdropScrim} />
            </div>

            <div className={styles.body}>
              <h2 className={styles.title}>{detail.data.title}</h2>
              <div className={styles.meta}>
                {detail.data.year ? <span>{detail.data.year}</span> : null}
                {detail.data.kind === "movie" &&
                formatRuntime(detail.data.runtimeMinutes) ? (
                  <span>{formatRuntime(detail.data.runtimeMinutes)}</span>
                ) : null}
                {detail.data.genres.length > 0 ? (
                  <span>{detail.data.genres.join(", ")}</span>
                ) : null}
                {detail.data.kind === "movie" && detail.data.director ? (
                  <span>Directed by {detail.data.director}</span>
                ) : null}
              </div>

              {detail.data.synopsis ? (
                <p className={styles.synopsis}>{detail.data.synopsis}</p>
              ) : null}

              {detail.data.kind === "movie" &&
              detail.data.availability.state !== "available" ? (
                <StatusPill tone="missing">
                  {detail.data.availability.state === "missing-file"
                    ? "File missing"
                    : "Offline"}
                </StatusPill>
              ) : null}

              <div className={styles.actions}>
                <button
                  type="button"
                  className={`${styles.button} ${styles.primary}`}
                  onClick={() => {
                    if (detail.data.kind === "movie") {
                      onPlay(detail.data, detail.data.progress?.positionSeconds ?? 0);
                      return;
                    }
                    const firstSeason = detail.data.seasons[0]?.number ?? 1;
                    void (async () => {
                      const episodes = await (await getCommands()).library_episodes(
                        detail.data.id,
                        firstSeason,
                      );
                      const first = episodes[0];
                      if (first) {
                        onPlay(
                          {
                            id: first.id,
                            kind: "episode",
                            title: first.title,
                            year: null,
                            synopsis: null,
                            runtimeMinutes: first.runtimeMinutes,
                            genres: [],
                            artwork: first.artwork,
                            availability: first.availability,
                            progress: first.progress,
                          },
                          first.progress?.positionSeconds ?? 0,
                        );
                      }
                    })();
                  }}
                  disabled={
                    detail.data.kind === "movie" &&
                    detail.data.availability.state !== "available"
                  }
                >
                  {detail.data.kind === "movie" &&
                  detail.data.progress?.positionSeconds
                    ? "Resume"
                    : "Play"}
                </button>
                <button
                  type="button"
                  className={`${styles.button} ${styles.secondary}`}
                  onClick={() => onQueue(toSummary(detail.data))}
                >
                  Add to Queue
                </button>
                <button
                  type="button"
                  className={`${styles.button} ${styles.secondary} ${
                    inWatchlist ? styles.secondaryOn : ""
                  }`}
                  onClick={() => onWatchlist(toSummary(detail.data))}
                >
                  {inWatchlist ? "In Watchlist" : "Add to Watchlist"}
                </button>
              </div>

              {detail.data.kind === "series" ? (
                <>
                  <SeasonPicker
                    series={detail.data}
                    active={season}
                    onSelect={setSeason}
                  />
                  {episodes.status === "loading" ? (
                    <div className={styles.episodeMeta}>Loading episodes…</div>
                  ) : null}
                  {episodes.status === "error" ? (
                    <ErrorState error={episodes.error} onRetry={episodes.reload} />
                  ) : null}
                  {episodes.status === "ready" ? (
                    <EpisodeList
                      episodes={episodes.data}
                      onPlay={(episode) =>
                        onPlay(
                          {
                            id: episode.id,
                            kind: "episode",
                            title: episode.title,
                            year: null,
                            synopsis: null,
                            runtimeMinutes: episode.runtimeMinutes,
                            genres: [],
                            artwork: episode.artwork,
                            availability: episode.availability,
                            progress: episode.progress,
                          },
                          episode.progress?.positionSeconds ?? 0,
                        )
                      }
                    />
                  ) : null}
                </>
              ) : null}
            </div>
          </>
        ) : null}
      </div>
    </div>
  );
}
