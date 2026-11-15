/**
 * FocusManager.
 *
 * One focus owner for the whole application. It implements the focus model
 * in docs/design/navigation-spec.md:
 *
 *  - Exactly one focused element at any time.
 *  - Left/Right move inside a shelf; at the edge, focus stays put rather than
 *    jumping to an unrelated control.
 *  - Up/Down move between shelves preserving the nearest column, clamping
 *    when the target shelf is shorter.
 *  - Escape closes the topmost overlay first; only then does Back navigate.
 *  - Closing an overlay restores focus to the exact element that opened it.
 *  - Nothing traps focus.
 *
 * Cards are real <button> elements, so Enter and Space work natively and
 * assistive technology gets correct semantics for free.
 */

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";

export const SHELF_ITEM_SELECTOR = "[data-shelf-item]";

interface OverlayEntry {
  readonly id: number;
  readonly close: () => void;
}

export interface FocusManagerValue {
  /** Registers a close handler for an overlay; returns its unregister fn. */
  pushOverlay: (close: () => void) => () => void;
  /** Number of open overlays. */
  overlayDepth: number;
  /** The last input modality, so components can show the ring deliberately. */
  lastInput: "keyboard" | "pointer";
  /** Tell the manager how the user is navigating. */
  notifyInput: (mode: "keyboard" | "pointer") => void;
}

const FocusContext = createContext<FocusManagerValue | null>(null);

export function useFocusManager(): FocusManagerValue {
  const value = useContext(FocusContext);
  if (!value) {
    throw new Error("useFocusManager must be used inside <FocusManager>.");
  }
  return value;
}

/** The element that had focus before an overlay opened. */
const restoreTargets = new WeakMap<HTMLElement, HTMLElement | null>();

export function useOverlay(close: () => void, active = true): void {
  const { pushOverlay } = useFocusManager();
  const closeRef = useRef(close);
  useEffect(() => {
    closeRef.current = close;
  });

  useEffect(() => {
    if (!active) return;
    const opener = document.activeElement as HTMLElement | null;
    const unregister = pushOverlay(() => closeRef.current());
    return () => {
      unregister();
      if (opener && document.contains(opener)) {
        opener.focus();
      }
    };
  }, [active, pushOverlay]);
}

export function FocusManager({ children }: { children: ReactNode }) {
  const [lastInput, setLastInput] = useState<"keyboard" | "pointer">("keyboard");
  const overlays = useRef<OverlayEntry[]>([]);
  const [overlayDepth, setOverlayDepth] = useState(0);

  const notifyInput = useCallback((mode: "keyboard" | "pointer") => {
    setLastInput((current) => (current === mode ? current : mode));
  }, []);

  const pushOverlay = useCallback((close: () => void) => {
    const entry: OverlayEntry = { id: overlays.current.length, close };
    overlays.current = [...overlays.current, entry];
    restoreTargets.set(
      document.activeElement as HTMLElement,
      document.activeElement as HTMLElement | null,
    );
    setOverlayDepth(overlays.current.length);
    return () => {
      overlays.current = overlays.current.filter((o) => o.id !== entry.id);
      setOverlayDepth(overlays.current.length);
    };
  }, []);

  // Escape closes the topmost overlay, and only then does Back navigate.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      const top = overlays.current[overlays.current.length - 1];
      if (!top) return;
      event.preventDefault();
      event.stopPropagation();
      top.close();
    };
    document.addEventListener("keydown", onKeyDown, true);
    return () => document.removeEventListener("keydown", onKeyDown, true);
  }, []);

  // Track the input modality so a mouse click still leaves a visible ring,
  // and drive the directional shelf navigation.
  useEffect(() => {
    const onPointer = () => notifyInput("pointer");
    const onKey = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (
        target &&
        (target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.isContentEditable)
      ) {
        return;
      }

      if (
        event.key === "Tab" ||
        event.key === "ArrowUp" ||
        event.key === "ArrowDown" ||
        event.key === "ArrowLeft" ||
        event.key === "ArrowRight" ||
        event.key === "Enter" ||
        event.key === " "
      ) {
        notifyInput("keyboard");
      }

      let moved = false;
      switch (event.key) {
        case "ArrowLeft":
          moved = moveHorizontal(-1);
          break;
        case "ArrowRight":
          moved = moveHorizontal(1);
          break;
        case "ArrowUp":
          moved = moveVertical(-1);
          break;
        case "ArrowDown":
          moved = moveVertical(1);
          break;
        default:
          break;
      }
      if (moved) event.preventDefault();
    };
    document.addEventListener("pointerdown", onPointer, true);
    document.addEventListener("keydown", onKey, true);
    return () => {
      document.removeEventListener("pointerdown", onPointer, true);
      document.removeEventListener("keydown", onKey, true);
    };
  }, [notifyInput]);

  const value = useMemo<FocusManagerValue>(
    () => ({
      pushOverlay,
      overlayDepth,
      lastInput,
      notifyInput,
    }),
    [pushOverlay, overlayDepth, lastInput, notifyInput],
  );

  return <FocusContext.Provider value={value}>{children}</FocusContext.Provider>;
}

/* ------------------------------------------------------------------ *
 * Shelf navigation
 * ------------------------------------------------------------------ */

function itemsOf(shelf: HTMLElement): HTMLElement[] {
  return Array.from(shelf.querySelectorAll<HTMLElement>(SHELF_ITEM_SELECTOR));
}

function shelfOf(element: HTMLElement | null): HTMLElement | null {
  if (!element) return null;
  return element.closest<HTMLElement>("[data-shelf]");
}

function shelvesInOrder(): HTMLElement[] {
  return Array.from(document.querySelectorAll<HTMLElement>("[data-shelf]"));
}

function keepVisible(element: HTMLElement): void {
  element.scrollIntoView({ block: "nearest", inline: "nearest" });
}

/**
 * Moves focus horizontally inside the current shelf.
 * Returns true when focus moved.
 */
export function moveHorizontal(delta: number): boolean {
  const active = document.activeElement as HTMLElement | null;
  const shelf = shelfOf(active);
  if (!shelf || !active) return false;

  const items = itemsOf(shelf);
  const index = items.indexOf(active);
  if (index === -1) return false;

  const next = items[index + delta];
  // At the edge: stay. Never jump to an unrelated control.
  if (!next) return false;

  next.focus();
  keepVisible(next);
  return true;
}

/**
 * Moves focus between shelves, preserving the nearest column.
 * Returns true when focus moved.
 */
export function moveVertical(delta: number): boolean {
  const active = document.activeElement as HTMLElement | null;
  const currentShelf = shelfOf(active);
  if (!currentShelf || !active) return false;

  const shelves = shelvesInOrder();
  const shelfIndex = shelves.indexOf(currentShelf);
  if (shelfIndex === -1) return false;

  const targetShelf = shelves[shelfIndex + delta];
  // No adjacent shelf: stay where we are.
  if (!targetShelf) return false;

  const currentItems = itemsOf(currentShelf);
  const column = Math.max(0, currentItems.indexOf(active));

  const targetItems = itemsOf(targetShelf);
  if (targetItems.length === 0) return false;

  // Preserve the column; clamp when the target shelf is shorter.
  const target = targetItems[Math.min(column, targetItems.length - 1)];
  if (!target) return false;

  target.focus();
  keepVisible(target);
  return true;
}

/** Moves to the first or last item in the current shelf. */
export function moveToEdge(edge: "first" | "last"): boolean {
  const active = document.activeElement as HTMLElement | null;
  const shelf = shelfOf(active);
  if (!shelf) return false;
  const items = itemsOf(shelf);
  const target = edge === "first" ? items[0] : items[items.length - 1];
  if (!target) return false;
  target.focus();
  keepVisible(target);
  return true;
}

/**
 * A keydown handler for the application shell. Arrow keys move focus;
 * Enter and Space are left to the focused control so activation works
 * natively.
 */
export function useShellKeyHandling(): void {
  const { notifyInput } = useFocusManager();

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      // Never hijack keys typed into a field.
      if (
        target &&
        (target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.isContentEditable)
      ) {
        return;
      }

      switch (event.key) {
        case "ArrowLeft":
          if (moveHorizontal(-1)) {
            event.preventDefault();
            notifyInput("keyboard");
          }
          break;
        case "ArrowRight":
          if (moveHorizontal(1)) {
            event.preventDefault();
            notifyInput("keyboard");
          }
          break;
        case "ArrowUp":
          if (moveVertical(-1)) {
            event.preventDefault();
            notifyInput("keyboard");
          }
          break;
        case "ArrowDown":
          if (moveVertical(1)) {
            event.preventDefault();
            notifyInput("keyboard");
          }
          break;
        case "Home":
          if (event.shiftKey && moveToEdge("first")) {
            event.preventDefault();
            notifyInput("keyboard");
          }
          break;
        case "End":
          if (event.shiftKey && moveToEdge("last")) {
            event.preventDefault();
            notifyInput("keyboard");
          }
          break;
        default:
          break;
      }
    };

    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [notifyInput]);
}
