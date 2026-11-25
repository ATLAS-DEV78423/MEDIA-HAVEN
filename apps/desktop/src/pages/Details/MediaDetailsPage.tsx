import { useCallback, useEffect, useMemo, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";

import { ImdbRating } from "@/components/cards/ImdbRating";
import { MediaCard } from "@/components/cards/MediaCard";
import { Shelf } from "@/components/shelf/Shelf";
import { ActionButton, Chip, ProgressBar, buttonClass } from "@/components/ui/Controls";
import { Icon } from "@/components/ui/Icon";
import { EmptyState } from "@/components/ui/Surfaces";
import { artFor, type Media } from "@/data/catalog";
import { api, describeError, type ShortlistItem } from "@/services/api";
import {
  detailToMedia,
  seasonsWithEpisodes,
  seriesToMedia,
  toMedia,
} from "@/services/adapt";
import type { EpisodeSummary, MediaDetail } from "@/services/types";
import { useAppState } from "@/state/AppState";
import styles from "./details.module.css";

/* Mirrors the Rust trust boundary for R-01 (ADR-0021). The backend repeats
 * this validation; the UI only avoids sending a source it already knows is
 * invalid. Rejections never echo the input, so no path reaches the screen. */
// Real magnets with several tracker tiers are ~2.2k characters; the host and
// domain validators use the same 8192 cap. Keep in step with those boundaries.
const MAX_SOURCE_LEN = 8192;
const MAGNET_PREFIX = "magnet:?xt=urn:btih:";

function hasControl(value: string): boolean {
  return Array.from(value).some((ch) => {
    const code = ch.codePointAt(0) ?? 0;
    return code <= 0x1f || code === 0x7f;
  });
}

function shortlistSize(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "Unknown size";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const power = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / 1024 ** power).toFixed(power === 0 ? 0 : 1)} ${units[power]}`;
}

function sourceProblem(raw: string): string | null {
  const source = raw.trim();
  if (!source) return "Enter a magnet link or a .torrent file name.";
  if (source.length > MAX_SOURCE_LEN || hasControl(source)) {
    return "That source is too long or contains invalid characters.";
  }
  const lower = source.toLowerCase();
  if (lower.startsWith(MAGNET_PREFIX)) return null;
  if (
    lower.endsWith(".torrent") &&
    source.length > ".torrent".length &&
    !/[\\/:]/.test(source) &&
    !source.includes("..") &&
    !source.startsWith(".")
  ) {
    return null;
  }
  return "Use a magnet link or a bare .torrent file name — no paths.";
}

export function MediaDetailsPage() {
  const { mediaId } = useParams();
  const navigate = useNavigate();
  const { data, isListed, toggleList } = useAppState();

  const [detail, setDetail] = useState<MediaDetail | null>(null);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [seasonNumber, setSeasonNumber] = useState<number | null>(null);
  const [episodes, setEpisodes] = useState<EpisodeSummary[]>([]);
  const [episodeError, setEpisodeError] = useState<string | null>(null);
  const [source, setSource] = useState("");
  const [sourceError, setSourceError] = useState<string | null>(null);
  const [sourceSaved, setSourceSaved] = useState(false);
  const [savingSource, setSavingSource] = useState(false);
  const [shortlist, setShortlist] = useState<ShortlistItem[]>([]);
  const [shortlistState, setShortlistState] = useState<
    "idle" | "loading" | "ready" | "setup_required" | "error"
  >("idle");
  const [shortlistError, setShortlistError] = useState<string | null>(null);
  const [confirmingMagnet, setConfirmingMagnet] = useState<string | null>(null);
  const [confirmedMagnet, setConfirmedMagnet] = useState<string | null>(null);

  const loadDetail = useCallback(async () => {
    if (!mediaId) return;
    setDetail(null);
    setDetailError(null);
    setEpisodes([]);
    setSeasonNumber(null);
    setSource("");
    setSourceError(null);
    setSourceSaved(false);
    setShortlist([]);
    setShortlistState("idle");
    setShortlistError(null);
    setConfirmingMagnet(null);
    setConfirmedMagnet(null);
    try {
      setDetail(await api.detail(mediaId));
    } catch (error) {
      setDetailError(describeError(error));
    }
  }, [mediaId]);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void loadDetail();
    }, 0);
    return () => window.clearTimeout(timer);
  }, [loadDetail]);

  const saveSource = useCallback(async () => {
    if (!mediaId) return;
    const problem = sourceProblem(source);
    if (problem) {
      setSourceError(problem);
      setSourceSaved(false);
      return;
    }
    setSourceError(null);
    setSavingSource(true);
    try {
      await api.sourceSet(mediaId, source.trim());
      setConfirmedMagnet(source.trim());
      setSourceSaved(true);
    } catch (error) {
      setSourceError(describeError(error));
      setSourceSaved(false);
      setConfirmedMagnet(null);
    } finally {
      setSavingSource(false);
    }
  }, [mediaId, source]);

  const findSources = useCallback(async () => {
    if (!detail) return;
    setShortlist([]);
    setShortlistError(null);
    setShortlistState("loading");
    try {
      const result = await api.sourceSearch(detail.title, detail.year);
      if (result.setup_required === true) {
        setShortlistState("setup_required");
        return;
      }
      setShortlist(result.items ?? []);
      setShortlistState("ready");
    } catch (error) {
      setShortlistError(describeError(error));
      setShortlistState("error");
    }
  }, [detail]);

  const confirmSource = useCallback(
    async (magnet: string) => {
      if (!mediaId) return;
      setSourceError(null);
      setConfirmingMagnet(magnet);
      try {
        await api.sourceSet(mediaId, magnet);
        setConfirmedMagnet(magnet);
        setSourceSaved(true);
        setDetail(await api.detail(mediaId));
      } catch (error) {
        setSourceError(describeError(error));
        setSourceSaved(false);
        setConfirmedMagnet(null);
      } finally {
        setConfirmingMagnet(null);
      }
    },
    [mediaId],
  );

  const activeSeason =
    detail?.kind === "series"
      ? seasonNumber ?? detail.seasons[0]?.number ?? null
      : null;

  const seriesId = detail?.kind === "series" ? detail.id : null;

  const loadEpisodes = useCallback(
    async (id: string, season: number) => {
      setEpisodeError(null);
      try {
        setEpisodes(await api.episodes(id, season));
      } catch (error) {
        setEpisodes([]);
        setEpisodeError(describeError(error));
      }
    },
    [],
  );

  useEffect(() => {
    if (!seriesId || activeSeason === null) return;
    const timer = window.setTimeout(() => {
      void loadEpisodes(seriesId, activeSeason);
    }, 0);
    return () => window.clearTimeout(timer);
  }, [seriesId, activeSeason, loadEpisodes]);

  const media = useMemo<Media | null>(() => {
    if (!detail) return null;
    const base = detailToMedia(detail);
    if (base.kind !== "series") return base;
    return { ...base, seasons: seasonsWithEpisodes(base.seasons ?? [], activeSeason ?? 0, episodes) };
  }, [detail, activeSeason, episodes]);

  const related = useMemo<Media[]>(() => {
    if (!detail || !data) return [];
    const pool: Media[] = [
      ...data.movies.map(toMedia),
      ...data.shows
        .filter((series) => series.id !== detail.id)
        .map(seriesToMedia),
    ];
    return pool
      .filter((candidate) => candidate.genres.some((genre) => detail.genres.includes(genre)))
      .slice(0, 5);
  }, [detail, data]);

  if (!media) {
    const art = mediaId ? artFor(mediaId, "") : null;
    return (
      <div className={`page ${styles.page}`}>
        {art ? (
          <div className={styles.backdrop}>
            <img className={styles.backdropArt} src={art.hero} alt="" decoding="async" />
            <div className={styles.backdropScrim} aria-hidden="true" />
            <button
              type="button"
              className={styles.back}
              onClick={() => navigate(-1)}
              aria-label="Go back"
              data-focusable="true"
            >
              <Icon name="chevronLeft" />
            </button>
          </div>
        ) : null}
        <EmptyState
          title={detailError ? "This title could not be loaded" : "Opening the title…"}
          body={
            detailError ??
            "Waiting for the local library service to return the record."
          }
          {...(detailError
            ? {
                action: (
                  <ActionButton variant="solid" onClick={() => void loadDetail()}>
                    Retry
                  </ActionButton>
                ),
              }
            : {})}
        />
      </div>
    );
  }

  const isSeries = media.kind === "series";
  const listed = isListed(media.id);
  const resumeEpisode = episodes.find((episode) => episode.progress && !episode.progress.completed);
  const playEpisode = resumeEpisode ?? episodes[0];
  const playHref = playEpisode
    ? `/player/${playEpisode.id}`
    : isSeries
      ? null
      : `/player/${media.id}`;
  const facts = [
    media.year ? { label: "Year", value: String(media.year) } : null,
    media.minutes ? { label: "Runtime", value: media.runtime } : null,
    detail?.kind === "movie" && detail.director
      ? { label: "Director", value: detail.director }
      : null,
    isSeries && detail?.kind === "series"
      ? { label: "Seasons", value: String(detail.seasons.length) }
      : null,
    isSeries && detail?.kind === "series"
      ? {
          label: "Episodes",
          value: String(
            detail.seasons.reduce((total, season) => total + season.episodeCount, 0),
          ),
        }
      : null,
    media.genres.length > 0
      ? { label: "Genres", value: media.genres.join(", ") }
      : null,
  ].filter((fact): fact is { label: string; value: string } => Boolean(fact));

  const availability = detail?.kind === "movie" ? detail.availability : null;

  return (
    <div className={`page ${styles.page}`}>
      <div className={styles.backdrop}>
        <img
          className={styles.backdropArt}
          src={media.art.hero}
          alt=""
          decoding="async"
        />
        <div className={styles.backdropScrim} aria-hidden="true" />
        <button
          type="button"
          className={styles.back}
          onClick={() => navigate(-1)}
          aria-label="Go back"
          data-focusable="true"
        >
          <Icon name="chevronLeft" />
        </button>
      </div>

      <article className={styles.sheet}>
        <div className={styles.head}>
          <div className={styles.titleBlock}>
            <h1 className={styles.title}>{media.title}</h1>
            <div className={styles.meta}>
              {media.year ? <span>{media.year}</span> : null}
              {media.runtime ? <span>{media.runtime}</span> : null}
              {media.genres.length > 0 ? <span>{media.genres.join(" · ")}</span> : null}
              {availability && availability.state !== "available" ? (
                <span className={styles.offline}>
                  {availability.state === "missing-file" ? "File missing" : "Offline"}
                </span>
              ) : null}
              {media.progress !== undefined ? (
                <span>{Math.round(media.progress * 100)}% watched</span>
              ) : null}
            </div>
          </div>

          <div className={styles.actions}>
            {playHref ? (
              <Link to={playHref} className={buttonClass("solid")} data-focusable="true">
                <Icon name="play" />
                <span>{resumeEpisode ? "Resume" : "Play"}</span>
                {resumeEpisode ? (
                  <span className={styles.actionNote}>
                    S{resumeEpisode.seasonNumber} E{resumeEpisode.episodeNumber}
                  </span>
                ) : null}
              </Link>
            ) : (
              <span className={buttonClass("glass")} aria-disabled="true">
                <Icon name="play" />
                <span>{isSeries ? "No episodes indexed" : "Play"}</span>
              </span>
            )}
            <ActionButton
              icon={listed ? "check" : "plus"}
              variant="glass"
              onClick={() => void toggleList(media.id)}
              aria-pressed={listed}
            >
              {listed ? "In My List" : "Add to My List"}
            </ActionButton>
          </div>
        </div>

        <div className={styles.body}>
          <p className={styles.synopsis}>{media.about}</p>

          <dl className={styles.facts}>
            {facts.map((fact) => (
              <div key={fact.label}>
                <dt>{fact.label}</dt>
                <dd>{fact.value}</dd>
              </div>
            ))}
          </dl>

          <ImdbRating
            imdbRating={detail?.imdbRating}
            voteCount={detail?.voteCount}
            ageRating={detail?.ageRating}
          />
        </div>

        <section className={styles.sourceSection} aria-label="Find sources">
          <h2 className={styles.sectionTitle}>Find sources</h2>
          <p className={styles.sourceHint}>
            Search the operator-configured indexer for this title. Nothing is
            downloaded until you confirm a result; the manual field below stays
            available as a fallback.
          </p>
          <div className={styles.sourceRow}>
            <ActionButton
              variant="glass"
              onClick={() => void findSources()}
              disabled={!detail || shortlistState === "loading"}
            >
              {shortlistState === "loading" ? "Searching…" : "Find sources"}
            </ActionButton>
          </div>
          {shortlistState === "loading" ? (
            <p className={styles.sourceHint} role="status">
              Searching the configured indexer…
            </p>
          ) : null}
          {shortlistState === "setup_required" ? (
            <p className={styles.sourceHint} role="status">
              Source search is not set up. Ask the operator to configure the
              Torznab endpoint and key. No request was made.
            </p>
          ) : null}
          {shortlistState === "error" ? (
            <p className={styles.sourceError} role="alert">
              Source search is unavailable. {shortlistError}
            </p>
          ) : null}
          {shortlistState === "ready" && shortlist.length === 0 ? (
            <p className={styles.sourceHint} role="status">
              No candidates were returned.
            </p>
          ) : null}
          {shortlistState === "ready" && shortlist.length > 0 ? (
            <ol className={styles.shortlist} aria-label="Source candidates">
              {shortlist.map((candidate, index) => (
                <li
                  key={`${candidate.source}-${index}`}
                  className={styles.shortlistRow}
                >
                  <span className={styles.candidateCopy}>
                    <span className={styles.candidateTitle}>{candidate.title}</span>
                    <span className={styles.candidateMeta}>
                      {shortlistSize(candidate.sizeBytes)} · {candidate.seeders} seeders ·{" "}
                      {candidate.resolution} · {candidate.source}
                    </span>
                    {confirmedMagnet === candidate.magnet ? (
                      <span className={styles.sourceSaved}>
                        Download started — see Downloads
                      </span>
                    ) : null}
                  </span>
                  <ActionButton
                    variant="glass"
                    onClick={() => void confirmSource(candidate.magnet)}
                    disabled={confirmingMagnet !== null || confirmedMagnet === candidate.magnet}
                  >
                    {confirmedMagnet === candidate.magnet ? "Confirmed" : "Confirm"}
                  </ActionButton>
                </li>
              ))}
            </ol>
          ) : null}
        </section>

        <section className={styles.sourceSection} aria-label="Acquisition source">
          <label className={styles.sourceLabel} htmlFor="item-source">
            Source
          </label>
          <p className={styles.sourceHint}>
            Attach a magnet link or a bare <code>.torrent</code> file name you
            already hold. Nothing is discovered or downloaded by this field.
          </p>
          <div className={styles.sourceRow}>
            <input
              id="item-source"
              className={styles.sourceInput}
              type="text"
              value={source}
              onChange={(event) => {
                setSource(event.target.value);
                setSourceSaved(false);
                setConfirmedMagnet(null);
              }}
              placeholder="magnet:?xt=urn:btih:… or episode.torrent"
              aria-invalid={sourceError !== null}
              disabled={savingSource}
              data-focusable="true"
            />
            <ActionButton
              variant="glass"
              onClick={() => void saveSource()}
              disabled={savingSource}
            >
              Save source
            </ActionButton>
          </div>
          {sourceError ? (
            <p className={styles.sourceError} role="alert">
              {sourceError}
            </p>
          ) : null}
          {sourceSaved ? (
            <p className={styles.sourceSaved} role="status">
              Download started. <Link to="/downloads">Track progress in Downloads</Link>.
            </p>
          ) : null}
        </section>

        {detail?.kind === "movie" && detail.cast.length > 0 ? (
          <p className={styles.synopsis} style={{ marginTop: "var(--s-6)" }}>
            <strong>Starring</strong> {detail.cast.join(", ")}
          </p>
        ) : null}

        {isSeries && detail?.kind === "series" && detail.seasons.length > 0 ? (
          <section className={styles.episodes} aria-label="Episodes">
            <div className={styles.seasonBar}>
              <h2 className={styles.sectionTitle}>Episodes</h2>
              <div className={styles.seasons} role="group" aria-label="Season">
                {detail.seasons.map((season) => (
                  <Chip
                    key={season.number}
                    active={season.number === activeSeason}
                    onClick={() => setSeasonNumber(season.number)}
                  >
                    Season {season.number}
                  </Chip>
                ))}
              </div>
            </div>

            {episodeError ? (
              <EmptyState
                title="Episodes could not be loaded"
                body={episodeError}
                action={
                  <ActionButton
                    variant="glass"
                    onClick={() =>
                      activeSeason !== null && void loadEpisodes(media.id, activeSeason)
                    }
                  >
                    Retry
                  </ActionButton>
                }
              />
            ) : (
              <ul className={styles.episodeList}>
                {episodes.map((episode) => (
                  <li key={episode.id}>
                    <Link
                      to={`/player/${episode.id}`}
                      className={styles.episode}
                      data-focusable="true"
                    >
                      <span className={styles.episodeThumb}>
                        <img src={media.art.still} alt="" loading="lazy" />
                        <span className={styles.episodePlay} aria-hidden="true">
                          <Icon name="play" />
                        </span>
                      </span>
                      <span className={styles.episodeCopy}>
                        <span className={styles.episodeTop}>
                          <span className={styles.episodeNumber}>
                            {episode.episodeNumber}
                          </span>
                          <span className={styles.episodeTitle}>{episode.title}</span>
                          {episode.runtimeMinutes ? (
                            <span className={styles.episodeTime}>
                              {episode.runtimeMinutes} min
                            </span>
                          ) : null}
                        </span>
                        {episode.progress && episode.progress.durationSeconds > 0 ? (
                          <span className={styles.episodeProgress}>
                            <ProgressBar
                              value={
                                episode.progress.positionSeconds /
                                episode.progress.durationSeconds
                              }
                            />
                          </span>
                        ) : null}
                      </span>
                    </Link>
                  </li>
                ))}
              </ul>
            )}
          </section>
        ) : null}
      </article>

      {related.length > 0 ? (
        <div className={styles.related}>
          <Shelf title="More Like This">
            {related.map((item) => (
              <MediaCard key={item.id} media={item} />
            ))}
          </Shelf>
        </div>
      ) : null}
    </div>
  );
}
