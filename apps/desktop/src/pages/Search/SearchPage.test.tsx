import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { api } from "@/services/api";
import type { MediaSummary } from "@/services/types";
import { SearchPage } from "./SearchPage";

vi.mock("@/services/api", () => ({
  describeError: () => "The library service is not answering.",
  api: {
    search: vi.fn(async (_text: string, _genres: string[] = []) => ({ items: [] })),
  },
}));

function media(id: string, title: string, genres: string[]): MediaSummary {
  return {
    id,
    title,
    kind: "movie",
    year: null,
    synopsis: null,
    runtimeMinutes: null,
    genres,
    artwork: [],
    availability: { state: "available" },
    progress: null,
  };
}

vi.mock("@/state/AppState", () => ({
  useAppState: () => ({
    data: {
      home: { shelves: [] },
      movies: [
        media("m1", "Alpha", ["Drama", "Sci-Fi"]),
        media("m2", "Beta", ["Drama", "Comedy"]),
        media("m3", "Interstellar", ["Sci-Fi"]),
        media("m4", "Alphabet", ["Sci-Fi"]),
      ],
      shows: [media("s1", "Gamma", ["Comedy", "Thriller"])],
      roots: { counts: { movies: 2, shows: 1, episodes: 0 } },
      downloads: { enabled: false },
    },
    recentSearches: [],
    rememberSearch: vi.fn(),
  }),
}));

const search = vi.mocked(api.search);

function mount() {
  return render(
    <MemoryRouter>
      <SearchPage />
    </MemoryRouter>,
  );
}

describe("search genre chips", () => {
  beforeEach(() => {
    search.mockClear();
  });

  it("derives sorted, de-duplicated genre chips from the local library", () => {
    mount();

    expect(screen.getByRole("button", { name: "All" })).toBeInTheDocument();
    for (const genre of ["Comedy", "Drama", "Sci-Fi", "Thriller"]) {
      expect(screen.getByRole("button", { name: genre })).toBeInTheDocument();
    }
    // "Drama" appears in two movies and "Comedy" in a movie and a show, but
    // each must render a single chip.
    expect(screen.getAllByRole("button", { name: "Drama" })).toHaveLength(1);
    expect(screen.getAllByRole("button", { name: "Comedy" })).toHaveLength(1);
  });

  it("passes the selected genre to api.search and re-runs when it changes", async () => {
    const user = userEvent.setup();
    mount();

    await user.type(screen.getByLabelText(/search your library/i), "matrix");
    await waitFor(() => {
      expect(search).toHaveBeenCalledWith("matrix", []);
    });

    await user.click(screen.getByRole("button", { name: "Drama" }));
    await waitFor(() => {
      expect(search).toHaveBeenCalledWith("matrix", ["Drama"]);
    });

    await user.click(screen.getByRole("button", { name: "All" }));
    await waitFor(() => {
      expect(search).toHaveBeenLastCalledWith("matrix", []);
    });
  });

  it("offers a tolerant 'Did you mean?' chip that re-searches the exact title", async () => {
    const user = userEvent.setup();
    mount();

    await user.type(screen.getByLabelText(/search your library/i), "interstelar");
    const suggestion = await screen.findByRole("button", { name: "Interstellar" });
    expect(screen.getByRole("group", { name: /did you mean/i })).toBeInTheDocument();

    await user.click(suggestion);

    await waitFor(() => {
      expect(search).toHaveBeenCalledWith("Interstellar", []);
    });
    expect(document.body.textContent).not.toMatch(
      /[A-Za-z]:\\|\/home\/|dev-only-not-for-production/,
    );
  });

  it("does not re-suggest an exact title match while results stay sparse", async () => {
    const user = userEvent.setup();
    mount();

    await user.type(screen.getByLabelText(/search your library/i), "Alpha");
    expect(await screen.findByRole("button", { name: "Alphabet" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Alpha" })).toBeNull();
  });
});
