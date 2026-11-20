# e2e smoke procedure (Playwright)

**Current state (2026-11-19): NOT RUN — no Playwright runner is installed in
this environment.** No `@playwright/test` / `playwright` package exists in
`apps/desktop/node_modules`, the repo root `node_modules`, or the global npm
root; `Get-Command playwright` resolves nothing and `npx --no-install playwright
--version` refuses (it would have to fetch `playwright@1.65.1`, which is blocked
without installation consent). There is no `playwright.config.*` and no suite in
this directory yet. The Playwright plugin/config state is **untouched / active**
— do not disable or modify it, and do not edit `vite.config.ts`.

This folder is the intended home of the e2e harness referenced in
`../README.md` (`npx playwright test`). The procedure below is what a human
runs **once Playwright is available**. Nothing here has been executed.

## 0. Install — REQUIRES EXPLICIT CONSENT

Do not run this without the user's approval; it installs packages and a browser
binary and mutates lockfiles.

```bash
# from apps/desktop — consent required for all three lines
npm i -D @playwright/test
npx playwright install chromium
# (optional OS deps; Windows usually needs none) npx playwright install-deps chromium
```

Do **not** use `--no-install` expectations until the package is truly present;
verify first:

```bash
npx playwright --version          # expect a version, e.g. 1.65.1
```

## 1. Start the app

The React app lives in `apps/desktop` and serves on port **5173**. It gates on
the backend at `http://127.0.0.1:8080` (`VITE_MEDIAHAVEN_API` overrides this).
With no backend, it renders the offline "library service is not answering"
panel instead of the catalogue — so a full catalogue smoke needs the backend
running too.

```bash
# terminal A — backend (required for catalogue views; consent if not already set up)
MEDIAHAVEN_DEV=1 cargo run -p mediahaven-core

# terminal B — Vite dev server
cd apps/desktop
npm run dev
# wait for: "Local: http://localhost:5173/"
```

Stop the dev server when done (Ctrl+C); confirm no orphan `node` dev-server
process remains (`Get-Process node` / `Stop-Process -Id <pid>` on Windows).

## 2. Baseline config to add (only after consent + install)

A minimal `playwright.config.ts` at the repo root should point `webServer` at
`npm run dev` in `apps/desktop`, `baseURL: http://127.0.0.1:5173`, and `reuseExistingServer`
so it works with a manually started server. Do not alter `apps/desktop/vite.config.ts`.

## 3. Smoke steps (manual or as a future spec)

Target: `http://127.0.0.1:5173`. Selectors below are the real accessible hooks
found in the UI. The app redirects to `/profiles` until a profile is chosen;
pick any profile first (button `Watch as <name>`).

1. **Boot / gate** — On first load a `role="status"` "Opening the local
   library…" screen appears, then the app shell. No console errors expected.
2. **Nav rail** — `nav[aria-label="Primary"]` is visible with links Home,
   Movies, TV Shows, Library, My List, Queue. Brand link has
   `aria-label="MEDIA HAVEN home"`; search action has
   `aria-label="Search the library"`.
3. **Hero** — On Home, `aria-label="Featured titles"` is present; the dot
   tablist is `role="tablist"` with `role="tab"` items labelled
   `Show <title>`. "Open <title>" links are present.
4. **Shelves** — Home renders one or more `<section>` shelves with `<h2>`
   titles (e.g. the "My List" shelf) and media cards. Cards carry
   `aria-label="<title>, <year>. <genres>"`. Empty shelves show their empty
   message rather than broken layout.
5. **Search** — Navigate via the `Search the library` action. Input has
   `placeholder="Search titles, genres, years"` and
   `aria-label="Search your library"`; type a known title, expect results under
   the `<h2>Results</h2>`, and a `Clear search` control to reset.
6. **Watchlist** — Navigate to `/watchlist` via the "My List" tab. Empty state
   reads "Nothing saved yet. Open a title and choose Add to My List." From a
   title detail page the toggle reads "Add to My List" / "In My List"; after
   adding, the item appears here.
7. **Keyboard-only focus** — Using only Tab/Shift+Tab from page load, every
   interactive element is reachable: all `[data-focusable="true"]` nodes take
   focus and show a visible focus ring; Enter/Space activates links and buttons.
   No focus trap on the main shell. (The app ships a `useSpatialFocus` hook —
   verify arrow keys don't strand focus.)
8. **Offline panel** — Stop the backend, reload `http://127.0.0.1:5173`. Expect
   the `ServiceDown` gate: heading "The library service is not answering",
   body explaining there is no offline catalogue copy, "Expected at
   http://127.0.0.1:8080", and a **Retry** button. No invented titles are
   shown. Capture console errors; a failed fetch to the API is expected here.

## 4. Record results

Record PASS / FAIL / **NOT RUN** per step in `progress.md`. `NOT RUN` must never
be reported as passing. See `docs/testing/test-matrix.md`.
