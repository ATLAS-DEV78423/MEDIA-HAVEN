/* Frontend half of the opt-in artwork path. A hostile or absent DTO must never
 * become an external/script image source — the renderer always falls back to
 * bundled art. Fakes only; no network and no live TMDB call. */

import { describe, expect, it } from "vitest";

import { artFromDto } from "./adapt";
import { apiBase } from "./errors";
import type { ArtworkRef } from "./types";

const HASH = "a".repeat(64);
const poster = (url: string | null): ArtworkRef[] => [
  { variant: "poster", url, alt: "Arrival poster" },
];
const bundled = () => artFromDto([], "m1", "Arrival");

describe("artFromDto", () => {
  it("uses the backend poster URL when present", () => {
    const art = artFromDto(poster(`/artwork/${HASH}`), "m1", "Arrival");
    expect(art.still).toBe(`${apiBase}/artwork/${HASH}`);
    expect(art.hero).toBe(`${apiBase}/artwork/${HASH}`);
  });

  it("falls back to bundled art when the backend sends no artwork", () => {
    const art = bundled();
    expect(art.still).toMatch(/^\/artwork\//);
    expect(art.hero).toMatch(/^\/artwork\//);
  });

  it("falls back to bundled art when artwork is undefined", () => {
    const art = artFromDto(undefined, "m1", "Arrival");
    expect(art.still).toBe(bundled().still);
    expect(art.hero).toBe(bundled().hero);
  });

  it("never renders a javascript:, absolute third-party URL or wrong-shape path", () => {
    for (const bad of [
      "javascript:alert(1)",
      "https://evil.example/poster.jpg",
      `/artwork/${"A".repeat(64)}`,
      "/artwork/abc",
      "/artwork/zzzz",
    ]) {
      const art = artFromDto(poster(bad), "m1", "Arrival");
      expect(art.still).toMatch(/^\/artwork\//);
      expect(art.hero).toMatch(/^\/artwork\//);
      expect(art.still).not.toContain("evil.example");
      expect(art.still).not.toContain("javascript:");
    }
  });
});
