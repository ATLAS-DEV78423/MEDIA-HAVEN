/* Display-only catalogue-card contract. Fakes only — no live TMDB call. */

import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";

import { TMDB_ATTRIBUTION_FRONTEND } from "@/components/cards/TmdbAttribution";
import { apiBase } from "@/services/errors";
import type { CatalogKind, CatalogScreen } from "@/services/types";
import { CatalogShelf } from "./CatalogShelf";

const POSTER = `/artwork/${"a".repeat(64)}`;

const withEntry: CatalogScreen = {
  available: true,
  entries: [
    {
      id: "tmdb-1",
      kind: "movie",
      title: "Real Film",
      year: 2020,
      synopsis: "A story.",
      rating: 7.2,
      artwork: [{ variant: "poster", url: POSTER, alt: "Real Film poster" }],
    },
  ],
};

function mount(catalog: CatalogScreen | null, kind: CatalogKind = "movie") {
  return render(
    <MemoryRouter>
      <CatalogShelf kind={kind} screen={catalog} />
    </MemoryRouter>,
  );
}

describe("CatalogShelf", () => {
  it("renders a display-only card with a once-prefixed poster and exact attribution", () => {
    const { container } = mount(withEntry);

    expect(screen.getByText("Real Film")).toBeInTheDocument();
    expect(screen.getByText(/2020/)).toBeInTheDocument();
    expect(screen.getByText(/7\.2/)).toBeInTheDocument();

    const image = container.querySelector("img");
    expect(image).not.toBeNull();
    expect(image!.getAttribute("src")).toBe(`${apiBase}${POSTER}`);
    expect(image!.getAttribute("src")!.startsWith(apiBase)).toBe(true);

    expect(screen.getByText(TMDB_ATTRIBUTION_FRONTEND)).toBeInTheDocument();
    // Catalogue entries are not in the local library: no detail navigation.
    expect(container.querySelector("a[href^='/details']")).toBeNull();
  });

  it("hides a failed poster instead of breaking the page", () => {
    const { container } = mount(withEntry);
    const image = container.querySelector("img")!;
    fireEvent.error(image);
    expect(image.hidden).toBe(true);
    expect(screen.getByText("Real Film")).toBeInTheDocument();
  });

  it("asks for a token and renders no cards when unavailable", () => {
    const { container } = mount({ available: false, entries: [] });

    expect(screen.getByText(/add a tmdb token in/i)).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Settings" })).toHaveAttribute(
      "href",
      "/settings",
    );
    expect(container.querySelector("img")).toBeNull();
    expect(container.querySelector("article")).toBeNull();
  });

  it("does not break on a null or empty screen", () => {
    const { unmount } = mount(null);
    expect(screen.getByRole("status")).toBeInTheDocument();
    unmount();

    const { container } = mount({ available: true, entries: [] });
    expect(screen.getByText(/no catalogue titles/i)).toBeInTheDocument();
    expect(container.querySelector("img")).toBeNull();
  });
});
