import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { PlaybackSession } from "@/services/types";
import { PlayerPage } from "./PlayerPage";

vi.mock("@/state/AppState", () => ({
  useAppState: () => ({
    saveProgress: vi.fn(async () => {}),
    refresh: vi.fn(async () => {}),
    preferences: { skipIntro: false },
  }),
}));
vi.mock("@/data/catalog", () => ({ artFor: () => null }));

const session: PlaybackSession = {
  sessionId: "s1",
  media: {
    id: "m1",
    kind: "movie",
    title: "First Film",
    year: null,
    synopsis: null,
    runtimeMinutes: null,
    genres: [],
    artwork: [],
    availability: { state: "available" },
    progress: null,
  },
  startPositionSeconds: 10,
  durationSeconds: 3600,
  subtitleTracks: [],
  audioTracks: [],
  canResume: false,
};

let invoke: ReturnType<typeof vi.fn>;
let emit: (payload: unknown) => void;
let observe: ReturnType<typeof vi.fn>;
let disconnect: ReturnType<typeof vi.fn>;
let resizeCallback: ResizeObserverCallback;

beforeEach(() => {
  emit = () => {};
  invoke = vi.fn(async (command: string) => {
    if (command === "playback_start") return session;
    return null; // player_control + event listen/unlisten
  });
  // Structurally richer than transport.ts's declaration; assigning via a
  // variable avoids an excess-property error while exercising the real bridge.
  const internals = {
    invoke,
    transformCallback: (cb: (payload: unknown) => void) => {
      emit = cb;
      return 3;
    },
    unregisterCallback: () => {},
  };
  window.__TAURI_INTERNALS__ = internals;

  observe = vi.fn();
  disconnect = vi.fn();
  class ResizeObserverMock {
    constructor(callback: ResizeObserverCallback) {
      resizeCallback = callback;
    }
    observe = observe;
    unobserve = vi.fn();
    disconnect = disconnect;
  }
  vi.stubGlobal("ResizeObserver", ResizeObserverMock);
});

afterEach(() => {
  delete window.__TAURI_INTERNALS__;
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

function mount() {
  return render(
    <MemoryRouter initialEntries={["/player/m1"]}>
      <Routes>
        <Route path="/player/:mediaId" element={<PlayerPage />} />
      </Routes>
    </MemoryRouter>,
  );
}

describe("player driven by the engine", () => {
  it("follows engine timePos and renders no filesystem path", async () => {
    mount();
    await screen.findByText("First Film");

    act(() => {
      emit({ timePos: 321, pause: false, volume: 80, mediaId: "m1" });
    });

    await waitFor(() => expect(screen.getByText("5:21")).toBeInTheDocument());
    expect(document.body.textContent).not.toMatch(/[A-Za-z]:\\|\/media\/root/);
  });

  it("routes seek and pause through the named player_control command", async () => {
    const user = userEvent.setup();
    mount();
    await screen.findByText("First Film");
    act(() => {
      emit({ timePos: 321, pause: false, volume: 80, mediaId: "m1" });
    });
    await waitFor(() => expect(screen.getByText("5:21")).toBeInTheDocument());

    await user.click(screen.getByRole("button", { name: /forward 15 seconds/i }));
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("player_control", {
        action: "seek",
        value: 336,
      });
    });

    await user.click(screen.getByRole("button", { name: /^pause$/i }));
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("player_control", {
        action: "pause",
        value: null,
      });
    });
  });
});

describe("video surface attachment", () => {
  it("sends only four numbers and no path or token", async () => {
    mount();
    await screen.findByText("First Film");

    const commands = invoke.mock.calls.map(([command]) => command);
    const attachIndex = commands.indexOf("player_attach_surface");
    expect(attachIndex).toBeGreaterThanOrEqual(0);

    const attachCall = invoke.mock.calls[attachIndex]!;
    const rect = attachCall[1] as Record<string, unknown>;
    expect(Object.keys(rect).sort()).toEqual(["height", "width", "x", "y"]);
    for (const value of Object.values(rect)) {
      expect(typeof value).toBe("number");
    }
    const serialised = JSON.stringify(attachCall);
    expect(serialised.toLowerCase()).not.toContain("path");
    expect(serialised.toLowerCase()).not.toContain("token");
  });

  it("attaches before starting playback on first mount", async () => {
    mount();
    await screen.findByText("First Film");

    const commands = invoke.mock.calls.map(([command]) => command);
    expect(commands.indexOf("player_attach_surface")).toBeLessThan(
      commands.indexOf("playback_start"),
    );
  });
});

describe("player teardown and layout observation", () => {
  it("stops playback on unmount before saving", async () => {
    const { unmount } = mount();
    await screen.findByText("First Film");

    unmount();

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("player_control", {
        action: "stop",
        value: null,
      });
    });
  });

  it("does not stop playback when the session loads — only on real unmount", async () => {
    const { unmount } = mount();
    await screen.findByText("First Film");
    // Flush passive effects from the null→session transition, where `save`
    // changes identity. The old cleanup dispatched `stop` here, killing mpv
    // immediately after it started.
    await act(async () => {});

    expect(invoke).not.toHaveBeenCalledWith("player_control", {
      action: "stop",
      value: null,
    });

    unmount();
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("player_control", {
        action: "stop",
        value: null,
      });
    });
  });

  it("uses a ResizeObserver on the player root and reports the surface", async () => {
    mount();
    await screen.findByText("First Film");

    expect(observe).toHaveBeenCalledTimes(1);
    const root = observe.mock.calls[0]![0] as HTMLElement;
    expect(root).toBeInstanceOf(HTMLElement);

    // A layout-only change (no window resize) must recompute the surface.
    invoke.mockClear();
    act(() => {
      resizeCallback([], {} as ResizeObserver);
    });
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "player_attach_surface",
        expect.objectContaining({ x: 0 }),
      );
    });
  });
});
