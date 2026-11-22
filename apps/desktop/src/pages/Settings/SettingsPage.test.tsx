import { render, screen, waitFor, within } from "@testing-library/react";
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
let configured: Record<string, boolean>;

beforeEach(() => {
  configured = {};
  invoke = vi.fn((command: string, args?: Record<string, unknown>) => {
    if (command === "download_root_get") return Promise.resolve("C:\\Media\\Downloads");
    if (command === "download_root_set") return Promise.resolve(args?.path);
    if (command === "provider_secret_set") {
      configured[args?.kind as string] = true;
      return Promise.resolve(null);
    }
    if (command === "provider_secret_has")
      return Promise.resolve(Boolean(configured[args?.kind as string]));
    if (command === "provider_secret_clear") {
      configured[args?.kind as string] = false;
      return Promise.resolve(null);
    }
    if (command === "indexers_seed")
      return Promise.resolve({
        added: ["yts", "nyaasi", "thepiratebay"],
        skipped: ["eztv-api: not in Prowlarr", "animetosho: not in Prowlarr"],
      });
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
    const row = input.parentElement!;
    expect(await within(row).findByText("Not configured")).toBeInTheDocument();

    await user.type(input, token);
    await user.click(screen.getByRole("button", { name: "Save token" }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("provider_secret_set", {
        kind: "tmdb",
        value: token,
      }),
    );
    expect(input).toHaveValue("");
    expect(within(row).getByText("Configured")).toBeInTheDocument();
    expect(within(row).queryByText("Not configured")).toBeNull();
    expect(document.body.innerHTML).not.toContain(token);
  });
});

describe("Prowlarr provider inputs", () => {
  it("saves and clears the address without ever rendering the saved value", async () => {
    const user = userEvent.setup();
    const value = "http://127.0.0.1:9696";
    mount();

    const input = await screen.findByLabelText("Prowlarr address");
    expect(input).toHaveAttribute("type", "text");
    const row = input.parentElement!;
    expect(await within(row).findByText("Not configured")).toBeInTheDocument();

    await user.type(input, value);
    await user.click(screen.getByRole("button", { name: "Save address" }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("provider_secret_set", {
        kind: "prowlarr_url",
        value,
      }),
    );
    expect(input).toHaveValue("");
    expect(within(row).getByText("Configured")).toBeInTheDocument();
    expect(document.body.innerHTML).not.toContain(value);

    await user.click(screen.getByRole("button", { name: "Clear address" }));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("provider_secret_clear", { kind: "prowlarr_url" }),
    );
    expect(within(row).getByText("Not configured")).toBeInTheDocument();
  });

  it("saves and clears the API key as a password field", async () => {
    const user = userEvent.setup();
    const value = "prowlarr-sentinel-key";
    mount();

    const input = await screen.findByLabelText("Prowlarr API key");
    expect(input).toHaveAttribute("type", "password");
    const row = input.parentElement!;
    expect(await within(row).findByText("Not configured")).toBeInTheDocument();

    await user.type(input, value);
    await user.click(screen.getByRole("button", { name: "Save key" }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("provider_secret_set", {
        kind: "prowlarr_key",
        value,
      }),
    );
    expect(input).toHaveValue("");
    expect(within(row).getByText("Configured")).toBeInTheDocument();
    expect(document.body.innerHTML).not.toContain(value);

    await user.click(screen.getByRole("button", { name: "Clear key" }));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("provider_secret_clear", { kind: "prowlarr_key" }),
    );
    expect(within(row).getByText("Not configured")).toBeInTheDocument();
  });
});

describe("source setup", () => {
  it("reports counts only and never puts the address or key in the DOM", async () => {
    const user = userEvent.setup();
    const url = "http://127.0.0.1:9696";
    const key = "prowlarr-sentinel-key-for-seeding";
    mount();

    await user.type(await screen.findByLabelText("Prowlarr address"), url);
    await user.click(screen.getByRole("button", { name: "Save address" }));
    await user.type(await screen.findByLabelText("Prowlarr API key"), key);
    await user.click(screen.getByRole("button", { name: "Save key" }));

    await user.click(screen.getByRole("button", { name: "Set up sources" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("indexers_seed", undefined));
    expect(await screen.findByText("3 added, 2 skipped.")).toBeInTheDocument();

    const dom = document.body.innerHTML;
    expect(dom).not.toContain(url);
    expect(dom).not.toContain(key);
  });
});
