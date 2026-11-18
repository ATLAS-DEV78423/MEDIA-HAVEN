/* Map backend DTOs onto the view model. No invented ratings, match scores or
 * taglines: fields the backend does not know about stay absent and the UI hides
 * them. Playback progress becomes a 0..1 ratio for the progress bar. */

import { artFor, runtimeLabel, type Episode, type Media, type Season } from "@/data/catalog";

import type {
  EpisodeSummary,
  MediaDetail,
  MediaSummary,
  PlaybackProgress,
  SeriesDetail,
} from "./types";

const ratio = (progress: PlaybackProgress | null | undefined): number | undefined => {
  if (!progress || progress.durationSeconds <= 0) return undefined;
  return Math.min(1, Math.max(0, progress.positionSeconds / progress.durationSeconds));
};

const synopsisOf = (value: string | null): string =>
  value?.trim() ? value.trim() : "No synopsis has been recorded for this title yet.";

export function toMedia(summary: MediaSummary): Media {
  const minutes = summary.runtimeMinutes;
  const progress = ratio(summary.progress);
  return {
    id: summary.id,
    title: summary.title,
    year: summary.year,
    kind: summary.kind === "series" ? "series" : "movie",
    genres: summary.genres,
    runtime: minutes ? runtimeLabel(minutes) : summary.kind === "series" ? "Series" : "",
    minutes,
    synopsis: synopsisOf(summary.synopsis),
    about: synopsisOf(summary.synopsis),
    art: artFor(summary.id, summary.title),
    ...(progress !== undefined ? { progress } : {}),
  };
}

export function seriesToMedia(series: SeriesDetail): Media {
  return {
    id: series.id,
    title: series.title,
    year: series.year,
    kind: "series",
    genres: series.genres,
    runtime:
      series.seasons.length === 1 ? "1 season" : `${series.seasons.length} seasons`,
    minutes: null,
    synopsis: synopsisOf(series.synopsis),
    about: synopsisOf(series.synopsis),
    art: artFor(series.id, series.title),
    seasons: series.seasons.map((season) => ({
      number: season.number,
      title: season.title,
      episodeCount: season.episodeCount,
      episodes: [],
    })),
  };
}

export function detailToMedia(detail: MediaDetail): Media {
  if (detail.kind === "series") return seriesToMedia(detail);
  const base = toMedia({
    ...detail,
    kind: "movie",
    artwork: detail.artwork,
    progress: detail.progress,
  });
  return {
    ...base,
    ...(detail.director ? { director: detail.director } : {}),
    ...(detail.cast.length > 0 ? { cast: detail.cast } : {}),
  };
}

export function toEpisode(episode: EpisodeSummary): Episode {
  const progress = ratio(episode.progress);
  return {
    id: episode.id,
    season: episode.seasonNumber,
    number: episode.episodeNumber,
    title: episode.title,
    synopsis: "",
    minutes: episode.runtimeMinutes ?? 0,
    ...(progress !== undefined ? { progress } : {}),
  };
}

export function seasonsWithEpisodes(
  seasons: Season[],
  seasonNumber: number,
  episodes: EpisodeSummary[],
): Season[] {
  return seasons.map((season) =>
    season.number === seasonNumber
      ? { ...season, episodes: episodes.map(toEpisode) }
      : season,
  );
}
