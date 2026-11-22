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

  it("uses a separate session-only shim for each provider kind in browser dev", async () => {
    await expect(hasProviderSecret("tmdb")).resolves.toBe(false);
    await expect(hasProviderSecret("prowlarr_url")).resolves.toBe(false);
    await expect(hasProviderSecret("prowlarr_key")).resolves.toBe(false);

    await setProviderSecret("tmdb", "dev-tmdb");
    await setProviderSecret("prowlarr_url", "dev-url");
    await setProviderSecret("prowlarr_key", "dev-key");

    await expect(hasProviderSecret("tmdb")).resolves.toBe(true);
    await expect(hasProviderSecret("prowlarr_url")).resolves.toBe(true);
    await expect(hasProviderSecret("prowlarr_key")).resolves.toBe(true);

    await clearProviderSecret("prowlarr_url");
    await expect(hasProviderSecret("prowlarr_url")).resolves.toBe(false);
    await expect(hasProviderSecret("prowlarr_key")).resolves.toBe(true);
  });

  it("routes all three kinds through named commands and never through storage", async () => {
    const invoke = vi.fn(async (command: string) =>
      command === "provider_secret_has" ? true : null,
    );
    window.__TAURI_INTERNALS__ = { invoke } as unknown as {
      invoke: (cmd: string, args?: Record<string, unknown> | undefined) => Promise<unknown>;
    };

    for (const [kind, value] of [
      ["tmdb", "host-token"],
      ["prowlarr_url", "host-url"],
      ["prowlarr_key", "host-key"],
    ] as const) {
      await setProviderSecret(kind, value);
      await expect(hasProviderSecret(kind)).resolves.toBe(true);
      await clearProviderSecret(kind);

      expect(invoke).toHaveBeenCalledWith("provider_secret_set", { kind, value });
      expect(invoke).toHaveBeenCalledWith("provider_secret_has", { kind });
      expect(invoke).toHaveBeenCalledWith("provider_secret_clear", { kind });
    }
    expect(sessionStorage.length).toBe(0);
  });

  it("rejects any kind outside the three-name allowlist", async () => {
    for (const kind of ["imdb", "prowlarr", "TMDB"]) {
      await expect(setProviderSecret(kind, "x")).rejects.toThrow("unsupported provider");
      await expect(hasProviderSecret(kind)).rejects.toThrow("unsupported provider");
      await expect(clearProviderSecret(kind)).rejects.toThrow("unsupported provider");
    }
  });
});
