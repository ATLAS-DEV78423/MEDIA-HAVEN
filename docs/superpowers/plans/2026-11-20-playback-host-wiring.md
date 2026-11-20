# Playback Host Wiring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire the accepted mpv adapter (`crates/playback`) into the Tauri host and renderer so playback is driven by a real engine instead of a 4× timer — while no path or token ever reaches the WebView (ADR-0015, accepted 2026-11-20).

**Architecture:** The backend exposes the media item's **container-path identity** to the **host only** (a host-internal command, not renderer-invocable). The host resolves it through A9's `host_path_map()` via `media_playback::resolve_source`, opens the file in `MpvPlayer`, and emits `player://state`. The renderer subscribes to that event and drops `SIMULATION_RATE`. mpv is not installed here, so live playback stays NOT RUN; all wiring is tested headlessly with fakes.

**Tech Stack:** Rust + Tauri 2 commands/events, existing `crates/playback`, `serde_json`, React 19 + `@tauri-apps/api` event listeners (already available via Tauri), Vitest.

**Spec:** `docs/decisions/0015-playback-descriptor.md` (Accepted), `docs/decisions/0002-playback-engine.md`, `2026-11-19-remaining-development.md` Lane B.

## Global Constraints

- No path or token in any renderer-facing DTO. `PlaybackSession` stays path-less. The container path is host-internal only.
- `playback_open` is host-internal, NOT a renderer command; no generic passthrough (ADR-0005 §3). The 24-command parity test must still pass unchanged.
- Failed source resolution fails closed with a legible `source unavailable` state (ADR-0015).
- No new dependency. No install of mpv (needs consent). No commit/host change. Playwright and `vite.config.ts` untouched.
- Keep the simulation as a dev-only fallback behind the existing dev client so browser development (no Tauri host) still works — do not delete the dev path; gate the real engine to the Tauri host.
- `cargo fmt`, clippy `-D warnings`, full workspace tests, and frontend typecheck/lint/Vitest/build must pass.

---

### Task 1: Host-internal container-path lookup + resolver

**Files:**
- Modify: `crates/app-core/src/ports/library.rs`, `crates/app-core/src/application/library.rs`
- Modify: `crates/persistence/src/sqlite/library_repo.rs`
- Modify: `crates/core/src/routes.rs`
- Modify: `apps/desktop/src-tauri/src/commands/renderer.rs`
- Test: `crates/persistence/tests/repository.rs`

**Interfaces:**
- Produces: `LibraryRepository::media_source_path(&self, id: &str) -> Result<Option<String>, AppError>` returning the stored `media_files.path` (container path) for a media id, or `None`.
- Consumes: `media_files.path` column (already stored by the scanner).

- [ ] **Step 1 — repository method + test.** Add the trait method and sqlite impl (`SELECT path FROM media_files WHERE media_id = ? LIMIT 1`). Test in `repository.rs`: a seeded media id returns its stored container path; unknown id returns `None`. Like metacharacters are not involved (exact match, bound parameter).
- [ ] **Step 2 — service passthrough** in `application/library.rs`.
- [ ] **Step 3 — backend route** `GET /playback_source/:media_id` returning `{ "path": <string|null> }`. Use **GET**, not POST: `crates/contract/tests/route_parity.rs` derives renderer commands from POST routes only (GET probes like `/healthz`/`/readyz` are excluded by construction). A GET route keeps this host-internal lookup OUT of the 24-command renderer surface, so the parity test stays green with no contract change. It still requires the signed auth middleware (`/readyz` is the precedent for an authenticated GET). Add the matching `.route(...get(...))` registration in `routes.rs` and a comment: host-internal, not renderer-invocable. Do NOT add any name to `crates/contract/src/lib.rs` COMMANDS.
- [ ] **Step 4 — host helper** in `src-tauri/src/player.rs` (new): `resolve_for_media(media_id, source_path) -> Result<PathBuf, PlayerError>` — builds the host map from the persisted A9 override / roots and calls `media_playback::resolve_source`. Fails closed. Add a unit test with a temp root and a fake map: valid resolves, stale path fails.

### Task 2: Host player state + commands + events

**Files:**
- Modify: `apps/desktop/src-tauri/src/player.rs`, `apps/desktop/src-tauri/src/lib.rs`, `apps/desktop/src-tauri/src/commands/renderer.rs`

- [ ] **Step 1 — `PlayerState`** in `player.rs`: an `Arc<Mutex<Option<MpvPlayer<ConnectedIpc, ProcessChild>>>>` held in Tauri managed state, plus the mpv executable path from config (default `mpv` on PATH, overridable by `MEDIAHAVEN_MPV` env). `open_media(media_id, start)` resolves via Task 1, spawns mpv if idle, opens the file, starts a background poller that emits `player://state` every 500 ms with `{ timePos, pause, volume, mediaId }` (display-only numbers, no path). `stop()` stops and drops the player (kills the child).
- [ ] **Step 2 — wire commands:** in `renderer.rs`, `playback_start` first calls `post(...)` to get the session, then calls `state.open_media(media_id, start_position_seconds)`; if resolution fails, return a `CommandError` mapping to `source unavailable` (legible). `playback_stop` also calls `state.stop()`. Add a Tauri `generate_handler!` registration and a `#[tauri::command]` `player_control(action, value)` for play/pause/seek/volume/set track — named, not passthrough. Add it to the contract as host-internal host→renderer visibility if needed; ensure the 24-command renderer parity test stays valid.
- [ ] **Step 3 — event tests:** a headless test that a fake player's status produces the expected JSON shape (no path/token keys). Emit tests are not runnable without the Tauri runtime — record NOT RUN.

### Task 3: Renderer consumes real engine events

**Files:**
- Modify: `apps/desktop/src/pages/Player/PlayerPage.tsx`, `apps/desktop/src/services/api.ts`, new `apps/desktop/src/services/player.ts`

**Interfaces:**
- Consumes: `player://state` events `{ timePos, pause, volume, mediaId }`; named commands `player_control`.
- Produces: the player UI driven by engine state.

- [ ] **Step 1 — `player.ts` client:** `listenPlayerState(cb)` using `@tauri-apps/api/event` `listen`, guarded so it is a no-op outside Tauri (dev browser keeps the simulation). `playerControl(action, value)` calls the named command only in the Tauri host.
- [ ] **Step 2 — `PlayerPage.tsx`:** subscribe to `player://state`; when events arrive, set `elapsed` from `timePos` and `playing` from `pause`; the simulated interval now runs **only** when no engine stream has been seen (dev fallback). Route seek/volume/pause through `playerControl` in the host; keep the existing checkpoint saves. Vitest with a fake event stream asserts the clock follows engine positions and no path is rendered.
- [ ] **Step 3 — bundle check:** `npm run check:bundle` still passes (no path/token string). Typecheck/lint/Vitest/build green.

### Task 4: Evidence + documentation

**Files:** `docs/security/boundary-evidence.md`, `progress.md`, `roadmap.md`

- [ ] Record the ADR-0015 executable evidence: resolution test, fail-closed test, no-path-in-DTO check (bundle scan), and the NOT RUN live playback smoke (mpv absent). Do NOT mark CP-3 passed. Record the new host-internal command and justify why it is not a renderer capability.

## Sequencing

Task 1 → Task 2 → Task 3 (they share interfaces). Task 4 after all. Tasks 1–3 are one coordinated lane (controller-owned files); dispatch as a single implementer to avoid conflicts. mpv install remains a human-consent gate for the live smoke only.
