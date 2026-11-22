import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  clearProviderSecret,
  hasProviderSecret,
  setProviderSecret,
} from "./providerSecrets";

beforeEach(() => {
  delete window.__TAURI_INTERNALS__;
  sessionStorage.clear();
});

afterEach(() => {
  delete window.__TAURI_INTERNALS__;
  sessionStorage.clear();
  vi.clearAllMocks();
});

describe("providerSecrets", () => {
  it("uses a session-only shim in browser dev with no host bridge", async () => {
    await expect(hasProviderSecret("tmdb")).resolves.toBe(false);
    await setProviderSecret("tmdb", "dev-only-token");
    await expect(hasProviderSecret("tmdb")).resolves.toBe(true);
    await clearProviderSecret("tmdb");
    await expect(hasProviderSecret("tmdb")).resolves.toBe(false);
  });

  it("routes only tmdb through the named commands and never through storage", async () => {
    const invoke = vi.fn(async (command: string) =>
      command === "provider_secret_has" ? true : null,
    );
    window.__TAURI_INTERNALS__ = { invoke } as unknown as {
      invoke: (cmd: string, args?: Record<string, unknown> | undefined) => Promise<unknown>;
    };

    await setProviderSecret("tmdb", "host-token");
    await expect(hasProviderSecret("tmdb")).resolves.toBe(true);
    await clearProviderSecret("tmdb");

    expect(invoke).toHaveBeenCalledWith("provider_secret_set", {
      kind: "tmdb",
      value: "host-token",
    });
    expect(invoke).toHaveBeenCalledWith("provider_secret_has", { kind: "tmdb" });
    expect(invoke).toHaveBeenCalledWith("provider_secret_clear", { kind: "tmdb" });
    expect(sessionStorage.length).toBe(0);
  });

  it("rejects any provider other than tmdb", async () => {
    await expect(setProviderSecret("imdb", "x")).rejects.toThrow("unsupported provider");
    await expect(hasProviderSecret("imdb")).rejects.toThrow("unsupported provider");
  });
});
