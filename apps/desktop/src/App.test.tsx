import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { App } from "./App";
import { AppStateProvider } from "./state/AppState";

const settings = {
  libraryRoots: [],
  playback: { preferredLanguage: "en", subtitleSize: "medium", autoplayNext: true },
  storage: { rootDisplayName: "Media", limitBytes: null },
  optionalServices: { downloadsEnabled: false, metadataProvider: null },
  diagnostics: {
    runtimeStatus: { state: "unknown" },
    backendStatus: { state: "ready" },
  },
  profiles: [
    {
      id: "profile-alex",
      name: "Alex",
      avatarColor: "",
      createdAt: "2026-11-01T00:00:00Z",
    },
  ],
};

const heroMedia = {
  id: "s-quiet",
  kind: "series",
  title: "The Quiet Earth",
  year: 2026,
  synopsis: "A scientist traces a transmission that should not exist.",
  runtimeMinutes: null,
  genres: ["sci-fi"],
  artwork: [],
  availability: { state: "available" },
  progress: null,
};

vi.mock("@/services/api", () => ({
  apiBase: "http://127.0.0.1:8080",
  describeError: () => "The library service is not answering.",
  api: {
    ready: vi.fn(async () => ({ status: "ready" })),
    home: vi.fn(async () => ({
      hero: {
        media: heroMedia,
        headline: "The Quiet Earth",
        actions: [{ kind: "play", label: "Play" }],
      },
      shelves: [
        {
          id: "recently-added-movies",
          title: "Recently Added Movies",
          items: [],
        },
      ],
    })),
    movies: vi.fn(async () => []),
    shows: vi.fn(async () => []),
    watchlist: vi.fn(async () => ({ entries: [] })),
    roots: vi.fn(async () => ({
      roots: [],
      counts: { movies: 0, shows: 0, episodes: 0 },
    })),
    settings: vi.fn(async () => settings),
    downloads: vi.fn(async () => ({
      enabled: false,
      tasks: [],
      storageUsedBytes: 0,
      storageLimitBytes: null,
    })),
  },
}));

function mount(route = "/") {
  return render(
    <AppStateProvider>
      <MemoryRouter initialEntries={[route]}>
        <App />
      </MemoryRouter>
    </AppStateProvider>,
  );
}

beforeEach(() => {
  window.localStorage.clear();
});

describe("app shell", () => {
  it("opens on profile selection when no profile is active", async () => {
    mount();

    expect(
      await screen.findByRole("heading", { name: /who's watching/i }),
    ).toBeInTheDocument();
  });

  it("enters the library after a profile is chosen", async () => {
    const user = userEvent.setup();
    mount();

    await user.click(
      await screen.findByRole("button", { name: /watch as alex/i }),
    );

    expect(
      await screen.findByRole("heading", { name: "The Quiet Earth" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "Primary" })).toBeInTheDocument();
  });
});
