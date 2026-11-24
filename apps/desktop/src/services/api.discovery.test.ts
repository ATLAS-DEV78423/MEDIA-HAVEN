/* Wrapper contracts for the opt-in discovery commands (ADR-0024).
 * Fake bridge only — no live TMDB or indexer call. */

import { afterEach, describe, expect, it, vi } from "vitest";

import { api } from "./api";

let invoke: ReturnType<typeof vi.fn>;

function installInvoke(response: unknown) {
  invoke = vi.fn(async () => response);
  window.__TAURI_INTERNALS__ = { invoke } as unknown as {
    invoke: (cmd: string, args?: Record<string, unknown> | undefined) => Promise<unknown>;
  };
}

afterEach(() => {
  delete window.__TAURI_INTERNALS__;
});

describe("discovery command wrappers", () => {
  it("sourceSearch sends title and year verbatim", async () => {
    const response = { items: [], setup_required: true };
    installInvoke(response);

    expect(await api.sourceSearch("A Film", 2024)).toEqual(response);
    expect(invoke).toHaveBeenCalledWith("source_search", {
      title: "A Film",
      year: 2024,
    });
  });

  it("sourceSearch passes a null year through", async () => {
    installInvoke({ items: [] });

    await api.sourceSearch("A Film", null);

    expect(invoke).toHaveBeenCalledWith("source_search", {
      title: "A Film",
      year: null,
    });
  });

  it("catalogBrowse sends the closed kind set", async () => {
    const response = { items: [{ id: "1", title: "Result" }] };
    installInvoke(response);

    expect(await api.catalogBrowse("tv")).toEqual(response);
    expect(invoke).toHaveBeenCalledWith("catalog_browse", { kind: "tv" });
  });

  it("catalogSearch sends the query", async () => {
    installInvoke({ items: [] });

    await api.catalogSearch("arrival");

    expect(invoke).toHaveBeenCalledWith("catalog_search", { query: "arrival" });
  });

  it("catalogWantedAdd forwards all three fields and returns the media id", async () => {
    installInvoke({ mediaId: "catalog-movie-1" });

    expect(
      await api.catalogWantedAdd("Arrival", 2016, "movie"),
    ).toEqual({ mediaId: "catalog-movie-1" });
    expect(invoke).toHaveBeenCalledWith("catalog_wanted_add", {
      title: "Arrival",
      year: 2016,
      kind: "movie",
    });
  });

  it("catalogWantedAdd forwards a null year and series kind", async () => {
    installInvoke({ mediaId: "catalog-series-1" });

    await api.catalogWantedAdd("Unknown Year", null, "series");

    expect(invoke).toHaveBeenCalledWith("catalog_wanted_add", {
      title: "Unknown Year",
      year: null,
      kind: "series",
    });
  });
});
