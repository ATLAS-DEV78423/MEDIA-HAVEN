import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, useLocation } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { DiscoverPage } from "./DiscoverPage";

const MOVIES = [
  {
    id: "101",
    title: "Arrival",
    year: 2016,
    kind: "movie" as const,
    overview: "A linguist works with the military.",
    posterPath: "/arrival-poster.jpg",
  },
];
const TV = [
  {
    id: "202",
    title: "The Quiet Signal",
    year: 2024,
    kind: "series" as const,
    overview: "A radio operator hears a reply.",
    posterPath: null,
  },
];
const ANIME = [
  {
    id: "anilist-1",
    title: "Spirited Away",
    year: 2001,
    kind: "movie" as const,
    overview: "A girl enters a world of spirits.",
    posterPath: null,
    posterUrl: "https://s4.anilist.co/file/anilistcdn/media/anime/cover/large/1.jpg",
  },
];

let invoke: ReturnType<typeof vi.fn>;

beforeEach(() => {
  invoke = vi.fn(async (command: string, args?: Record<string, unknown>) => {
    switch (command) {
      case "catalog_browse":
        return { items: args?.kind === "tv" ? TV : MOVIES };
      case "catalog_search":
        return { items: String(args?.query).toLowerCase() === "arrival" ? MOVIES : [] };
      case "catalog_anime":
        return args?.query
          ? { items: String(args.query).toLowerCase() === "spirited away" ? ANIME : [] }
          : { items: ANIME };
      case "catalog_wanted_add":
        return { mediaId: "catalog-movie-101" };
      default:
        throw new Error(`unexpected command: ${command}`);
    }
  });
  window.__TAURI_INTERNALS__ = { invoke } as unknown as {
    invoke: (cmd: string, args?: Record<string, unknown> | undefined) => Promise<unknown>;
  };
});

afterEach(() => {
  delete window.__TAURI_INTERNALS__;
  vi.clearAllMocks();
});

function RouteProbe() {
  const { pathname, state } = useLocation();
  const poster = (state as { poster?: string } | null)?.poster ?? "";
  return (
    <span data-testid="route">
      {pathname} {poster}
    </span>
  );
}

function mount() {
  render(
    <MemoryRouter initialEntries={["/discover"]}>
      <DiscoverPage />
      <RouteProbe />
    </MemoryRouter>,
  );
}

describe("Discover catalog", () => {
  it("browses movies by default and switches to TV", async () => {
    const user = userEvent.setup();
    mount();

    expect(
      await screen.findByRole("button", { name: /play arrival, 2016/i }),
    ).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("catalog_browse", { kind: "movie" });
    expect(
      screen.getByText(
        "This product uses the TMDB API but is not endorsed or certified by TMDB.",
      ),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "TV" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("catalog_browse", { kind: "tv" });
    });
    expect(
      await screen.findByRole("button", {
        name: /play the quiet signal, 2024/i,
      }),
    ).toBeInTheDocument();
  });

  it("browses AniList anime and selects it through the existing details flow", async () => {
    const user = userEvent.setup();
    mount();

    await user.click(await screen.findByRole("button", { name: "Anime" }));

    const anime = await screen.findByRole("button", { name: /play spirited away, 2001/i });
    expect(invoke).toHaveBeenCalledWith("catalog_anime", { query: null });
    expect(screen.getByAltText("Spirited Away poster")).toHaveAttribute(
      "src",
      "https://s4.anilist.co/file/anilistcdn/media/anime/cover/large/1.jpg",
    );
    expect(screen.getByText("Anime catalogue data provided by AniList."))
      .toBeInTheDocument();

    await user.click(anime);
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("catalog_wanted_add", {
        title: "Spirited Away",
        year: 2001,
        kind: "movie",
      });
    });
    expect(screen.getByTestId("route")).toHaveTextContent("/details/catalog-movie-101");
  });

  it("carries the clicked poster into the expanded view", async () => {
    const user = userEvent.setup();
    mount();

    await user.click(await screen.findByRole("button", { name: "Anime" }));
    await user.click(await screen.findByRole("button", { name: /play spirited away, 2001/i }));

    await waitFor(() => {
      expect(screen.getByTestId("route")).toHaveTextContent(
        "https://s4.anilist.co/file/anilistcdn/media/anime/cover/large/1.jpg",
      );
    });
  });

  it("searches AniList when Anime is selected", async () => {
    const user = userEvent.setup();
    mount();
    await user.click(await screen.findByRole("button", { name: "Anime" }));
    await screen.findByRole("button", { name: /play spirited away, 2001/i });

    await user.type(screen.getByRole("searchbox", { name: /search the catalog/i }), "Spirited Away");
    await user.click(screen.getByRole("button", { name: /^search$/i }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("catalog_anime", { query: "spirited away" });
    });
    expect(screen.getByRole("heading", { name: "Search results" })).toBeInTheDocument();
  });

  it("renders TMDB posters and falls back to bundled art", async () => {
    const user = userEvent.setup();
    mount();

    const arrival = await screen.findByAltText("Arrival poster");
    expect(arrival).toHaveAttribute(
      "src",
      "https://image.tmdb.org/t/p/w500/arrival-poster.jpg",
    );
    expect(arrival).toHaveAttribute("loading", "lazy");

    await user.click(screen.getByRole("button", { name: "TV" }));
    const quiet = await screen.findByAltText("The Quiet Signal poster");
    expect(quiet.getAttribute("src")).toContain("/artwork/");
    expect(quiet.getAttribute("src")).not.toContain("image.tmdb.org");
  });

  it("searches the catalog through catalog_search", async () => {
    const user = userEvent.setup();
    mount();
    await screen.findByRole("button", { name: /play arrival, 2016/i });

    await user.type(screen.getByRole("searchbox", { name: /search the catalog/i }), "Arrival");
    await user.click(screen.getByRole("button", { name: /^search$/i }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("catalog_search", { query: "arrival" });
    });
    expect(screen.getByRole("heading", { name: "Search results" })).toBeInTheDocument();
  });

  it("normalises the outgoing query and labels the displayed result", async () => {
    const user = userEvent.setup();
    mount();
    await screen.findByRole("button", { name: /play arrival, 2016/i });

    await user.type(screen.getByRole("searchbox", { name: /search the catalog/i }), "The Arrival");
    await user.click(screen.getByRole("button", { name: /^search$/i }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("catalog_search", { query: "arrival" });
    });
    expect(await screen.findByText("Showing results for arrival")).toBeInTheDocument();
    expect(document.body.textContent).not.toMatch(
      /[A-Za-z]:\\|\/home\/|dev-only-not-for-production/,
    );
  });

  it("retries once with a tolerant fallback when a search has no results", async () => {
    const user = userEvent.setup();
    mount();
    await screen.findByRole("button", { name: /play arrival, 2016/i });

    await user.type(
      screen.getByRole("searchbox", { name: /search the catalog/i }),
      "Arrival Extra",
    );
    await user.click(screen.getByRole("button", { name: /^search$/i }));

    await waitFor(() => {
      const queries = invoke.mock.calls
        .filter(([command]) => command === "catalog_search")
        .map(([, args]) => args?.query);
      expect(queries).toEqual(["arrival extra", "arrival"]);
    });
    expect(screen.getByRole("heading", { name: "Search results" })).toBeInTheDocument();
    expect(screen.getByText("Showing results for arrival")).toBeInTheDocument();
  });

  it("creates a wanted item then navigates to its details", async () => {
    const user = userEvent.setup();
    mount();

    await user.click(
      await screen.findByRole("button", { name: /play arrival, 2016/i }),
    );

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("catalog_wanted_add", {
        title: "Arrival",
        year: 2016,
        kind: "movie",
      });
    });
    expect(screen.getByTestId("route")).toHaveTextContent("/details/catalog-movie-101");
    expect(document.body.textContent).not.toMatch(
      /magnet:|[A-Za-z]:\\|\/home\/|dev-only-not-for-production/,
    );
  });

  it("clears stale error and setup state when the query is cleared", async () => {
    const user = userEvent.setup();
    // A catalog response can leave setup_required set on the search path.
    invoke = vi.fn(async (command: string) => {
      if (command === "catalog_browse") return { items: MOVIES };
      if (command === "catalog_search") return { items: [], setup_required: true };
      throw new Error(`unexpected command: ${command}`);
    });
    window.__TAURI_INTERNALS__ = { invoke } as unknown as {
      invoke: (cmd: string, args?: Record<string, unknown> | undefined) => Promise<unknown>;
    };
    mount();
    await screen.findByRole("button", { name: /play arrival, 2016/i });

    const box = screen.getByRole("searchbox", { name: /search the catalog/i });
    await user.type(box, "x");
    await user.click(screen.getByRole("button", { name: /^search$/i }));
    expect(
      await screen.findByRole("heading", { name: /catalog is not set up/i }),
    ).toBeInTheDocument();
    await user.clear(box);
    expect(screen.queryByRole("heading", { name: /catalog is not set up/i })).toBeNull();
    expect(
      screen.getByRole("button", { name: /play arrival, 2016/i }),
    ).toBeInTheDocument();

    // A failed search leaves an error, which clearing the box must also erase.
    invoke = vi.fn(async (command: string) => {
      if (command === "catalog_browse") return { items: MOVIES };
      if (command === "catalog_search") throw new Error("search failed");
      throw new Error(`unexpected command: ${command}`);
    });
    window.__TAURI_INTERNALS__ = { invoke } as unknown as {
      invoke: (cmd: string, args?: Record<string, unknown> | undefined) => Promise<unknown>;
    };
    await user.type(box, "boom");
    await user.click(screen.getByRole("button", { name: /^search$/i }));
    expect(
      await screen.findByRole("heading", { name: /catalog is unavailable/i }),
    ).toBeInTheDocument();
    await user.clear(box);
    expect(screen.queryByRole("heading", { name: /catalog is unavailable/i })).toBeNull();
  });

  it("shows a setup state when the catalog opt-in is missing", async () => {
    invoke = vi.fn(async (command: string) => {
      if (command === "catalog_browse") return { items: [], setup_required: true };
      throw new Error(`unexpected command: ${command}`);
    });
    window.__TAURI_INTERNALS__ = { invoke } as unknown as {
      invoke: (cmd: string, args?: Record<string, unknown> | undefined) => Promise<unknown>;
    };
    mount();

    expect(
      await screen.findByRole("heading", { name: /catalog is not set up/i }),
    ).toBeInTheDocument();
    expect(
      screen.queryByText(
        "This product uses the TMDB API but is not endorsed or certified by TMDB.",
      ),
    ).toBeNull();
  });

  it("suggests a corrected title for a single-word typo via one prefix retry", async () => {
    const user = userEvent.setup();
    const prefixed = [
      {
        id: "903",
        title: "Interstellar Odyssey",
        year: 2014,
        kind: "movie" as const,
        overview: "",
        posterPath: null,
      },
      {
        id: "901",
        title: "Interstellar",
        year: 2014,
        kind: "movie" as const,
        overview: "",
        posterPath: null,
      },
      {
        id: "902",
        title: "Interstellar Wars",
        year: 2016,
        kind: "movie" as const,
        overview: "",
        posterPath: null,
      },
    ];
    invoke = vi.fn(async (command: string, args?: Record<string, unknown>) => {
      if (command === "catalog_browse") return { items: MOVIES };
      if (command === "catalog_search") {
        return { items: String(args?.query) === "interstel" ? prefixed : [] };
      }
      throw new Error(`unexpected command: ${command}`);
    });
    window.__TAURI_INTERNALS__ = { invoke } as unknown as {
      invoke: (cmd: string, args?: Record<string, unknown> | undefined) => Promise<unknown>;
    };
    mount();
    await screen.findByRole("button", { name: /play arrival, 2016/i });

    await user.type(
      screen.getByRole("searchbox", { name: /search the catalog/i }),
      "interstelar",
    );
    await user.click(screen.getByRole("button", { name: /^search$/i }));

    expect(
      await screen.findByRole("button", { name: /play interstellar, 2014/i }, { timeout: 3000 }),
    ).toBeInTheDocument();
    const playButtons = screen.getAllByRole("button", { name: /^play /i });
    expect(playButtons[0]).toHaveAccessibleName("Play Interstellar, 2014");
    const queries = invoke.mock.calls
      .filter(([command]) => command === "catalog_search")
      .map(([, args]) => args?.query);
    expect(queries).toEqual(["interstelar", "interstel"]);
    expect(queries.length).toBeLessThanOrEqual(2);
    expect(screen.getByTestId("normalized-query")).toHaveTextContent(
      "Showing results for interstel",
    );
  });

  it("keeps last-good cards and shows a note when a browse retry fails", async () => {
    const user = userEvent.setup();
    let tvAttempts = 0;
    invoke = vi.fn(async (command: string, args?: Record<string, unknown>) => {
      if (command === "catalog_browse") {
        if (args?.kind === "movie") return { items: MOVIES };
        tvAttempts += 1;
        throw new Error("throttled");
      }
      throw new Error(`unexpected command: ${command}`);
    });
    window.__TAURI_INTERNALS__ = { invoke } as unknown as {
      invoke: (cmd: string, args?: Record<string, unknown> | undefined) => Promise<unknown>;
    };
    mount();
    await screen.findByRole("button", { name: /play arrival, 2016/i });

    await user.click(screen.getByRole("button", { name: "TV" }));

    expect(
      await screen.findByTestId("catalog-stale-note", undefined, { timeout: 3000 }),
    ).toHaveTextContent("Catalog temporarily unavailable — showing last loaded titles");
    expect(
      screen.getByRole("button", { name: /play arrival, 2016/i }),
    ).toBeInTheDocument();
    await waitFor(() => expect(tvAttempts).toBe(2), { timeout: 3000 });
  });

  it("recovers from the unavailable state through the Retry action", async () => {
    const user = userEvent.setup();
    let browseAttempts = 0;
    invoke = vi.fn(async (command: string) => {
      if (command === "catalog_browse") {
        browseAttempts += 1;
        // The mount attempt and its single bounded retry both fail; the
        // user-triggered attempt (third) succeeds.
        if (browseAttempts <= 2) throw new Error("throttled");
        return { items: MOVIES };
      }
      throw new Error(`unexpected command: ${command}`);
    });
    window.__TAURI_INTERNALS__ = { invoke } as unknown as {
      invoke: (cmd: string, args?: Record<string, unknown> | undefined) => Promise<unknown>;
    };
    mount();

    expect(
      await screen.findByRole("heading", { name: /catalog is unavailable/i }, { timeout: 3000 }),
    ).toBeInTheDocument();
    const retry = screen.getByRole("button", { name: "Retry" });
    expect(retry).toBeEnabled();

    await user.click(retry);

    expect(
      await screen.findByRole("button", { name: /play arrival, 2016/i }, { timeout: 3000 }),
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Retry" })).toBeNull();
    expect(browseAttempts).toBe(3);
  });

  it("does not stack retries while a retry is in flight", async () => {
    const user = userEvent.setup();
    let release: (() => void) | undefined;
    let browseAttempts = 0;
    invoke = vi.fn(async (command: string) => {
      if (command === "catalog_browse") {
        browseAttempts += 1;
        if (browseAttempts <= 2) throw new Error("throttled");
        if (browseAttempts === 3) {
          await new Promise<void>((resolve) => {
            release = resolve;
          });
        }
        return { items: MOVIES };
      }
      throw new Error(`unexpected command: ${command}`);
    });
    window.__TAURI_INTERNALS__ = { invoke } as unknown as {
      invoke: (cmd: string, args?: Record<string, unknown> | undefined) => Promise<unknown>;
    };
    mount();

    const retry = await screen.findByRole("button", { name: "Retry" }, { timeout: 3000 });
    await user.click(retry);

    await waitFor(() => expect(release).toBeTypeOf("function"));
    expect(screen.getByRole("button", { name: "Retry" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Retry" }));
    expect(browseAttempts).toBe(3);

    release?.();
    expect(
      await screen.findByRole("button", { name: /play arrival, 2016/i }, { timeout: 3000 }),
    ).toBeInTheDocument();
  });
});
