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
let tmdbConfigured: boolean;

beforeEach(() => {
  tmdbConfigured = false;
  invoke = vi.fn((command: string, args?: Record<string, unknown>) => {
    if (command === "download_root_get") return Promise.resolve("C:\\Media\\Downloads");
    if (command === "download_root_set") return Promise.resolve(args?.path);
    if (command === "provider_secret_set") {
      tmdbConfigured = true;
      return Promise.resolve(null);
    }
    if (command === "provider_secret_has") return Promise.resolve(tmdbConfigured);
    if (command === "provider_secret_clear") {
      tmdbConfigured = false;
      return Promise.resolve(null);
    }
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

describe("operator TMDB token", () => {
  it("saves through the named command, then clears the field and shows only Configured", async () => {
    const user = userEvent.setup();
    const token = "tmdb-sentinel-token";
    mount();

    const input = await screen.findByLabelText("TMDB read access token");
    expect(input).toHaveAttribute("type", "password");
    expect(await screen.findByText("Not configured")).toBeInTheDocument();

    await user.type(input, token);
    await user.click(screen.getByRole("button", { name: "Save token" }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("provider_secret_set", {
        kind: "tmdb",
        value: token,
      }),
    );
    expect(input).toHaveValue("");
    expect(await screen.findByText("Configured")).toBeInTheDocument();
    expect(screen.queryByText("Not configured")).toBeNull();
    expect(document.body.innerHTML).not.toContain(token);
  });
});
