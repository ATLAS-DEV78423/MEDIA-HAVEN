# Browser QA — 2026-11-23

Target: `http://127.0.0.1:5173/` (Vite dev server) with the real backend at
`http://127.0.0.1:8080`. Browser: headless Chromium via Playwright, 1440×900.
TMDB and Prowlarr credentials were injected into the backend process only.

## Smoke test

- Title: `MEDIA_HAVEN`. Profile selection rendered.
- Console/runtime errors: one 404 for `POST /imdb_lookup`. This is a
  host-only Tauri command; the browser dev client has no route for it, so the
  Details page silently falls back. No other console errors.
- Network: all other requests 200.
- Screenshots were captured to `C:/Users/Stanley/AppData/Local/Temp/opencode/ui-*.png`
  (not committed — they are large and contain only seeded data).

## Interactions

- [✓] Pick profile "Alex" → Home loaded with real seeded data.
- [✓] Home showed posters served from `127.0.0.1:8080/artwork/<hash>` and both
  attribution lines (TMDB text and "Posters: IMDb").
- [✓] Movies tab: 5 cards, all with real poster URLs.
- [✓] Detail page for a seeded movie: synopsis, runtime, attribution, source
  field, Find sources button.
- [✓] Discover → Movies: 20 catalogue entries with poster images (Iron Man,
  The Odyssey, …); cold cache took about 25 seconds.
- [✓] Anime tab (after wiring the missing nav link) → Discover: 20 anime
  entries with posters (Pokémon, Doraemon, One Piece, …); cold cache took
  about 50 seconds.
- [✓] Setup: typed a local folder path, clicked "Scan this folder"; backend
  indexed 1 file and showed "Your library is ready".
- [✓] Clicked the new "sample" title. Play was enabled after the fix.
- [✓] Clicked Play → Player page opened with "PLAYBACK PREVIEW", 0:12 elapsed
  and a progress control. This is the browser preview; no native mpv is
  installed and the Tauri host command is not available in a browser.

## Bugs found and fixed during this run

1. Renderer sent snake_case argument names (`display_name`, `media_id`,
   `root_id`, `queue_id`, `ordered_ids`, `series_id`, `season_number`) while
   the backend uses camelCase. Setup failed with a 400 before the fix.
   Fixed in `services/api.ts` and updated the affected tests.
2. `source_search` was wrapped as `{ args: … }` but the host route expects
   the fields directly. Fixed.
3. Movie Play links were derived from episode lists, so movies could never
   play. Fixed: a movie with `availability: available` now links to
   `/player/<media id>`.
4. The Anime page existed but had no navigation tab. Added it to `Nav.tsx`.

All 59 frontend tests and all six frontend gates pass after the fixes.

## Visual

- No committed baseline screenshots, so visual regression is **INCONCLUSIVE**,
  not PASS.
- No layout overflow was observed at 1440×900 in the captured screenshots.

## Accessibility

- axe-core was **NOT RUN**. Keyboard navigation was tested earlier by unit
  tests only; no screen-reader pass was performed.

## Verdict

**INCONCLUSIVE for release** — the full click-through works, including a real
indexed file and a playable preview, but there is no visual baseline, no axe
run, no native player, and the browser-dev `imdb_lookup` 404 remains by design.
The four bugs found here were fixed and re-verified with the test suite.
