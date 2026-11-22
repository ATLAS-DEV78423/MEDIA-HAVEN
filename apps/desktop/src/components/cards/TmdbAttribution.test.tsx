import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import {
  IMDB_POSTER_ATTRIBUTION,
  TMDB_ATTRIBUTION_FRONTEND,
  TmdbAttribution,
} from "./TmdbAttribution";

describe("TmdbAttribution", () => {
  it("exports both exact attribution lines and renders them together", () => {
    expect(TMDB_ATTRIBUTION_FRONTEND).toBe(
      "This product uses the TMDB API but is not endorsed or certified by TMDB.",
    );
    expect(IMDB_POSTER_ATTRIBUTION).toBe("Posters: IMDb");

    render(<TmdbAttribution />);

    expect(screen.getByText(TMDB_ATTRIBUTION_FRONTEND)).toBeInTheDocument();
    expect(screen.getByText(IMDB_POSTER_ATTRIBUTION)).toBeInTheDocument();
  });
});
