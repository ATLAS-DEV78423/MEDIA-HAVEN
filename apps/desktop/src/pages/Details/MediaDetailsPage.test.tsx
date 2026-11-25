import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
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

async function mount(id = "m1", title = "A Local Film") {
  render(
    <MemoryRouter initialEntries={[`/details/${id}`]}>
      <Routes>
        <Route path="/details/:mediaId" element={<MediaDetailsPage />} />
      </Routes>
    </MemoryRouter>,
  );
  await screen.findByRole("heading", { name: title });
}

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

  it("keeps the disabled Play span for a series with no episodes", async () => {
    detailResult = SERIES_DETAIL;
    await mount("s1", "A Local Series");

    const disabled = screen
      .getByText("No episodes indexed")
      .closest("span[aria-disabled]");
    expect(disabled).toHaveAttribute("aria-disabled", "true");
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
