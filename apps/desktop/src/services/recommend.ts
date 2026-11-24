/* Local-only recommendation for the "Because you watched" shelf.
 *
 * No network, no persisted profile: the seed is simply the title with the
 * most playback progress, and picks are the unwatched titles sharing the most
 * genres with it. Pure and deterministic so the same library always renders
 * the same shelf. */

import type { Media } from "@/data/catalog";

const byTitle = (a: Media, b: Media): number =>
  a.title < b.title ? -1 : a.title > b.title ? 1 : 0;

export interface Recommendation {
  seed: Media;
  picks: Media[];
}

export function recommendBecauseYouWatched(
  items: Media[],
  limit = 6,
): Recommendation | null {
  const watched = items.filter(
    (item) => item.progress !== undefined && item.progress > 0,
  );
  if (watched.length === 0) return null;

  const seed = [...watched].sort(
    (a, b) => (b.progress ?? 0) - (a.progress ?? 0) || byTitle(a, b),
  )[0];
  if (!seed) return null;
  const seedGenres = new Set(seed.genres);

  const picks = items
    .filter((item) => item.progress === undefined || item.progress === 0)
    .map((item) => ({
      media: item,
      overlap: item.genres.filter((genre) => seedGenres.has(genre)).length,
    }))
    .sort((a, b) => b.overlap - a.overlap || byTitle(a.media, b.media))
    .slice(0, limit)
    .map(({ media }) => media);

  if (picks.length === 0) return null;
  return { seed, picks };
}
