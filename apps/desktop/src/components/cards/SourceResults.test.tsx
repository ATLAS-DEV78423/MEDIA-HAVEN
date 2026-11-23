import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { SOURCE_STATUS, SourceResults } from "./SourceResults";

const MAGNET = "magnet:?xt=urn:btih:ABC123&dn=A.Local.Film.2024";
let invoke: ReturnType<typeof vi.fn>;

beforeEach(() => {
  invoke = vi.fn(async (command: string) => {
    switch (command) {
      case "source_search":
        return {
          results: [
            {
              id: "7-0",
              title: "A.Local.Film.2024.1080p",
              size: 1048576,
              seeders: 12,
              magnet: MAGNET,
            },
          ],
        };
      case "download_add":
        return { id: "hash" };
      case "source_set":
        return null;
      default:
        throw new Error(`unexpected command: ${command}`);
    }
  });
  window.__TAURI_INTERNALS__ = { invoke };
});

afterEach(() => {
  delete window.__TAURI_INTERNALS__;
  vi.clearAllMocks();
});

function mount() {
  render(<SourceResults mediaId="m1" query={{ title: "A Local Film", year: 2024 }} />);
}

describe("SourceResults", () => {
  it("shows candidates only after Find sources is pressed", async () => {
    const user = userEvent.setup();
    mount();

    expect(screen.queryByText("A.Local.Film.2024.1080p")).toBeNull();
    await user.click(screen.getByRole("button", { name: /find sources/i }));

    expect(await screen.findByText("A.Local.Film.2024.1080p")).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith("source_search", {
      title: "A Local Film",
      year: 2024,
    });
  });

  it("sends a chosen source to download-add then source-set with fixed status", async () => {
    const user = userEvent.setup();
    mount();
    await user.click(screen.getByRole("button", { name: /find sources/i }));
    await screen.findByText("A.Local.Film.2024.1080p");

    await user.click(screen.getByRole("button", { name: /^choose$/i }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("download_add", { source: MAGNET });
      expect(invoke).toHaveBeenCalledWith("source_set", {
        mediaId: "m1",
        source: MAGNET,
      });
    });
    const calls = invoke.mock.calls.map((call) => call[0]);
    expect(calls.indexOf("download_add")).toBeLessThan(calls.indexOf("source_set"));
    expect(screen.getByRole("status")).toHaveTextContent(SOURCE_STATUS.added);
    expect(screen.getByRole("status").textContent).not.toContain(MAGNET);
  });

  it("reports a started download separately when mapping fails", async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === "source_search") {
        return {
          results: [
            {
              id: "7-0",
              title: "A.Local.Film.2024.1080p",
              size: 1048576,
              seeders: 12,
              magnet: MAGNET,
            },
          ],
        };
      }
      if (command === "download_add") return { id: "hash" };
      throw new Error("mapping rejected");
    });
    const user = userEvent.setup();
    mount();
    await user.click(screen.getByRole("button", { name: /find sources/i }));
    await screen.findByText("A.Local.Film.2024.1080p");

    await user.click(screen.getByRole("button", { name: /^choose$/i }));

    expect(await screen.findByRole("status")).toHaveTextContent(
      SOURCE_STATUS.mappingFailed,
    );
    expect(invoke).toHaveBeenCalledWith("download_add", { source: MAGNET });
    expect(invoke).toHaveBeenCalledWith("source_set", {
      mediaId: "m1",
      source: MAGNET,
    });
    expect(screen.getByRole("status").textContent).not.toContain(MAGNET);
  });

  it("uses fixed text when no candidates are found", async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === "source_search") return { results: [] };
      throw new Error(`unexpected command: ${command}`);
    });
    const user = userEvent.setup();
    mount();
    await user.click(screen.getByRole("button", { name: /find sources/i }));

    expect(await screen.findByRole("status")).toHaveTextContent(SOURCE_STATUS.empty);
  });
});
