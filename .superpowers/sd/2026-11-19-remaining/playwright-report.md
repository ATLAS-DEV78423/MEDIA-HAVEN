# Playwright UI smoke check — 2026-11-19

## Status

**NO RUNNER INSTALLED → smoke NOT RUN.** No browser automation was executed.
Playwright plugin/config state: **untouched / active** (not disabled, removed,
or modified; `vite.config.ts` untouched).

## Task

Inventory Playwright tooling; if a runner exists, run a minimal smoke against
`http://127.0.0.1:5173`; if not, document the procedure and report NOT RUN
honestly. Constraints: no install, no host config changes, no Docker, no
commit, no production-source edits.

## Commands run and exact output

1. `Get-Command playwright -ErrorAction SilentlyContinue | Format-List *`
   → (no output; command not found) `---EXIT:---`

2. `npx --no-install playwright --version 2>&1`
   ```
   npm error npx canceled due to missing packages and no YES option: ["playwright@1.65.1"]
   npm error A complete log of this run can be found in: C:\Users\Stanley\AppData\Local\npm-cache\_logs\2026-11-19T16_39_39_188Z-debug-0.log
   ---EXIT:1---
   ```
   (Interpretation: npx would have to fetch `playwright@1.65.1`; it is not locally
   resolvable. Per constraints, nothing was installed.)

3. Presence checks (exact output):
   ```
   MISSING: apps/desktop/node_modules/@playwright/test
   MISSING: apps/desktop/node_modules/playwright
   MISSING: node_modules/@playwright/test
   MISSING: node_modules/playwright
   ```
   Global npm root `C:\Users\Stanley\AppData\Roaming\npm\node_modules` — no
   playwright entry (`GLOBAL ROOT: ...` then `---done---`, no `GLOBAL FOUND` line).

4. Glob `**/playwright.config.*` → `No files found`.
   Glob `tests/**/*` → only `tests/test-media/README.md`, `tests/README.md`,
   `tests/dependency_graph.sh` (no `tests/e2e/`, no spec).

5. `apps/desktop/package.json` devDependencies inspected: contains vite, vitest,
   jsdom, testing-library, tauri CLI — **no** `@playwright/test` or `playwright`.
   Scripts: `dev: vite`. No Playwright script.

## What ran vs NOT RUN

- RAN: read-only tooling inventory (above) + source inspection of `App.tsx`,
  `Nav.tsx`, `Shelf.tsx`, `errors.ts`, `AppState` wiring, and grep of accessible
  labels to ground the smoke procedure.
- NOT RUN: dev server start, `npm run dev`, any browser/Playwright test, console
  error capture, offline-panel observation. None of these were possible without
  a runner, and starting/installing is out of scope for this task.
- NOT CREATED: `tests/e2e/smoke.spec.ts` — the task permits this **only if** a
  runner exists. It does not, so no spec was written and no browser was launched.

## Offline panel note

Not observed (no browser run). From source, `App.tsx` renders a `ServiceDown`
gate when `status === "error" || !data`: heading "The library service is not
answering", explanatory body, "Expected at {apiBase}", and a Retry button;
`apiBase` defaults to `http://127.0.0.1:8080` (`services/errors.ts`) unless
`VITE_MEDIAHAVEN_API` is set. With the backend down on a real browser run this
panel is expected and a failed fetch to 8080 is an anticipated console error,
not a smoke failure. Full catalogue smoke requires the backend running.

## Files written

- `tests/e2e/README.md` — smoke procedure with consent-gated install commands,
  real selectors, and all eight smoke steps.
- `.superpowers/sdd/2026-11-19-remaining/playwright-report.md` — this report.

No other files created or modified.

## Concerns / open items

- A Playwright runner must be installed (with user consent) before any smoke can
  actually run: `npm i -D @playwright/test` + `npx playwright install chromium`.
- No `playwright.config.ts` exists; one is needed (webServer → `npm run dev`,
  `baseURL: http://127.0.0.1:5173`). Must not touch `vite.config.ts`.
- The app requires a selected profile (`/profiles`) and a backend on :8080 for
  catalogue views; otherwise only the offline gate is reachable.
- `tests/README.md` still says `tests/e2e/` is "not yet created" and lists the
  e2e harness as NOT RUN — now stale regarding directory existence; left
  unchanged because edits are restricted to the two allowed files.
- Console-error threshold and expected failed-fetch noise for the offline case
  still need to be agreed when the suite is built.
