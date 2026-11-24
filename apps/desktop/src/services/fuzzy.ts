/* Local, dependency-free title tolerance for search suggestions.
 * Pure and deterministic: the same query and library always yield the same list.
 * No network, no paths, no persisted state. */

/** Minimum score to surface a suggestion. Tuned so one typo still matches
 *  ("interstelar" -> "Interstellar") while unrelated words are dropped. */
export const MIN_SUGGESTION_SCORE = 0.45;

export interface SuggestionCandidate {
  id: string;
  title: string;
  kind?: string;
}

export interface TitleSuggestion {
  id: string;
  title: string;
  score: number;
}

const CONTROL_OR_PUNCTUATION = /[\p{C}\p{P}\p{S}]/gu;

/** Lowercase, drop a leading article (the/a/an), strip punctuation and control
 *  characters, and collapse whitespace. Used for both display and matching. */
export function normalizeTitle(value: string): string {
  const cleaned = value
    .toLowerCase()
    .replace(CONTROL_OR_PUNCTUATION, " ")
    .replace(/\s+/g, " ")
    .trim();
  return cleaned.replace(/^(the|a|an)\s+/, "");
}

/** Edit distance on the raw strings (case-sensitive, no normalisation). */
export function levenshtein(a: string, b: string): number {
  if (a === b) return 0;
  if (a.length === 0) return b.length;
  if (b.length === 0) return a.length;
  let previous = Array.from({ length: b.length + 1 }, (_, index) => index);
  let current = new Array<number>(b.length + 1);
  for (let i = 1; i <= a.length; i += 1) {
    current[0] = i;
    for (let j = 1; j <= b.length; j += 1) {
      const cost = a[i - 1] === b[j - 1] ? 0 : 1;
      current[j] = Math.min(
        (previous[j] ?? 0) + 1,
        (current[j - 1] ?? 0) + 1,
        (previous[j - 1] ?? 0) + cost,
      );
    }
    [previous, current] = [current, previous];
  }
  return previous[b.length] ?? 0;
}

/** 0..1 title similarity after normalisation; 1 means an equivalent title. */
export function similarity(a: string, b: string): number {
  const left = normalizeTitle(a);
  const right = normalizeTitle(b);
  if (left === right) return 1;
  const longest = Math.max(left.length, right.length);
  if (longest === 0) return 0;
  return 1 - levenshtein(left, right) / longest;
}

const byText = (a: string, b: string): number => (a < b ? -1 : a > b ? 1 : 0);

function scoreTitle(needle: string, tokens: string[], title: string): number {
  const candidate = normalizeTitle(title);
  if (!candidate) return 0;
  if (candidate === needle) return 1;

  const edit = similarity(needle, candidate);
  const substring =
    candidate.length >= 2 && (candidate.includes(needle) || needle.includes(candidate));
  const words = candidate.split(" ");
  const matched = tokens.filter((token) =>
    words.some(
      (word) => word === token || word.startsWith(token) || similarity(word, token) >= 0.8,
    ),
  ).length;

  return Math.max(edit, substring ? 0.9 : 0, (0.8 * matched) / tokens.length);
}

/** Rank candidates for a query. Exact matches first, then prefix/substring
 *  matches, then token and edit-distance similarity; below-threshold entries
 *  are dropped. Order is score descending, title ascending, id ascending. */
export function suggestTitles(
  query: string,
  candidates: SuggestionCandidate[],
  limit = 5,
): TitleSuggestion[] {
  const needle = normalizeTitle(query);
  if (needle.length < 2 || limit <= 0) return [];
  const tokens = needle.split(" ");
  const seen = new Set<string>();

  return candidates
    .filter(({ id }) => {
      if (seen.has(id)) return false;
      seen.add(id);
      return true;
    })
    .map((candidate) => ({
      candidate,
      score: scoreTitle(needle, tokens, candidate.title),
    }))
    .filter(({ score }) => score >= MIN_SUGGESTION_SCORE)
    .sort(
      (a, b) =>
        b.score - a.score ||
        byText(a.candidate.title, b.candidate.title) ||
        byText(a.candidate.id, b.candidate.id),
    )
    .slice(0, limit)
    .map(({ candidate, score }) => ({
      id: candidate.id,
      title: candidate.title,
      score,
    }));
}
