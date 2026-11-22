import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { seedSources } from "./sources";

beforeEach(() => {
  delete window.__TAURI_INTERNALS__;
});

afterEach(() => {
  delete window.__TAURI_INTERNALS__;
  vi.clearAllMocks();
});

describe("seedSources", () => {
  it("uses a fixed dev shim with no host bridge and no secrets", async () => {
    const result = await seedSources();
    expect(result).toEqual({
      added: ["yts", "nyaasi", "thepiratebay"],
      skipped: ["eztv-api: not in Prowlarr", "animetosho: not in Prowlarr"],
    });
    expect(sessionStorage.length).toBe(0);
    expect(localStorage.length).toBe(0);
  });

  it("forwards to the named host command with no arguments", async () => {
    const invoke = vi.fn(async () => ({
      added: ["yts"],
      skipped: ["animetosho: not in Prowlarr"],
    }));
    window.__TAURI_INTERNALS__ = { invoke } as unknown as {
      invoke: (cmd: string, args?: Record<string, unknown> | undefined) => Promise<unknown>;
    };

    await expect(seedSources()).resolves.toEqual({
      added: ["yts"],
      skipped: ["animetosho: not in Prowlarr"],
    });
    expect(invoke).toHaveBeenCalledWith("indexers_seed", undefined);
  });
});
