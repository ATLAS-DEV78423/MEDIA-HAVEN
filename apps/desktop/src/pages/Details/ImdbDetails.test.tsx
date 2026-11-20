/* Fake-DTO rendering proof for the IMDb display fields. No network, no key. */

import { render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { IMDB_ATTRIBUTION } from "@/components/cards/ImdbRating";
import type { MovieDetail } from "@/services/types";
import { MediaDetailsPage } from "./MediaDetailsPage";

vi.mock("@/state/AppState", () => ({
  useAppState: () => ({
    data: { movies: [], shows: [] },
    isListed: () => false,
    toggleList: () => undefined,
  }),
}));

const RATED: MovieDetail = {
  id: "m1",
  kind: "movie",
  title: "A Rated Film",
  year: 2024,
  synopsis: null,
  runtimeMinutes: 100,
  genres: [],
  artwork: [],
  progress: null,
  director: null,
  cast: [],
  availability: { state: "available" },
  imdbRating: 9.3,
  voteCount: 2_500_000,
  ageRating: "R",
};

let detail: MovieDetail;
let invoke: ReturnType<typeof vi.fn>;

beforeEach(() => {
  detail = RATED;
  invoke = vi.fn(async (command: string) => {
    if (command === "library_detail") return detail;
    if (command === "library_episodes") return [];
    throw new Error(`unexpected command: ${command}`);
  });
  window.__TAURI_INTERNALS__ = { invoke };
});

afterEach(() => {
  delete window.__TAURI_INTERNALS__;
  vi.clearAllMocks();
});

async function mount() {
  render(
    <MemoryRouter initialEntries={["/details/m1"]}>
      <Routes>
        <Route path="/details/:mediaId" element={<MediaDetailsPage />} />
      </Routes>
    </MemoryRouter>,
  );
  await screen.findByRole("heading", { name: "A Rated Film" });
}

describe("IMDb display in Details", () => {
  it("shows the cached rating, vote count, age rating and attribution", async () => {
    await mount();
    const badge = await screen.findByTestId("imdb-rating");
    expect(badge).toHaveTextContent("9.3");
    expect(badge).toHaveTextContent("2,500,000");
    expect(badge).toHaveTextContent("R");
    expect(screen.getByText(IMDB_ATTRIBUTION)).toBeTruthy();
  });

  it("shows no rating and invents nothing when fields are absent", async () => {
    detail = { ...RATED };
    delete detail.imdbRating;
    delete detail.voteCount;
    delete detail.ageRating;
    await mount();
    expect(screen.queryByTestId("imdb-rating")).toBeNull();
    expect(document.body.textContent).not.toContain(IMDB_ATTRIBUTION);
  });
});
