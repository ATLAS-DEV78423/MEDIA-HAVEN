/* The view model the interface renders.
 *
 * Nothing here is fixture data any more: every Media object is adapted from a
 * `mediahaven-core` response in services/adapt.ts. Artwork is the bundled
 * local set until a metadata provider supplies posters (ADR-0011); the mapping
 * is deterministic so a title keeps the same still across refreshes.
 */

export type MediaKind = "movie" | "series";

export interface Episode {
  id: string;
  season: number;
  number: number;
  title: string;
  synopsis: string;
  minutes: number;
  progress?: number;
}

export interface Season {
  number: number;
  title: string;
  episodeCount: number;
  episodes: Episode[];
}

export interface Media {
  id: string;
  title: string;
  year: number | null;
  kind: MediaKind;
  genres: string[];
  runtime: string;
  minutes: number | null;
  synopsis: string;
  about: string;
  art: { hero: string; still: string };
  director?: string;
  cast?: string[];
  seasons?: Season[];
  /** 0..1 resume ratio from the backend, absent until started. */
  progress?: number;
}

interface ArtSet {
  hero: string;
  still: string;
}

const ART: Record<string, ArtSet> = {
  dune: { hero: "/artwork/dune-hero.jpg", still: "/artwork/dune-card.jpg" },
  br: { hero: "/artwork/br-hero.jpg", still: "/artwork/br-card.jpg" },
  arrival: { hero: "/artwork/arrival.jpg", still: "/artwork/arrival.jpg" },
  quiet: { hero: "/artwork/quiet-earth.jpg", still: "/artwork/quiet-earth.jpg" },
  after: { hero: "/artwork/after-hours.jpg", still: "/artwork/after-hours.jpg" },
  tomorrow: { hero: "/artwork/tomorrow-war.jpg", still: "/artwork/tomorrow-war.jpg" },
  starwars: { hero: "/artwork/starwars.jpg", still: "/artwork/starwars.jpg" },
};

const KEYWORDS: [RegExp, string][] = [
  [/dune/i, "dune"],
  [/blade runner|2049/i, "br"],
  [/arrival/i, "arrival"],
  [/quiet earth/i, "quiet"],
  [/after hours/i, "after"],
  [/tomorrow war/i, "tomorrow"],
  [/star wars/i, "starwars"],
];

const FALLBACK = ["dune", "arrival", "quiet", "br", "after", "tomorrow", "starwars"];

const hash = (value: string): number => {
  let total = 0;
  for (let index = 0; index < value.length; index += 1) {
    total = (total * 31 + value.charCodeAt(index)) % 100000;
  }
  return total;
};

export function artFor(id: string, title: string): ArtSet {
  const keyword = KEYWORDS.find(([pattern]) => pattern.test(title))?.[1];
  const key = keyword ?? FALLBACK[hash(id) % FALLBACK.length] ?? "dune";
  return ART[key] ?? { hero: "/artwork/dune-hero.jpg", still: "/artwork/dune-card.jpg" };
}

export function runtimeLabel(minutes: number | null | undefined): string {
  if (!minutes || minutes <= 0) return "";
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  if (hours === 0) return `${rest}m`;
  return rest === 0 ? `${hours}h` : `${hours}h ${rest}m`;
}
