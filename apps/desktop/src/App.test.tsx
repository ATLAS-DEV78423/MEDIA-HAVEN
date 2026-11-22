import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { App } from "./App";
import { api } from "@/services/api";
import { startRuntime } from "@/services/runtime";
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

vi.mock("@/services/runtime", () => ({
  startRuntime: vi.fn(async () => "ready"),
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

afterEach(() => {
  vi.useRealTimers();
});

const notListening = () =>
  Object.assign(new Error("not listening"), { code: "backend_unreachable" });

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

  it("retries a starting backend until it answers, then opens the app", async () => {
    vi.useFakeTimers();
    const ready = vi.mocked(api.ready);
    ready.mockReset();
    ready
      .mockRejectedValueOnce(notListening())
      .mockRejectedValueOnce(notListening())
      .mockResolvedValueOnce({ status: "ready" });

    mount();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(ready).toHaveBeenCalledTimes(1);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    expect(ready).toHaveBeenCalledTimes(2);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    expect(ready).toHaveBeenCalledTimes(3);
    expect(
      screen.getByRole("heading", { name: /who's watching/i }),
    ).toBeInTheDocument();
  });

  it("stops immediately on a configuration error and shows fixed setup text", async () => {
    const ready = vi.mocked(api.ready);
    ready.mockReset();
    ready.mockRejectedValueOnce(
      Object.assign(new Error("config"), { code: "credential_mismatch" }),
    );
    vi.mocked(startRuntime).mockResolvedValueOnce("setup-required");

    mount();

    expect(
      await screen.findByText(/Setup is required: install WSL2 and Docker first\./),
    ).toBeInTheDocument();
    expect(ready).toHaveBeenCalledTimes(1);
    const dom = document.body.innerHTML;
    expect(dom).not.toContain("MEDIA_HAVEN");
    expect(dom).not.toContain("compose.yaml");
  });
});
