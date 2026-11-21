import { describe, it, expect, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { MemoryRouter, Routes, Route, useLocation } from "react-router-dom";
import { useSpatialFocus } from "./useSpatialFocus";
import { useBackNavigation } from "./useBackNavigation";

function FocusHarness() {
  useSpatialFocus();
  return (
    <div>
      <input aria-label="setup path" defaultValue="D:\\Media" />
      <button data-focusable="true">Alpha</button>
      <button data-focusable="true">Beta</button>
    </div>
  );
}

function LocationLabel() {
  const loc = useLocation();
  return <div data-testid="loc">{loc.pathname}</div>;
}

function BackHarness() {
  useBackNavigation();
  return <LocationLabel />;
}

describe("keyboard navigation", () => {
  beforeEach(() => {
    document.documentElement.removeAttribute("data-player");
    window.history.replaceState(null, "", "/");
    // jsdom gives every element a 0x0 rect, which the spatial hook treats as
    // invisible. Give elements realistic geometry so navigation can be tested.
    const pos = new WeakMap<HTMLElement, number>();
    let count = 0;
    HTMLElement.prototype.getBoundingClientRect = function () {
      if (!pos.has(this)) pos.set(this, count++ * 100);
      const y = pos.get(this)!;
      return {
        x: 10, y, width: 100, height: 40,
        top: y, left: 10, right: 110, bottom: y + 40,
        toJSON: () => ({}),
      } as DOMRect;
    };
  });

  it("ArrowDown from a focused text input moves focus to a focusable element", () => {
    render(<FocusHarness />);
    const input = screen.getByLabelText("setup path") as HTMLInputElement;
    input.focus();
    expect(document.activeElement).toBe(input);

    act(() => {
      input.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }));
    });

    expect(document.activeElement).not.toBe(input);
    expect(document.activeElement?.getAttribute("data-focusable")).toBe("true");
  });

  it("ArrowLeft inside a text input keeps native caret behavior", () => {
    render(<FocusHarness />);
    const input = screen.getByLabelText("setup path") as HTMLInputElement;
    input.focus();
    const before = document.activeElement;
    act(() => {
      input.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowLeft", bubbles: true }));
    });
    expect(document.activeElement).toBe(before);
  });

  it("Escape navigates back to the previous route", () => {
    render(
      <MemoryRouter initialEntries={["/", "/movies"]} initialIndex={1}>
        <BackHarness />
        <Routes>
          <Route path="/" element={<div>home</div>} />
          <Route path="/movies" element={<div>movies</div>} />
        </Routes>
      </MemoryRouter>,
    );
    expect(screen.getByTestId("loc").textContent).toBe("/movies");

    act(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    });

    expect(screen.getByTestId("loc").textContent).toBe("/");
  });

  it("Escape does nothing when the player owns the keyboard", () => {
    document.documentElement.dataset.player = "active";
    render(
      <MemoryRouter initialEntries={["/", "/movies"]} initialIndex={1}>
        <BackHarness />
      </MemoryRouter>,
    );
    act(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    });
    expect(screen.getByTestId("loc").textContent).toBe("/movies");
  });
});
