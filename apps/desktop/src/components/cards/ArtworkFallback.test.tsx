/* A cached TMDB poster can be evicted from the 64 MiB cache while its URL is
 * still in a DTO. The image element must degrade to bundled art, never show a
 * broken image — and must not loop if the fallback itself errors. Fakes only. */

import { fireEvent, render } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";

import { artFor, type Media } from "@/data/catalog";
import { MediaCard } from "./MediaCard";

const TMDB_POSTER = `/artwork/${"a".repeat(64)}`;

const media: Media = {
  id: "m1",
  title: "Arrival",
  year: 2016,
  kind: "movie",
  genres: ["Sci-Fi"],
  runtime: "1h 56m",
  minutes: 116,
  synopsis: "",
  about: "",
  art: { hero: TMDB_POSTER, still: TMDB_POSTER },
};

describe("artwork onError fallback", () => {
  it("swaps a failed TMDB image to a bundled path exactly once", () => {
    const { container } = render(
      <MemoryRouter>
        <MediaCard media={media} />
      </MemoryRouter>,
    );
    const image = container.querySelector("img");
    expect(image).not.toBeNull();
    expect(image!.getAttribute("src")).toBe(TMDB_POSTER);

    const bundled = artFor(media.id, media.title).still;
    fireEvent.error(image!);
    expect(image!.getAttribute("src")).toBe(bundled);
    expect(bundled).toMatch(/^\/artwork\//);

    // The dataset flag guards against an error loop: a second error is a no-op.
    fireEvent.error(image!);
    expect(image!.getAttribute("src")).toBe(bundled);
  });
});
