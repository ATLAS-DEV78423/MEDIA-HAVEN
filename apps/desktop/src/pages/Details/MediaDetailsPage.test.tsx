import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { MediaDetail } from "@/services/types";
import { MediaDetailsPage } from "./MediaDetailsPage";

vi.mock("@/state/AppState", () => ({
  useAppState: () => ({
    data: { movies: [], shows: [] },
    isListed: () => false,
    toggleList: () => undefined,
  }),
}));

const DETAIL: MediaDetail = {
  id: "m1",
  kind: "movie",
  title: "A Local Film",
  year: 2024,
  synopsis: null,
  runtimeMinutes: 100,
  genres: [],
  artwork: [],
  progress: null,
  director: null,
  cast: [],
  availability: { state: "available" },
};

let invoke: ReturnType<typeof vi.fn>;

beforeEach(() => {
  invoke = vi.fn(async (command: string) => {
    switch (command) {
      case "library_detail":
        return DETAIL;
      case "library_episodes":
        return [];
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

async function mount() {
  render(
    <MemoryRouter initialEntries={["/details/m1"]}>
      <Routes>
        <Route path="/details/:mediaId" element={<MediaDetailsPage />} />
      </Routes>
    </MemoryRouter>,
  );
  await screen.findByRole("heading", { name: "A Local Film" });
}

describe("Details source field", () => {
  it("validates before sending and saves through the named command", async () => {
    const user = userEvent.setup();
    await mount();

    const input = screen.getByLabelText(/^source$/i);
    const save = screen.getByRole("button", { name: /save source/i });

    await user.click(save);
    expect(screen.getByRole("alert")).toHaveTextContent(/enter a magnet/i);
    expect(invoke).not.toHaveBeenCalledWith("source_set", expect.anything());

    await user.type(input, "/downloads/episode.torrent");
    await user.click(save);
    expect(screen.getByRole("alert")).toHaveTextContent(/no paths/i);
    expect(invoke).not.toHaveBeenCalledWith("source_set", expect.anything());

    const magnet = "magnet:?xt=urn:btih:ABC123&dn=episode";
    await user.clear(input);
    await user.type(input, magnet);
    await user.click(save);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("source_set", {
        mediaId: "m1",
        source: magnet,
      });
    });
    expect(await screen.findByRole("status")).toHaveTextContent(/source saved/i);
    expect(document.body.textContent).not.toContain("/downloads/");
  });
});
