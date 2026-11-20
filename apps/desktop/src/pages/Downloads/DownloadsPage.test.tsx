import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { AcquisitionRow } from "@/services/types";
import { DownloadsPage } from "./DownloadsPage";

vi.mock("@/state/AppState", () => ({
  useAppState: () => ({
    data: {
      downloads: {
        enabled: true,
        tasks: [],
        storageUsedBytes: 0,
        storageLimitBytes: null,
      },
    },
  }),
}));

let row: AcquisitionRow;
let invoke: ReturnType<typeof vi.fn>;

beforeEach(() => {
  row = {
    id: "torrent-1",
    torrentHash: "hash-1",
    mediaId: null,
    state: "downloading",
    progress: 0.1,
  };
  invoke = vi.fn((command: string, args?: Record<string, unknown>) => {
    switch (command) {
      case "download_status":
        return Promise.resolve([] as AcquisitionRow[]);
      case "download_add": {
        const source = String(args?.source ?? "").trim();
        const valid =
          source.startsWith("magnet:?xt=urn:btih:") ||
          /^[^\\/:*]+\.torrent$/i.test(source);
        if (!valid) {
          throw { code: "invalid_request", message: "The request was rejected." };
        }
        // Deliberately returns the same id every time: duplicate suppression
        // must hold in the UI even if a transport does not deduplicate.
        return Promise.resolve(row);
      }
      case "download_remove":
        return Promise.resolve(null);
      default:
        return Promise.resolve(null);
    }
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
      <DownloadsPage />
    </MemoryRouter>,
  );
}

async function add(source: string) {
  const user = userEvent.setup();
  await user.click(await screen.findByRole("button", { name: /add source/i }));
  await user.type(await screen.findByLabelText(/magnet link or \.torrent/i), source);
  await user.click(screen.getByRole("button", { name: /^add$/i }));
}

describe("downloads driven by named acquisition commands", () => {
  it("adds a source through download_add and shows a real status row", async () => {
    mount();
    await add("magnet:?xt=urn:btih:AAA");

    expect(await screen.findByText("torrent-1")).toBeInTheDocument();
    expect(screen.getByText(/Downloading · 10%/)).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("download_add", {
      source: "magnet:?xt=urn:btih:AAA",
    });
    expect(screen.queryByText(/[A-Za-z]:\\|\/downloads\//)).toBeNull();
  });

  it("does not create a second row when the same source is added twice", async () => {
    mount();
    await add("magnet:?xt=urn:btih:SAME");
    await screen.findByText("torrent-1");
    await add("magnet:?xt=urn:btih:SAME");

    const list = screen.getByRole("list", { name: "Acquisitions" });
    expect(within(list).getAllByRole("listitem")).toHaveLength(1);
  });

  it("rejects an invalid source without adding a row", async () => {
    mount();
    await add("not a source");

    expect(await screen.findByRole("alert")).toHaveTextContent(/request was rejected/i);
    expect(screen.queryByText("torrent-1")).not.toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("download_add", { source: "not a source" });
  });

  it("removes a row through download_remove", async () => {
    const user = userEvent.setup();
    mount();
    await add("episode.torrent");
    await screen.findByText("torrent-1");

    await user.click(screen.getByRole("button", { name: /remove download torrent-1/i }));

    await waitFor(() => expect(screen.queryByText("torrent-1")).not.toBeInTheDocument());
    expect(screen.getByText(/no downloads yet/i)).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("download_remove", { id: "torrent-1" });
  });
});
