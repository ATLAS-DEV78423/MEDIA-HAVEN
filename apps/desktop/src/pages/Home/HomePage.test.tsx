import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, useLocation } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { api, type CatalogItem } from "@/services/api";
import { HomePage } from "./HomePage";

const MOVIES: CatalogItem[] = [
  {
    id: "301",
    title: "Dune",
    year: 2021,
    kind: "movie",
    overview: "A desert planet.",
    posterPath: "/dune.jpg",
  },
];
const TV: CatalogItem[] = [
  {
    id: "401",
    title: "The Quiet Signal",
    year: 2024,
    kind: "series",
    overview: "A radio operator hears a reply.",
    posterPath: null,
  },
];

vi.mock("@/state/AppState", () => ({
  useAppState: () => ({
    data: {
      home: { hero: null, shelves: [] },
      movies: [],
      watchlist: { entries: [] },
      roots: { counts: { movies: 0, shows: 0, episodes: 0 } },
    },
    activeProfile: null,
  }),
}));

vi.mock("@/services/api", () => ({
  api: {
    catalogBrowse: vi.fn(),
    catalogWantedAdd: vi.fn(),
  },
}));

const browse = vi.mocked(api.catalogBrowse);
const wantedAdd = vi.mocked(api.catalogWantedAdd);

beforeEach(() => {
  browse.mockReset();
  wantedAdd.mockReset();
  browse.mockImplementation(async (kind) => ({ items: kind === "movie" ? MOVIES : TV }));
  wantedAdd.mockResolvedValue({ mediaId: "catalog-new" });
});

function RouteProbe() {
  const { pathname } = useLocation();
  return <span data-testid="route">{pathname}</span>;
}

function mount() {
  return render(
    <MemoryRouter initialEntries={["/"]}>
      <HomePage />
      <RouteProbe />
    </MemoryRouter>,
  );
}

describe("Home catalog shelves", () => {
  it("renders trending shelves with TMDB posters and a bundled fallback", async () => {
    mount();

    expect(
      await screen.findByRole("heading", { name: "Trending Movies" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Trending TV" })).toBeInTheDocument();
    expect(browse).toHaveBeenCalledWith("movie");
    expect(browse).toHaveBeenCalledWith("tv");

    const poster = screen.getByAltText("Dune poster");
    expect(poster).toHaveAttribute(
      "src",
      "https://image.tmdb.org/t/p/w500/dune.jpg",
    );
    expect(poster).toHaveAttribute("loading", "lazy");
    const fallback = screen.getByAltText("The Quiet Signal poster");
    expect(fallback.getAttribute("src")).toContain("/artwork/");
    expect(fallback.getAttribute("src")).not.toContain("image.tmdb.org");
  });

  it("plays a poster card: wanted item then Details, with no path or magnet", async () => {
    const user = userEvent.setup();
    mount();

    await user.click(await screen.findByRole("button", { name: /play dune, 2021/i }));

    await waitFor(() => {
      expect(wantedAdd).toHaveBeenCalledWith("Dune", 2021, "movie");
    });
    expect(screen.getByTestId("route")).toHaveTextContent("/details/catalog-new");
    expect(document.body.textContent).not.toMatch(
      /magnet:|[A-Za-z]:\\|\/home\/|dev-only-not-for-production/,
    );
  });

  it("renders no catalog shelves when the catalog is not set up", async () => {
    browse.mockResolvedValue({ items: [], setup_required: true });
    mount();

    await waitFor(() => expect(browse).toHaveBeenCalledTimes(2));
    expect(screen.queryByRole("heading", { name: "Trending Movies" })).toBeNull();
    expect(screen.queryByRole("heading", { name: "Trending TV" })).toBeNull();
  });

  it("stays quiet after one bounded retry when a catalog browse keeps failing", async () => {
    browse.mockRejectedValue(new Error("catalog unavailable"));
    mount();

    await waitFor(() => expect(browse).toHaveBeenCalledTimes(4), { timeout: 3000 });
    expect(screen.queryByRole("heading", { name: /trending/i })).toBeNull();
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.queryByTestId("catalog-stale-note")).toBeNull();
  });

  it("recovers trending shelves when the retry succeeds", async () => {
    browse
      .mockRejectedValueOnce(new Error("throttled"))
      .mockRejectedValueOnce(new Error("throttled"))
      .mockImplementation(async (kind) => ({ items: kind === "movie" ? MOVIES : TV }));
    mount();

    expect(
      await screen.findByRole("heading", { name: "Trending Movies" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Trending TV" })).toBeInTheDocument();
    await waitFor(() => expect(browse).toHaveBeenCalledTimes(4), { timeout: 3000 });
    expect(screen.queryByTestId("catalog-stale-note")).toBeNull();
  });
});
