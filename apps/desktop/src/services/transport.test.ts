import { afterEach, describe, expect, it, vi } from "vitest";

import { api } from "./api";
import { ApiFailure } from "./errors";

type Invoke = (
  cmd: string,
  args?: Record<string, unknown> | undefined,
) => Promise<unknown>;

function installInvoke(impl: Invoke): ReturnType<typeof vi.fn<Invoke>> {
  const invoke = vi.fn<Invoke>(impl);
  window.__TAURI_INTERNALS__ = { invoke };
  return invoke;
}

afterEach(() => {
  delete window.__TAURI_INTERNALS__;
  vi.clearAllMocks();
});

describe("renderer transport", () => {
  it("routes home through the named command and returns its value", async () => {
    const value = { hero: null, shelves: [] };
    const invoke = installInvoke(async () => value);

    await expect(api.home()).resolves.toBe(value);
    expect(invoke).toHaveBeenCalledWith("library_home", {});
  });

  it("maps episodes arguments to Rust snake_case keys", async () => {
    const invoke = installInvoke(async () => []);

    await api.episodes("s1", 2);

    expect(invoke).toHaveBeenCalledWith("library_episodes", {
      series_id: "s1",
      season_number: 2,
    });
  });

  it("uses settings_get for readiness", async () => {
    const invoke = installInvoke(async () => ({ status: "ready" }));

    await expect(api.ready()).resolves.toEqual({ status: "ready" });
    expect(invoke).toHaveBeenCalledWith("settings_get", {});
  });

  it("maps a host error into an ApiFailure with its code and message", async () => {
    installInvoke(async () => {
      throw { code: "credential_mismatch", message: "The install credentials were rejected." };
    });

    const error = await api.settings().catch((thrown: unknown) => thrown);

    expect(error).toBeInstanceOf(ApiFailure);
    expect(error).toMatchObject({
      code: "credential_mismatch",
      message: "The install credentials were rejected.",
    });
  });
});
