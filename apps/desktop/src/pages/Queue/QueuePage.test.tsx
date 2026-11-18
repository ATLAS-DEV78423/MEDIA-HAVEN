import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { type QueueEntry, type QueueEntryState } from "@/services/api";
import type { MediaSummary } from "@/services/types";
import { QueuePage } from "./QueuePage";

vi.mock("@/state/AppState", () => ({
  useAppState: () => ({
    data: {
      movies: [media("m1", "First Film"), media("m2", "Second Film")],
      shows: [],
    },
  }),
}));

function media(id: string, title: string): MediaSummary {
  return {
    id,
    title,
    kind: "movie",
    year: null,
    synopsis: null,
    runtimeMinutes: null,
    genres: [],
    artwork: [],
    availability: { state: "available" },
    progress: null,
  };
}

function entry(
  id: string,
  mediaId: string,
  title: string,
  position: number,
  state: QueueEntryState = "pending",
): QueueEntry {
  return { id, media: media(mediaId, title), position, state };
}

const TITLES: Record<string, string> = { m1: "First Film", m2: "Second Film" };

let stored: QueueEntry[] = [];
let invoke: ReturnType<typeof vi.fn>;

function installTransport() {
  invoke = vi.fn(async (command: string, args?: Record<string, unknown>) => {
    switch (command) {
      case "queue_list":
        return { entries: stored.map((item) => ({ ...item })) };
      case "queue_add": {
        const mediaId = args?.media_id as string;
        // Duplicate suppression belongs to the backend, never the UI.
        if (!stored.some((item) => item.media.id === mediaId)) {
          stored.push({
            id: `q-${stored.length + 1}`,
            media: media(mediaId, TITLES[mediaId] ?? mediaId),
            position: stored.length,
            state: "pending",
          });
        }
        return null;
      }
      case "queue_remove": {
        const queueId = args?.queue_id as string;
        stored = stored.filter((item) => item.id !== queueId);
        return null;
      }
      case "queue_reorder": {
        const orderedIds = args?.ordered_ids as string[];
        stored = orderedIds.map((id) => {
          const found = stored.find((item) => item.id === id);
          if (!found) throw new Error(`unknown queue id: ${id}`);
          return found;
        });
        return null;
      }
      default:
        throw new Error(`unexpected command: ${command}`);
    }
  });
  window.__TAURI_INTERNALS__ = { invoke };
}

function mount() {
  return render(
    <MemoryRouter>
      <QueuePage />
    </MemoryRouter>,
  );
}

/** Queue row titles only — the add-control <option> labels share these strings. */
function listedTitles(): string[] {
  const list = screen.getByRole("list");
  return within(list)
    .getAllByText(/Film$/)
    .map((node) => node.textContent ?? "");
}

beforeEach(() => {
  stored = [];
  installTransport();
});

afterEach(() => {
  delete window.__TAURI_INTERNALS__;
  vi.clearAllMocks();
});

describe("queue screen", () => {
  it("renders entries in the order returned by the backend", async () => {
    stored = [
      entry("q1", "m1", "First Film", 0),
      entry("q2", "m2", "Second Film", 1, "active"),
    ];
    mount();

    await screen.findByRole("list");
    expect(listedTitles()).toEqual(["First Film", "Second Film"]);
    expect(screen.getByText(/Playing/)).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("queue_list", {});
  });

  it("shows an empty state and honest no-download copy", async () => {
    mount();

    expect(
      await screen.findByRole("heading", { name: /your queue is empty/i }),
    ).toBeInTheDocument();
    expect(screen.getByText(/nothing is downloading/i)).toBeInTheDocument();
    expect(screen.getByText(/never fetches a file/i)).toBeInTheDocument();
  });

  it("adds through the named command without de-duplicating in the UI", async () => {
    const user = userEvent.setup();
    mount();
    await screen.findByRole("heading", { name: /up next/i });

    await user.selectOptions(screen.getByLabelText(/add a title/i), "m1");
    await user.click(screen.getByRole("button", { name: /add to queue/i }));

    await screen.findByRole("list");
    expect(listedTitles()).toEqual(["First Film"]);
    expect(invoke).toHaveBeenCalledWith("queue_add", { media_id: "m1" });

    // The same title stays in the picker; the UI still forwards the add and
    // the backend (faked here) is the one that keeps a single entry.
    await user.click(screen.getByRole("button", { name: /add to queue/i }));
    await waitFor(() => {
      expect(invoke.mock.calls.filter(([cmd]) => cmd === "queue_add")).toHaveLength(2);
    });
    expect(listedTitles()).toEqual(["First Film"]);
  });

  it("reorders via queue_reorder and re-renders the new order", async () => {
    const user = userEvent.setup();
    stored = [
      entry("q1", "m1", "First Film", 0),
      entry("q2", "m2", "Second Film", 1),
    ];
    mount();
    await screen.findByRole("list");

    await user.click(
      screen.getByRole("button", { name: /move second film up/i }),
    );

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("queue_reorder", {
        ordered_ids: ["q2", "q1"],
      });
    });
    await waitFor(() => {
      expect(listedTitles()).toEqual(["Second Film", "First Film"]);
    });
  });

  it("removes only after confirmation, using the queue id", async () => {
    const user = userEvent.setup();
    stored = [entry("q1", "m1", "First Film", 0)];
    mount();
    await screen.findByRole("list");

    await user.click(
      screen.getByRole("button", { name: /remove first film from queue/i }),
    );
    expect(screen.getByText(/remove this entry/i)).toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith("queue_remove", expect.anything());

    await user.click(screen.getByRole("button", { name: /^remove$/i }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("queue_remove", { queue_id: "q1" });
    });
    expect(
      await screen.findByRole("heading", { name: /your queue is empty/i }),
    ).toBeInTheDocument();
  });
});
