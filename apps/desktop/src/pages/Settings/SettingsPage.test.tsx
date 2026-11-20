import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { SettingsPage } from "./SettingsPage";

vi.mock("@/state/AppState", () => ({
  useAppState: () => ({
    data: {
      settings: {
        libraryRoots: [],
        playback: { preferredLanguage: "en", subtitleSize: "medium", autoplayNext: true },
        storage: { rootDisplayName: "Media", limitBytes: null },
        optionalServices: { downloadsEnabled: true, metadataProvider: null },
        diagnostics: {
          runtimeStatus: { state: "ready" },
          backendStatus: { state: "ready" },
        },
        profiles: [],
      },
      roots: { roots: [], counts: { movies: 0, shows: 0, episodes: 0 } },
    },
    activeProfile: null,
    preferences: {
      autoplayNext: true,
      skipIntro: false,
      subtitleSize: "medium",
      reduceMotion: false,
    },
    updatePreference: vi.fn(),
  }),
}));

let invoke: ReturnType<typeof vi.fn>;

beforeEach(() => {
  invoke = vi.fn((command: string, args?: Record<string, unknown>) => {
    if (command === "download_root_get") return Promise.resolve("C:\\Media\\Downloads");
    if (command === "download_root_set") return Promise.resolve(args?.path);
    return Promise.resolve(null);
  });
  window.__TAURI_INTERNALS__ = { invoke } as unknown as {
    invoke: (cmd: string, args?: Record<string, unknown> | undefined) => Promise<unknown>;
  };
});

afterEach(() => {
  delete window.__TAURI_INTERNALS__;
  vi.clearAllMocks();
});

function mount() {
  return render(
    <MemoryRouter>
      <SettingsPage />
    </MemoryRouter>,
  );
}

describe("acquisition directory setting", () => {
  it("reads the current root and saves a new one through named commands", async () => {
    const user = userEvent.setup();
    mount();

    const input = await screen.findByLabelText("Download folder");
    await waitFor(() => expect(input).toHaveValue("C:\\Media\\Downloads"));

    await user.clear(input);
    await user.type(input, "C:\\New\\Downloads");
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(invoke).toHaveBeenCalledWith("download_root_set", {
      path: "C:\\New\\Downloads",
    });
    expect(await screen.findByRole("status")).toHaveTextContent(/restart the download service/i);
  });
});
