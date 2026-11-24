# Streaming adoptions + playback completion — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development.
> Use parallel lanes only when file sets are disjoint (explicit human instruction,
> 2026-11-23). Ponytail discipline stays active in every lane.

**Goal:** Finish the in-window playback work by running its blocking live gate,
fix the player teardown defect, and adopt three local-first features observed in
the Miauwatch reference that fit this project's non-negotiables.

**Spec authority:** `docs/superpowers/specs/2026-11-23-in-window-playback-design.md`
and the feature decisions recorded in `docs/reference/mauflix-cross-reference.md`.

**Source instruction:** human message 2026-11-23 ("integrate mpv directly into the
ui ... connect all the screens ... see how it gets user recommendations and also a
movie catalogue ... how the user is able to search ... adopt ... use subagent
driven development ... keep the ponytail plugin active ... parallel execution").

## Global Constraints

- Windows 11 x64, Tauri 2, single `main` window only. No second window, ever.
- **No content-source bundling**: no indexers, trackers, scrapers, magnet lookup,
  debrid, or instructional acquisition code. The reference pipeline is documented
  only, never ported.
- No new runtime dependency, no new crate, no new backend route, no change to the
  24-command `COMMANDS` set or route parity.
- No commit, push, install, download of executables, or host change without
  explicit human permission.
- mpv remains user-supplied at `MEDIAHAVEN_MPV`; never downloaded silently.
- Ponytail discipline: smallest change that fully works; no abstraction, no
  "for later" code; finish every caller, test and fixture the change breaks.
- Design tokens only in UI; no hard-coded colours or spacing.
- Use `CARGO_TARGET_DIR=target/verify` for repository Rust verification; the
  spike uses `target/spike` so parallel lanes cannot contend on a linker lock.

## Task 1 — Blocking live spike (controller-owned, verification only)

Run the throwaway probe in `spikes/mpv-embed/` with the present mpv binary and an
authorised local fixture. Observe: (1) no second taskbar/top-level window,
(2) host controls visible/clickable, (3) resize follows rect, (4) no orphan mpv.
Record PASS/FAIL per gate. A failure reopens the design decision.

## Task 2 — Player teardown and layout observation (Lane P)

**Files:** `apps/desktop/src-tauri/src/player.rs`, `apps/desktop/src/services/player.ts`,
`apps/desktop/src/services/player.test.ts`, `apps/desktop/src/pages/Player/PlayerPage.tsx`,
`apps/desktop/src/pages/Player/PlayerPage.test.tsx`.

- Add a `stop` action to the existing host-only `player_control` command so the
  renderer can end playback; keep `COMMANDS` at 24.
- On PlayerPage unmount, stop playback (hides the child surface) before saving.
- Replace the `window.resize` listener with a `ResizeObserver` on the player root,
  recomputing the rect from the top bar and controls bounds.
- Tests: host `control("stop")` idempotent when idle; renderer sends `stop`;
  unmount stops playback; ResizeObserver reports the surface.

## Task 3 — Search genre chips (Lane S)

**Files:** `apps/desktop/src/pages/Search/SearchPage.tsx`,
`apps/desktop/src/pages/Search/search.module.css`,
`apps/desktop/src/pages/Search/SearchPage.test.tsx` (new if absent).

- Derive available genres from the already-loaded local library.
- Chip selection passes `genres` to the existing `api.search(text, genres)`;
  changing query or chips cancels stale requests as today.
- No new command, no new backend work, no online call.

## Task 4 — Local "Because you watched" shelf (Lane R)

**Files:** `apps/desktop/src/services/recommend.ts` (new),
`apps/desktop/src/services/recommend.test.ts` (new),
`apps/desktop/src/pages/Home/HomePage.tsx`,
`apps/desktop/src/pages/Home/home.module.css` (only if strictly required).

- Pure local recommender: rank unwatched titles by genre overlap with the
  highest-progress watched title. No network, no persisted profile data.
- Render one extra shelf on Home, only when a recommendation exists.
- Tests: ranking prefers shared genres, excludes watched titles, empty on no
  progress.

## Task 5 — Verification, checklist and docs (controller-owned)

- Run all Rust gates and all frontend gates; record PASS/FAIL/NOT RUN honestly.
- Write `docs/reference/mauflix-cross-reference.md` with the requested checklist
  and cross-reference, including what was deliberately NOT adopted and why.
- Update `roadmap.md` and `progress.md`; no checkpoint marked passed by an agent.
