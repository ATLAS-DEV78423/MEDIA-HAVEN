/* The renderer-facing link that finally puts offline IMDb data on screen:
 * api.detail() merges the opt-in lookup result into the detail DTO. No network.
 */

import { afterEach, describe, expect, it, vi } from "vitest";

import { api } from "./api";
import type { MovieDetail } from "./types";

const BASE: MovieDetail = {
  id: "m1",
  kind: "movie",
  title: "A Film",
  year: 2024,
  synopsis: null,
  runtimeMinutes: 100,
  genres: [],
  artwork: [],
  progress: null,
  director: null,
  cast: [],
  availability: { state: "available" },
};

function installInvoke(imdb: () => Promise<unknown> | unknown) {
  const invoke = vi.fn(async (command: string) => {
    if (command === "library_detail") return BASE;
    if (command === "imdb_lookup") return imdb();
    throw new Error(`unexpected command: ${command}`);
  });
  window.__TAURI_INTERNALS__ = { invoke } as unknown as {
    invoke: (cmd: string, args?: Record<string, unknown> | undefined) => Promise<unknown>;
  };
  return invoke;
}

afterEach(() => {
  delete window.__TAURI_INTERNALS__;
});

describe("api.detail IMDb merge", () => {
  it("populates the display fields from the offline lookup", async () => {
    installInvoke(() => ({ imdbRating: 8.1, voteCount: 1200, ageRating: "PG-13" }));

    const detail = await api.detail("m1");

    expect(detail.imdbRating).toBe(8.1);
    expect(detail.voteCount).toBe(1200);
    expect(detail.ageRating).toBe("PG-13");
  });

  it("leaves the local detail intact when the lookup is unavailable", async () => {
    installInvoke(() => {
      throw new Error("IMDb opt-in is off");
    });

    const detail = await api.detail("m1");

    expect(detail.imdbRating).toBeUndefined();
    expect(detail.title).toBe("A Film");
  });
});
