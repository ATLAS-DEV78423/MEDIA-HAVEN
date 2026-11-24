import { describe, expect, it } from "vitest";

import type { Media } from "@/data/catalog";

import { recommendBecauseYouWatched } from "./recommend";

const media = (overrides: Partial<Media> & { id: string; title: string }): Media => ({
  year: null,
  kind: "movie",
  genres: [],
  runtime: "",
  minutes: null,
  synopsis: "",
  about: "",
  art: { hero: "", still: "" },
  ...overrides,
});

describe("recommendBecauseYouWatched", () => {
  it("ranks unwatched titles by shared genres with the seed", () => {
    const items: Media[] = [
      media({ id: "seed", title: "Seed", genres: ["Action", "Sci-Fi"], progress: 0.8 }),
      media({ id: "two", title: "Two Matches", genres: ["Action", "Sci-Fi", "Drama"] }),
      media({ id: "one", title: "One Match", genres: ["Action"] }),
      media({ id: "none", title: "No Match", genres: ["Romance"] }),
    ];

    const result = recommendBecauseYouWatched(items);

    expect(result?.seed.id).toBe("seed");
    expect(result?.picks.map((m) => m.id)).toEqual(["two", "one", "none"]);
  });

  it("excludes watched titles and the seed from picks", () => {
    const items: Media[] = [
      media({ id: "seed", title: "Seed", genres: ["Action"], progress: 0.9 }),
      media({ id: "watched", title: "Also Watched", genres: ["Action"], progress: 0.2 }),
      media({ id: "zero", title: "Just Started", genres: ["Action"], progress: 0 }),
      media({ id: "pick", title: "Fresh", genres: ["Action"] }),
    ];

    const result = recommendBecauseYouWatched(items);

    expect(result?.picks.map((m) => m.id)).toEqual(["pick", "zero"]);
  });

  it("picks the greatest-progress watched item as the seed, tie-broken by title", () => {
    const items: Media[] = [
      media({ id: "low", title: "Aardvark", genres: ["Action"], progress: 0.4 }),
      media({ id: "high", title: "Zebra", genres: ["Drama"], progress: 0.7 }),
      media({ id: "pick", title: "Pick", genres: ["Drama"] }),
    ];

    expect(recommendBecauseYouWatched(items)?.seed.id).toBe("high");

    const tied: Media[] = [
      media({ id: "b", title: "Zulu", genres: ["Action"], progress: 0.6 }),
      media({ id: "a", title: "Alpha", genres: ["Drama"], progress: 0.6 }),
      media({ id: "pick", title: "Pick", genres: ["Drama"] }),
    ];

    expect(recommendBecauseYouWatched(tied)?.seed.id).toBe("a");
  });

  it("breaks equal-overlap ties by title for deterministic output", () => {
    const items: Media[] = [
      media({ id: "seed", title: "Seed", genres: ["Action"], progress: 0.5 }),
      media({ id: "z", title: "Zulu", genres: ["Action"] }),
      media({ id: "a", title: "Alpha", genres: ["Action"] }),
      media({ id: "m", title: "Mike", genres: ["Action"] }),
    ];

    const result = recommendBecauseYouWatched(items);

    expect(result?.picks.map((m) => m.title)).toEqual(["Alpha", "Mike", "Zulu"]);
  });

  it("returns null when nothing has progress", () => {
    const items: Media[] = [
      media({ id: "a", title: "Alpha", genres: ["Action"] }),
      media({ id: "b", title: "Beta", genres: ["Action"], progress: 0 }),
    ];

    expect(recommendBecauseYouWatched(items)).toBeNull();
    expect(recommendBecauseYouWatched([])).toBeNull();
  });

  it("returns null when the seed has no unwatched candidates", () => {
    const items: Media[] = [
      media({ id: "seed", title: "Seed", genres: ["Action"], progress: 0.5 }),
    ];

    expect(recommendBecauseYouWatched(items)).toBeNull();
  });

  it("respects the limit", () => {
    const items: Media[] = [
      media({ id: "seed", title: "Seed", genres: ["Action"], progress: 0.5 }),
      media({ id: "a", title: "A", genres: ["Action"] }),
      media({ id: "b", title: "B", genres: ["Action"] }),
      media({ id: "c", title: "C", genres: ["Action"] }),
    ];

    expect(recommendBecauseYouWatched(items, 2)?.picks.map((m) => m.id)).toEqual(["a", "b"]);
  });

  it("does not mutate the input array or its items", () => {
    const items: Media[] = [
      media({ id: "seed", title: "Zulu", genres: ["Action"], progress: 0.5 }),
      media({ id: "other", title: "Alpha", genres: ["Action"], progress: 0.9 }),
      media({ id: "pick", title: "Beta", genres: ["Action"] }),
    ];
    const snapshot = JSON.stringify(items);

    recommendBecauseYouWatched(items);

    expect(JSON.stringify(items)).toBe(snapshot);
  });
});
