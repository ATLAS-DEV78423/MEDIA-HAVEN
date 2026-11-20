import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { IMDB_ATTRIBUTION, ImdbRating } from "./ImdbRating";

describe("ImdbRating", () => {
  it("shows a rating and vote count with attribution", () => {
    render(<ImdbRating imdbRating={8.72} voteCount={2_500_000} />);
    const badge = screen.getByTestId("imdb-rating");
    expect(badge).toHaveTextContent("8.7");
    expect(badge).toHaveTextContent("2,500,000");
    expect(screen.getByText(IMDB_ATTRIBUTION)).toBeTruthy();
  });

  it("renders nothing when no rating or age rating is cached", () => {
    const { container } = render(
      <ImdbRating imdbRating={null} voteCount={null} ageRating={null} />,
    );
    expect(container.textContent).toBe("");
    expect(screen.queryByTestId("imdb-rating")).toBeNull();
  });

  it("shows an age rating with attribution", () => {
    render(<ImdbRating ageRating="PG-13" />);
    expect(screen.getByTestId("imdb-rating")).toHaveTextContent("PG-13");
    expect(screen.getByText(IMDB_ATTRIBUTION)).toBeTruthy();
  });

  it("never invents a value from a vote count alone", () => {
    const { container } = render(<ImdbRating voteCount={42} />);
    expect(container.textContent).toBe("");
  });
});
