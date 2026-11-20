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
});

afterEach(() => {
  delete window.__TAURI_INTERNALS__;
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
