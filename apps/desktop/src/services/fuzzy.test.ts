import { describe, expect, it } from "vitest";

import {
  levenshtein,
  MIN_SUGGESTION_SCORE,
  normalizeTitle,
  similarity,
  suggestTitles,
  type SuggestionCandidate,
} from "./fuzzy";

const candidates: SuggestionCandidate[] = [
  { id: "1", title: "The Dark Knight" },
  { id: "2", title: "Interstellar" },
  { id: "3", title: "Batman Begins" },
  { id: "4", title: "Arrival" },
  { id: "5", title: "Dune" },
];

describe("normalizeTitle", () => {
  it("ignores case, leading articles, punctuation and repeated whitespace", () => {
    expect(normalizeTitle("The Dark Knight")).toBe("dark knight");
    expect(normalizeTitle("dark knight")).toBe("dark knight");
    expect(normalizeTitle("  The. DARK—Knight!  ")).toBe("dark knight");
    expect(normalizeTitle("An\n\tInterstellar's    Tale")).toBe("interstellar s tale");
    expect(normalizeTitle("A Movie")).toBe("movie");
    expect(normalizeTitle("Matrix")).toBe("matrix");
  });

  it("makes equivalent titles score identically", () => {
    expect(similarity("The Dark Knight", "dark knight")).toBe(1);
    expect(levenshtein("The Dark Knight", "dark knight")).toBeGreaterThan(0);
  });
});

describe("levenshtein / similarity", () => {
  it("counts raw edits and returns a bounded 0..1 similarity", () => {
    expect(levenshtein("kitten", "sitting")).toBe(3);
    expect(levenshtein("", "abc")).toBe(3);
    expect(similarity("interstelar", "Interstellar")).toBeGreaterThan(0.9);
    expect(similarity("interstelar", "Interstellar")).toBeLessThanOrEqual(1);
    expect(similarity("movie", "unrelated")).toBeLessThan(MIN_SUGGESTION_SCORE);
  });
});

describe("suggestTitles", () => {
  it("tolerates a spelling error", () => {
    expect(suggestTitles("interstelar", candidates)[0]?.title).toBe("Interstellar");
  });

  it("ranks substring matches highly", () => {
    expect(suggestTitles("knight", candidates)[0]?.title).toBe("The Dark Knight");
    expect(suggestTitles("dark", candidates).map((item) => item.title)).toContain(
      "The Dark Knight",
    );
  });

  it("drops candidates below the documented threshold", () => {
    expect(suggestTitles("interstelar", candidates).map((item) => item.id)).not.toContain("4");
    expect(suggestTitles("zzzzzzzz", candidates)).toEqual([]);
    expect(suggestTitles("a", candidates)).toEqual([]);
  });

  it("orders by score then title and is deterministic", () => {
    const tied: SuggestionCandidate[] = [
      { id: "z", title: "Batman Returns" },
      { id: "a", title: "Batman Begins" },
      { id: "m", title: "Batman" },
    ];
    const first = suggestTitles("bat", tied);
    const second = suggestTitles("bat", tied);
    expect(second).toEqual(first);
    expect(first.map((item) => item.title)).toEqual([
      "Batman",
      "Batman Begins",
      "Batman Returns",
    ]);
  });

  it("caps the result list and de-duplicates ids", () => {
    const many: SuggestionCandidate[] = [
      ...candidates,
      { id: "1", title: "The Dark Knight" },
      { id: "6", title: "Batman" },
      { id: "7", title: "Batman Returns" },
    ];
    const limited = suggestTitles("bat", many, 2);
    expect(limited).toHaveLength(2);
    expect(new Set(limited.map((item) => item.id)).size).toBe(2);
    expect(suggestTitles("bat", many, 0)).toEqual([]);
  });
});
