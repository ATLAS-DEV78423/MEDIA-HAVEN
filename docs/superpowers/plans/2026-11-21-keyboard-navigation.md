# Full Keyboard Navigation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or executing-plans. Steps use `- [ ]` checkboxes.

**Goal:** Make the entire UI usable with only Arrow keys, Enter and Escape — including exiting text inputs and going back a page with Escape.

**Architecture:** Extend the existing `useSpatialFocus` hook with two behaviors: (1) when a text input/textarea/contenteditable has focus and an **Up/Down arrow** is pressed (which does nothing useful in a single-line input), blur it and move spatial focus to the nearest focusable element; (2) a global **Escape** handler that navigates back via react-router when no dialog/overlay/player has handled it. Keep the player's existing arrow ownership and dialog-local Escape handlers intact.

**Tech Stack:** React 19 + react-router 7, existing `useSpatialFocus` hook, `@testing-library/react` + Vitest. No new dependency.

**Spec:** roadmap P4-03 (keyboard/remote focus navigation), `Development.md` §7.4 (TV remote).

## Global Constraints
- The player owns arrow keys — do not change `PlayerPage`'s handler (it already does Escape → navigate(-1)).
- Dialog-local Escape (profiles dialog, AddSourceDialog, search clear) must keep working; global Escape must not double-handle an already-prevented event (`event.defaultPrevented`).
- No path/token in DTOs; no commit unless instructed; Playwright/vite untouched.
- Tests are evidence: `PASS`/`FAIL`/`NOT RUN` describe observed runs.

## Task 1: Escape-to-blur for text inputs

**Files:** `apps/desktop/src/hooks/useSpatialFocus.ts`, test `apps/desktop/src/hooks/useSpatialFocus.test.tsx` (new or extend existing focus test).

- [ ] In `onKeyDown`, before the current `INPUT/TEXTAREA/SELECT` early-return: if the focused element is a single-line text input or contenteditable AND the key is ArrowUp or ArrowDown, call `element.blur()` then find the nearest focusable element from the blurred position (use the existing `nearest()` logic with all visible `data-focusable` items) and focus it. Prevent default. For ArrowLeft/ArrowRight inside a text input, keep native caret behavior (do not hijack). For SELECT, keep native behavior.
- [ ] Test: render a component with a text input + focusable buttons; focus the input; press ArrowDown; assert document.activeElement is a focusable button, not the input.

## Task 2: Global Escape-to-back

**Files:** `apps/desktop/src/hooks/useBackNavigation.ts` (new), `apps/desktop/src/App.tsx` (call the hook), test file.

- [ ] New hook: `window.addEventListener("keydown", ...)`; if `event.key === "Escape"`, `!event.defaultPrevented`, not player-active (`document.documentElement.dataset.player !== "active"`), and there is no visible `[role="dialog"][aria-modal="true"]`, then `event.preventDefault()` and `navigate(-1)` (from react-router `useNavigate`). If history has no entry, go to `/` instead (`navigate("/", { replace: true })`) — guard with `window.history.state?.idx` where available.
- [ ] Call `useBackNavigation()` in `App()` alongside `useSpatialFocus()`.
- [ ] Test: mount a small router with two routes; navigate to the second; dispatch Escape; assert URL returned to the first route. Second test: an open dialog with its own Escape handler that calls preventDefault; assert global hook does not navigate.

## Task 3: Browser verification

- [ ] With the app running (backend + Vite), drive it with Playwright using ONLY Arrow keys, Enter, Escape: profile picker → Home → Movies → Library → back to Home → Search input → ArrowDown exits input → Details → Escape back. Record what passes/fails with exact focus labels. Fix any real defect found.
- [ ] Run full gates: typecheck, lint, Vitest, tokens, build, bundle, `git diff --check`.
