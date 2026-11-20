# Release Blockers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Clear every code-side blocker to playing a downloaded file: R‑01 per-item source mapping, Defender trigger, download-remove persistence, UI status rendering, and the restart-lost scheduler/session.

**Architecture:** R‑01 is implemented as the recommended option A — the operator attaches a magnet/`.torrent` *source* to each library item via its Details screen (or a source supplied at download time is associated with an item). This sets `download_items.media_id`, which unblocks the existing prefetch + cleanup + security gates. A local sources store backs it, no discovery/bundled sources. Remaining items are small focused fixes in existing files.

**Tech Stack:** Rust (axum/sqlx, existing `acquisition` crate), React 19, existing SQLite. No new dependency.

**Spec:** `media_haven_consolidated_specification.md` R‑01; ADR‑0018 (no bundled sources); ADR‑0021 (R‑01 mechanism — to be written by Task 0 as a reversible ruling: human said "solve whatever you can", recommended option A adopted, reversible).

## Global Constraints

- Ruling (human delegated 2026‑11‑21): "solve whatever you can" → adopt option A (per-item user-supplied source, no discovery). Record in ADR‑0021 as reversible by the human; do not bundle trackers/indexers/catalogues (ADR‑0018). Lawful use only.
- The security gates must not be weakened: a file is playable only when probe=Clean AND Defender=Clean AND it is tied to a media_id. When R‑01 sets media_id, guard that the row's verification states are checked — a `None` state must NOT become an allow for a downloaded row.
- No new dependency; no path in renderer DTOs; no commit unless instructed; Playwright/vite untouched.
- `PASS`/`FAIL`/`NOT RUN` describe observed runs. Live download not possible without lawful source — logic proven with fakes/fixtures, reported honestly.

---

## Task 0: ADR‑0021 + R‑01 source store and Details UI

**Files:** create `docs/decisions/0021-r01-source-mapping.md`; create `crates/persistence/migrations/0007_item_sources.sql`; modify `crates/app-core/src/domain/media.rs`, `crates/app-core/src/ports/library.rs`, `crates/app-core/src/application/library.rs`, `crates/persistence/src/sqlite/library_repo.rs`; modify `apps/desktop/src/pages/Details/*` (Sources field), `apps/desktop/src/services/{types,api}.ts`; report at `.superpowers/sdd/2026-11-21-release/task-0-report.md`.

**Interfaces:**
- Produces: `item_sources` table (`media_id TEXT PRIMARY KEY, source TEXT NOT NULL, source_name TEXT`); port `source_for_media(media_id) -> Option<String>`, `set_item_source(media_id, source)`; named command `source_set(media_id, source)`; `source_for` in core now reads the store instead of returning `None`.

- [ ] **Step 1 — ADR‑0021:** record the R‑01 mechanism decision: per-item operator-supplied source via Details, no discovery, reversible; reference ADR‑0018 and the human's delegation. Status Accepted with attribution "human delegated 'solve whatever you can' 2026‑11‑21; mechanism chosen by agent as recommended option A, reversible". Index it in `docs/decisions/README.md`.
- [ ] **Step 2 — failing test** in `crates/persistence/tests/`: set a source for a media id, read it back; unknown id → None; empty/oversized/non-magnet-non-.torrent source rejected. Run → FAIL (table/method missing).
- [ ] **Step 3 — migration + repo:** `0007_item_sources.sql`; implement `source_for_media`/`set_item_source` with bound params; `set_item_source` must also set `download_items.media_id` for any download row whose torrent hash matches the source, so the gates can see it. Test the tie: after setting a source, a download row with that hash has `media_id` set.
- [ ] **Step 4 — guard:** add a test that a download row with `media_id` set but verification_state NULL is NOT allowed to play (fail closed). Adjust `refuse_unverified`/gate if needed so `None` for a *download row* is refused, while library imports (no download row) remain allowed.
- [ ] **Step 5 — wire `source_for`:** in `crates/core/src/acquisition.rs` replace the `None` stub with a call to `source_for_media`; keep prefetch failure non-fatal.
- [ ] **Step 6 — UI:** a "Source" field on Details (magnet/`.torrent` path text + Save) calling the named command; no path/credential shown. Vitest against a fake transport.
- [ ] **Step 7 — verify:** fmt/clippy/tests, frontend checks, parity 24, `git diff --check`.

## Task 1: Defender trigger + download-remove persistence

**Files:** `crates/core/src/acquisition.rs`, `crates/persistence/src/sqlite/library_repo.rs`, `apps/desktop/src-tauri/src/commands/renderer.rs`, report.

- [ ] After qB reports a file complete (the inspect path already persists probe state), call `defender_scan` on the host-resolved path and persist via a new `set_defender_verification`. Mockable with a trait; live scan NOT RUN. Test: clean result persisted; threat -> blocked.
- [ ] `download_remove` currently removes only the in-memory task: delete the `download_items`/`file_mappings`/`torrent_tasks` rows for that id too (never source media). Test row count before/after; file count unchanged.
- [ ] Verify as above.

## Task 2: UI status + restart-persistent scheduler/session

**Files:** `apps/desktop/src/pages/Downloads/*`, `apps/desktop/src/services/types.ts`, `crates/core/src/acquisition.rs`, `crates/persistence` (if needed), report.

- [ ] Render `mediaStatus` (`scanning/blocked/unavailable/ready`) from the existing DTO field in Downloads and Details; Vitest with a fake row.
- [ ] Persist live-playback session and pending cleanup schedule in SQLite (small table or reuse `cleanup_entries` with a `due_at`); on core start, recover pending passes; a playing file is still excluded. Test recovery from a simulated restart; no unjustified deletion.
- [ ] Verify as above.

## Task 3: Packaging (installer + SBOM)

**Files:** `infrastructure/packaging/windows/*`, `.github/workflows/release.yml`, `docs/operations/*`, report.

- [ ] Tauri bundler config: per-user install, no admin, unsigned (O‑08 decided), version from one source. Reproducible build command. SBOM generation (use an existing tool if available; if it needs a new install, document NOT RUN rather than installing without consent). Third-party licences bundled. Checksums published. Record exact commands/results; a full installer build may need WebView2 tooling already present — attempt and report honestly.
- [ ] Verify config + what runs; docs updated with honest limitations.

## Sequencing

Task 0 → Task 1 → Task 2 (shared gate/store seams). Task 3 independent (packaging files) — may run in parallel with Task 0–2. All before the IMDb plan. No checkpoint passed by agents.
