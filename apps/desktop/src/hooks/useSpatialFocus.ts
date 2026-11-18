import { useEffect } from "react";

/* tvOS-style spatial navigation for a remote, a controller or arrow keys.
   Focus is real DOM focus, so Enter, Space and screen readers behave natively. */

const SELECTOR = '[data-focusable="true"]';

function isVisible(element: HTMLElement): boolean {
  const rect = element.getBoundingClientRect();
  if (rect.width < 1 || rect.height < 1) return false;
  const style = window.getComputedStyle(element);
  return (
    style.visibility !== "hidden" &&
    style.display !== "none" &&
    style.opacity !== "0"
  );
}

function nearest(
  items: HTMLElement[],
  active: HTMLElement,
  direction: "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown",
): HTMLElement | null {
  const from = active.getBoundingClientRect();
  const originX = from.left + from.width / 2;
  const originY = from.top + from.height / 2;
  const horizontal = direction === "ArrowLeft" || direction === "ArrowRight";

  let best: HTMLElement | null = null;
  let bestScore = Number.POSITIVE_INFINITY;

  for (const item of items) {
    if (item === active) continue;
    const rect = item.getBoundingClientRect();
    const dx = rect.left + rect.width / 2 - originX;
    const dy = rect.top + rect.height / 2 - originY;

    let primary: number;
    switch (direction) {
      case "ArrowRight":
        if (dx <= 8) continue;
        primary = dx;
        break;
      case "ArrowLeft":
        if (dx >= -8) continue;
        primary = -dx;
        break;
      case "ArrowDown":
        if (dy <= 8) continue;
        primary = dy;
        break;
      case "ArrowUp":
        if (dy >= -8) continue;
        primary = -dy;
        break;
    }

    const secondary = horizontal ? Math.abs(dy) : Math.abs(dx);
    const aligned = horizontal
      ? Math.abs(dy) < from.height * 0.55
      : Math.abs(dx) < from.width * 0.55;
    const score = primary + secondary * (aligned ? 0.35 : 2.2);

    if (score < bestScore) {
      bestScore = score;
      best = item;
    }
  }

  return best;
}

export function useSpatialFocus(): void {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.defaultPrevented) return;
      if (event.metaKey || event.ctrlKey || event.altKey) return;
      if (!event.key.startsWith("Arrow")) return;
      // The player owns the arrow keys for seeking.
      if (document.documentElement.dataset.player === "active") return;

      const target = event.target;
      if (target instanceof HTMLElement) {
        const tag = target.tagName;
        if (
          tag === "INPUT" ||
          tag === "TEXTAREA" ||
          tag === "SELECT" ||
          target.isContentEditable
        ) {
          return;
        }
      }

      const items = Array.from(
        document.querySelectorAll<HTMLElement>(SELECTOR),
      ).filter(isVisible);
      if (items.length === 0) return;

      const active =
        document.activeElement instanceof HTMLElement
          ? document.activeElement
          : null;

      if (!active || !items.includes(active)) {
        event.preventDefault();
        items[0]?.focus({ preventScroll: true });
        return;
      }

      const next = nearest(
        items,
        active,
        event.key as "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown",
      );
      if (!next) return;

      event.preventDefault();
      next.focus({ preventScroll: true });
      const reduced =
        document.documentElement.dataset.motion === "reduced" ||
        window.matchMedia("(prefers-reduced-motion: reduce)").matches;
      next.scrollIntoView({
        block: "nearest",
        inline: "nearest",
        behavior: reduced ? "auto" : "smooth",
      });
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);
}
