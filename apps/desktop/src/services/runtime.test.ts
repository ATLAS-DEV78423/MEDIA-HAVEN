import { afterEach, describe, expect, it, vi } from "vitest";

import { startRuntime } from "./runtime";

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
  vi.unstubAllEnvs();
  vi.clearAllMocks();
});

describe("startRuntime", () => {
  it("returns the fixed dev status without calling the host", async () => {
    vi.stubEnv("DEV", true);
    const invoke = installInvoke(async () => "ready");

    await expect(startRuntime()).resolves.toBe("dev");
    expect(invoke).not.toHaveBeenCalled();
  });

  it("invokes the host-only runtime_start command outside dev", async () => {
    vi.stubEnv("DEV", false);
    const invoke = installInvoke(async () => "ready");

    await expect(startRuntime()).resolves.toBe("ready");
    expect(invoke).toHaveBeenCalledWith("runtime_start", {});
  });
});
