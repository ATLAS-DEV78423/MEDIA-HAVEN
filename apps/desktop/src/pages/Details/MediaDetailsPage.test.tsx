import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Link, MemoryRouter, Route, Routes } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { EpisodeSummary, MediaDetail } from "@/services/types";
import { MediaDetailsPage } from "./MediaDetailsPage";

vi.mock("@/state/AppState", () => ({
  useAppState: () => ({
    data: { movies: [], shows: [] },
    isListed: () => false,
    toggleList: () => undefined,
  }),
}));

const DETAIL: MediaDetail = {
  id: "m1",
  kind: "movie",
  title: "A Local Film",
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

const SERIES_DETAIL: MediaDetail = {
  id: "s1",
  kind: "series",
  title: "A Local Series",
  year: 2023,
  synopsis: null,
  genres: [],
  artwork: [],
  seasons: [{ number: 1, title: "Season 1", episodeCount: 1 }],
};

const EPISODE: EpisodeSummary = {
  id: "e1",
  seriesId: "s1",
  seriesTitle: "A Local Series",
  seasonNumber: 1,
  episodeNumber: 1,
  title: "Pilot",
  runtimeMinutes: 45,
  artwork: [],
  availability: { state: "available" },
  progress: null,
  nextUp: false,
};

let invoke: ReturnType<typeof vi.fn>;
let searchResult: unknown;
let detailResult: MediaDetail;
let episodeResult: EpisodeSummary[];

const MAGNET =
  "magnet:?xt=urn:btih:TESTHASH&dn=film&tr=https%3A%2F%2Ftracker.example%2Fannounce%3Fkey%3DSECRET123";

const SHORTLIST = [
  {
    title: "A Local Film 2024 1080p",
    sizeBytes: 2147483648,
    seeders: 42,
    resolution: "1080p",
    source: "Example Tracker",
    magnet: MAGNET,
  },
];

beforeEach(() => {
  searchResult = { items: SHORTLIST };
  detailResult = DETAIL;
  episodeResult = [];
  invoke = vi.fn(async (command: string) => {
    switch (command) {
      case "library_detail":
        return detailResult;
      case "library_episodes":
        return episodeResult;
      case "source_search":
        return searchResult;
      case "source_set":
        return { downloadId: "d-started" };
      default:
        throw new Error(`unexpected command: ${command}`);
    }
  });
  window.__TAURI_INTERNALS__ = { invoke };
});

afterEach(() => {
  delete window.__TAURI_INTERNALS__;
  vi.clearAllMocks();
});

async function mount(id = "m1", title = "A Local Film", state: unknown = null) {
  render(
    <MemoryRouter initialEntries={[{ pathname: `/details/${id}`, state }]}>
      <Routes>
        <Route path="/details/:mediaId" element={<MediaDetailsPage />} />
      </Routes>
    </MemoryRouter>,
  );
  await screen.findByRole("heading", { name: title });
}

describe("Details expanded view", () => {
  it("uses the clicked catalog poster as the backdrop", async () => {
    const poster = "https://example.test/poster.jpg";
    await mount("m1", "A Local Film", { poster });

    await waitFor(() => {
      expect(document.querySelector('[data-testid="detail-backdrop"]')).toHaveAttribute(
        "src",
        poster,
      );
    });
  });
});

describe("Details source field", () => {
  it("validates before sending and saves through the named command", async () => {
    const user = userEvent.setup();
    await mount();

    const input = screen.getByLabelText(/^source$/i);
    const save = screen.getByRole("button", { name: /save source/i });

    await user.click(save);
    expect(screen.getByRole("alert")).toHaveTextContent(/enter a magnet/i);
    expect(invoke).not.toHaveBeenCalledWith("source_set", expect.anything());

    await user.type(input, "/downloads/episode.torrent");
    await user.click(save);
    expect(screen.getByRole("alert")).toHaveTextContent(/no paths/i);
    expect(invoke).not.toHaveBeenCalledWith("source_set", expect.anything());

    const magnet = "magnet:?xt=urn:btih:ABC123&dn=episode";
    await user.clear(input);
    await user.type(input, magnet);
    await user.click(save);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("source_set", {
        media_id: "m1",
        source: magnet,
      });
    });
    expect(await screen.findByRole("status")).toHaveTextContent(/download started/i);
    expect(
      screen.getByRole("link", { name: /track progress in downloads/i }),
    ).toHaveAttribute("href", "/downloads");
    expect(document.body.textContent).not.toContain("/downloads/");
  });

  it("accepts a real multi-tracker magnet longer than 2048 characters", async () => {
    const user = userEvent.setup();
    await mount();

    const magnet = `magnet:?xt=urn:btih:ABC123&dn=ep&tr=${"x".repeat(2200)}`;
    expect(magnet.length).toBeGreaterThan(2048);
    fireEvent.change(screen.getByLabelText(/^source$/i), {
      target: { value: magnet },
    });
    await user.click(screen.getByRole("button", { name: /save source/i }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("source_set", {
        media_id: "m1",
        source: magnet,
      });
    });
    expect(screen.queryByRole("alert")).toBeNull();
    expect(await screen.findByRole("status")).toHaveTextContent(/download started/i);
  });
});

describe("Details shortlist", () => {
  it("searches, shows ranked fields and confirms the chosen magnet", async () => {
    const user = userEvent.setup();
    await mount();

    await user.click(screen.getByRole("button", { name: /^find sources$/i }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("source_search", {
        title: "A Local Film",
        year: 2024,
      });
    });

    const list = await screen.findByRole("list", { name: "Source candidates" });
    const row = within(list).getByRole("listitem");
    expect(row).toHaveTextContent(/42 seeders/);
    expect(row).toHaveTextContent(/1080p/);
    expect(row).toHaveTextContent(/Example Tracker/);

    await user.click(within(row).getByRole("button", { name: /^confirm$/i }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("source_set", {
        media_id: "m1",
        source: MAGNET,
      });
    });
    expect(await screen.findByRole("status")).toHaveTextContent(/download started/i);
    expect(within(row).getByText(/download started/i)).toBeTruthy();
    expect(within(row).getByRole("button", { name: /confirmed/i })).toBeDisabled();

    // The chosen magnet carries a tracker credential; it goes to source_set
    // verbatim but is never painted into the renderer's DOM.
    const rendered = document.body.textContent ?? "";
    expect(rendered).not.toContain(MAGNET);
    expect(rendered).not.toContain("SECRET123");
    expect(rendered).not.toContain("/downloads/");
  });

  it("shows a clear setup state instead of an empty result", async () => {
    const user = userEvent.setup();
    searchResult = { items: [], setup_required: true };
    await mount();

    await user.click(screen.getByRole("button", { name: /^find sources$/i }));

    expect(await screen.findByRole("status")).toHaveTextContent(/not set up/i);
    expect(
      screen.queryByRole("list", { name: "Source candidates" }),
    ).toBeNull();
  });
});

describe("Details play control", () => {
  it("links a movie with no episodes to its own player route", async () => {
    await mount();

    const play = screen.getByRole("link", { name: /^play$/i });
    expect(play).toHaveAttribute("href", "/player/m1");
    expect(screen.queryByText("No episodes indexed")).toBeNull();
  });

  it("offers the acquire control for a series with no episodes", async () => {
    detailResult = SERIES_DETAIL;
    await mount("s1", "A Local Series");

    expect(screen.getByRole("button", { name: /^play$/i })).toBeTruthy();
    expect(screen.queryByText("No episodes indexed")).toBeNull();
    expect(screen.queryByRole("link", { name: /^play$/i })).toBeNull();
  });

  it("uses the first episode id for a series with an episode", async () => {
    detailResult = SERIES_DETAIL;
    episodeResult = [EPISODE];
    await mount("s1", "A Local Series");

    const play = await screen.findByRole("link", { name: /^play$/i });
    expect(play).toHaveAttribute("href", "/player/e1");
  });
});

describe("Details wanted-state copy", () => {
  it("does not show a zero season count for a series with no episodes", async () => {
    detailResult = { ...SERIES_DETAIL, seasons: [] };
    await mount("s1", "A Local Series");

    expect(screen.queryByText("Seasons")).toBeNull();
    expect(screen.queryByText("Episodes")).toBeNull();
  });

  it("shows downloading instead of file missing while acquiring", async () => {
    const user = userEvent.setup();
    detailResult = { ...DETAIL, availability: { state: "missing-file" } };
    await mount();

    await user.click(screen.getByRole("button", { name: /^play$/i }));

    await waitFor(() => {
      expect(screen.getAllByText(/downloading/i).length).toBeGreaterThan(0);
    });
    expect(screen.queryByText("File missing")).toBeNull();
  });
});

describe("Details source search failures", () => {
  it("shows a search failure instead of an empty shortlist", async () => {
    const user = userEvent.setup();
    searchResult = { items: [], error: "Source search could not be reached." };
    await mount();

    await user.click(screen.getByRole("button", { name: /^find sources$/i }));

    expect(await screen.findByRole("alert")).toHaveTextContent(/could not be reached/i);
    expect(screen.queryByText(/no candidates were returned/i)).toBeNull();
  });

  it("reports a search failure when Play cannot reach the indexer", async () => {
    const user = userEvent.setup();
    detailResult = { ...DETAIL, availability: { state: "missing-file" } };
    searchResult = { items: [], error: "Source search could not be reached." };
    await mount();

    await user.click(screen.getByRole("button", { name: /^play$/i }));

    expect(await screen.findByRole("alert")).toHaveTextContent(/could not be reached/i);
    expect(invoke).not.toHaveBeenCalledWith("source_set", expect.anything());
  });
});

describe("Details acquisition progress", () => {
  it("finds a source, shows progress, then offers Play Now", async () => {
    detailResult = { ...DETAIL, availability: { state: "missing-file" } };
    await mount();

    let polls = 0;
    invoke = vi.fn(async (command: string) => {
      switch (command) {
        case "library_detail":
          return detailResult;
        case "source_search":
          return searchResult;
        case "source_set":
          return { downloadId: "d1" };
        case "download_status":
          polls += 1;
          return [
            {
              id: "d1",
              torrentHash: "h",
              mediaId: "m1",
              state: polls === 1 ? "downloading" : "ready",
              progress: polls === 1 ? 0.4 : 1,
            },
          ];
        default:
          return [];
      }
    });
    window.__TAURI_INTERNALS__ = { invoke };

    const flush = () =>
      act(async () => {
        await Promise.resolve();
        await Promise.resolve();
        await Promise.resolve();
      });

    vi.useFakeTimers();
    try {
      await act(async () => {
        fireEvent.click(screen.getByRole("button", { name: /^play$/i }));
      });
      await flush();
      expect(invoke).toHaveBeenCalledWith("source_search", {
        title: "A Local Film",
        year: 2024,
      });
      expect(invoke).toHaveBeenCalledWith("source_set", {
        media_id: "m1",
        source: MAGNET,
      });

      await act(async () => {
        vi.advanceTimersByTime(2000);
      });
      await flush();
      expect(screen.getByRole("status").textContent).toMatch(/downloading/i);
      expect(screen.getByRole("status").textContent).toContain("40%");

      await act(async () => {
        vi.advanceTimersByTime(2000);
      });
      await flush();
      expect(screen.getByRole("link", { name: /play now/i })).toHaveAttribute(
        "href",
        "/player/m1",
      );
    } finally {
      vi.useRealTimers();
    }
  });

  it("finds a source for a series with no episodes and offers Play Now", async () => {
    detailResult = SERIES_DETAIL;
    episodeResult = [];
    await mount("s1", "A Local Series");

    let polls = 0;
    invoke = vi.fn(async (command: string) => {
      switch (command) {
        case "library_detail":
          return detailResult;
        case "library_episodes":
          return episodeResult;
        case "source_search":
          return searchResult;
        case "source_set":
          return { downloadId: "d-series" };
        case "download_status":
          polls += 1;
          return [
            {
              id: "d-series",
              torrentHash: "h-series",
              mediaId: "s1",
              state: polls === 1 ? "downloading" : "ready",
              progress: polls === 1 ? 0.25 : 1,
            },
          ];
        default:
          return [];
      }
    });
    window.__TAURI_INTERNALS__ = { invoke };

    const flush = () =>
      act(async () => {
        await Promise.resolve();
        await Promise.resolve();
        await Promise.resolve();
      });

    vi.useFakeTimers();
    try {
      await act(async () => {
        fireEvent.click(screen.getByRole("button", { name: /^play$/i }));
      });
      await flush();
      expect(invoke).toHaveBeenCalledWith("source_search", {
        title: "A Local Series",
        year: 2023,
      });
      expect(invoke).toHaveBeenCalledWith("source_set", {
        media_id: "s1",
        source: MAGNET,
      });

      await act(async () => {
        vi.advanceTimersByTime(2000);
      });
      await flush();
      expect(screen.getByRole("status").textContent).toContain("25%");

      await act(async () => {
        vi.advanceTimersByTime(2000);
      });
      await flush();
      expect(screen.getByRole("link", { name: /play now/i })).toHaveAttribute(
        "href",
        "/player/s1",
      );
    } finally {
      vi.useRealTimers();
    }
  });

  it("reports a failed download instead of showing endless progress", async () => {
    detailResult = { ...DETAIL, availability: { state: "missing-file" } };
    await mount();

    let polls = 0;
    invoke = vi.fn(async (command: string) => {
      switch (command) {
        case "library_detail":
          return detailResult;
        case "source_search":
          return searchResult;
        case "source_set":
          return { downloadId: "d1" };
        case "download_status":
          polls += 1;
          return [
            {
              id: "d1",
              torrentHash: "h",
              mediaId: "m1",
              state: "error",
              progress: 0.1,
            },
          ];
        default:
          return [];
      }
    });
    window.__TAURI_INTERNALS__ = { invoke };

    const flush = () =>
      act(async () => {
        await Promise.resolve();
        await Promise.resolve();
        await Promise.resolve();
      });

    vi.useFakeTimers();
    try {
      await act(async () => {
        fireEvent.click(screen.getByRole("button", { name: /^play$/i }));
      });
      await flush();
      await act(async () => {
        vi.advanceTimersByTime(2000);
      });
      await flush();

      expect(screen.getByRole("alert")).toHaveTextContent(
        "The download failed. Try another source.",
      );
      expect(screen.getByRole("status").textContent).not.toMatch(/downloading/i);

      await act(async () => {
        vi.advanceTimersByTime(4000);
      });
      await flush();
      expect(polls).toBe(1);
    } finally {
      vi.useRealTimers();
    }
  });

  it("does not install the previous title's poll after a mid-search route change", async () => {
    detailResult = SERIES_DETAIL;
    episodeResult = [];
    let resolveSearch: (value: unknown) => void = () => undefined;
    const pendingSearch = new Promise<unknown>((resolve) => {
      resolveSearch = resolve;
    });
    let statusCalls = 0;
    invoke = vi.fn(async (command: string) => {
      switch (command) {
        case "library_detail":
          return detailResult;
        case "library_episodes":
          return episodeResult;
        case "source_search":
          return pendingSearch;
        case "source_set":
          return { downloadId: "d1" };
        case "download_status":
          statusCalls += 1;
          return [];
        default:
          return [];
      }
    });
    window.__TAURI_INTERNALS__ = { invoke };

    render(
      <MemoryRouter initialEntries={[{ pathname: "/details/s1", state: null }]}>
        <Link to="/details/s2" data-testid="switch-to-s2">
          switch
        </Link>
        <Routes>
          <Route path="/details/:mediaId" element={<MediaDetailsPage />} />
        </Routes>
      </MemoryRouter>,
    );
    await screen.findByRole("heading", { name: "A Local Series" });

    const flush = () =>
      act(async () => {
        await Promise.resolve();
        await Promise.resolve();
        await Promise.resolve();
      });

    vi.useFakeTimers();
    try {
      await act(async () => {
        fireEvent.click(screen.getByRole("button", { name: /^play$/i }));
      });
      await flush();

      await act(async () => {
        fireEvent.click(screen.getByTestId("switch-to-s2"));
      });
      await act(async () => {
        vi.advanceTimersByTime(1);
      });
      await flush();
      const callsBeforeResolve = statusCalls;

      await act(async () => {
        resolveSearch({ items: SHORTLIST });
      });
      await flush();
      await act(async () => {
        vi.advanceTimersByTime(5000);
      });
      await flush();

      expect(invoke).not.toHaveBeenCalledWith("source_set", expect.anything());
      expect(statusCalls).toBe(callsBeforeResolve);
    } finally {
      vi.useRealTimers();
    }
  });

  it("offers Play Now on load when a ready download already exists", async () => {
    detailResult = { ...DETAIL, availability: { state: "missing-file" } };
    invoke = vi.fn(async (command: string) => {
      switch (command) {
        case "library_detail":
          return detailResult;
        case "download_status":
          return [
            {
              id: "d1",
              torrentHash: "h",
              mediaId: "m1",
              state: "ready",
              progress: 1,
            },
          ];
        default:
          return [];
      }
    });
    window.__TAURI_INTERNALS__ = { invoke };

    await mount();

    const play = await screen.findByRole("link", { name: /play now/i });
    expect(play).toHaveAttribute("href", "/player/m1");
  });
});
