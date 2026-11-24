import { afterEach, describe, expect, it, vi } from "vitest";
import { attachPlayerSurface, playerControl } from "./player";

afterEach(() => {
  delete window.__TAURI_INTERNALS__;
  vi.clearAllMocks();
});

describe("attachPlayerSurface", () => {
  it("sends only four numbers and no path or token", async () => {
    const invoke = vi.fn().mockResolvedValue(null);
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: { invoke },
    });
    await attachPlayerSurface({ x: 0, y: 72, width: 1280, height: 480 });
    expect(invoke).toHaveBeenCalledWith("player_attach_surface", {
      x: 0, y: 72, width: 1280, height: 480,
    });
    const serialised = JSON.stringify(invoke.mock.calls[0]);
    expect(serialised.toLowerCase()).not.toContain("path");
    expect(serialised.toLowerCase()).not.toContain("token");
  });
});

describe("playerControl", () => {
  it("routes the stop action through the named command", async () => {
    const invoke = vi.fn().mockResolvedValue(null);
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: { invoke },
    });
    await playerControl("stop");
    expect(invoke).toHaveBeenCalledWith("player_control", {
      action: "stop",
      value: null,
    });
  });
});
