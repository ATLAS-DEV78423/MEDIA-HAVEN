import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import type { MediaSummary } from "@/types/media";
import { MediaCard } from "@/components/cards/MediaCard";
import { MediaShelf } from "@/components/shelves/MediaShelf";
import { FocusManager, useOverlay } from "@/components/navigation/FocusManager";

function media(id: string, title: string): MediaSummary {
  return {
    id,
    kind: "movie",
    title,
    year: 2024,
    synopsis: null,
    runtimeMinutes: 100,
    genres: ["Drama"],
    artwork: [],
    availability: { state: "available" },
    progress: null,
  };
}

function Harness() {
  const [overlayOpen, setOverlayOpen] = useState(false);

  return (
    <FocusManager>
      <MediaShelf title="First">
        {[0, 1, 2].map((i) => (
          <MediaCard
            key={i}
            media={media(`a${i}`, `First ${i}`)}
            onOpen={() => setOverlayOpen(true)}
          />
        ))}
      </MediaShelf>
      <MediaShelf title="Second">
        {[0, 1].map((i) => (
          <MediaCard
            key={i}
            media={media(`b${i}`, `Second ${i}`)}
            onOpen={() => setOverlayOpen(true)}
          />
        ))}
      </MediaShelf>
      {overlayOpen ? <Overlay onClose={() => setOverlayOpen(false)} /> : null}
    </FocusManager>
  );
}

function Overlay({ onClose }: { readonly onClose: () => void }) {
  useOverlay(onClose);
  return (
    <div role="dialog" aria-label="Test overlay">
      <button type="button" onClick={onClose}>
        Close
      </button>
    </div>
  );
}

describe("shelf focus navigation", () => {
  it("moves right and left within a shelf", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    const first = screen.getByRole("button", { name: /First 0/ });
    first.focus();
    expect(document.activeElement).toBe(first);

    await user.keyboard("{ArrowRight}");
    expect(document.activeElement).toBe(
      screen.getByRole("button", { name: /First 1/ }),
    );

    await user.keyboard("{ArrowLeft}");
    expect(document.activeElement).toBe(
      screen.getByRole("button", { name: /First 0/ }),
    );
  });

  it("stays put at the left edge instead of jumping away", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    const first = screen.getByRole("button", { name: /First 0/ });
    first.focus();
    await user.keyboard("{ArrowLeft}");

    expect(document.activeElement).toBe(first);
  });

  it("stays put at the right edge", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    const last = screen.getByRole("button", { name: /First 2/ });
    last.focus();
    await user.keyboard("{ArrowRight}");

    expect(document.activeElement).toBe(last);
  });

  it("moves down to the next shelf preserving the column", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    screen.getByRole("button", { name: /First 1/ }).focus();
    await user.keyboard("{ArrowDown}");

    // Second shelf has two cards, so column 1 exists there.
    expect(document.activeElement).toBe(
      screen.getByRole("button", { name: /Second 1/ }),
    );
  });

  it("clamps to the last card when the target shelf is shorter", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    // Column 2 does not exist in the second shelf, which has two cards.
    screen.getByRole("button", { name: /First 2/ }).focus();
    await user.keyboard("{ArrowDown}");

    expect(document.activeElement).toBe(
      screen.getByRole("button", { name: /Second 1/ }),
    );
  });

  it("moves back up to the previous shelf", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    screen.getByRole("button", { name: /Second 0/ }).focus();
    await user.keyboard("{ArrowUp}");

    expect(document.activeElement).toBe(
      screen.getByRole("button", { name: /First 0/ }),
    );
  });

  it("does not move focus when there is no adjacent shelf", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    const last = screen.getByRole("button", { name: /Second 1/ });
    last.focus();
    await user.keyboard("{ArrowDown}");

    expect(document.activeElement).toBe(last);
  });
});

describe("overlay focus restoration", () => {
  it("restores focus to the card that opened the overlay", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    const opener = screen.getByRole("button", { name: /First 1/ });
    opener.focus();
    await user.click(opener);

    expect(screen.getByRole("dialog", { name: "Test overlay" })).toBeInTheDocument();

    await user.keyboard("{Escape}");
    expect(document.activeElement).toBe(opener);
  });
});
