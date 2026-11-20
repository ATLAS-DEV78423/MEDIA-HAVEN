# Acquisition acceptance matrix

Maps the twelve §14 acceptance criteria of
`media_haven_consolidated_specification.md` to **observed** evidence from the
torrent-acquisition conversion (ADRs 0017–0019 accepted 2026-11-20; plan
`docs/superpowers/plans/2026-11-20-torrent-acquisition-conversion.md`, Tasks 1–7).

**Rule.** `PASS` / `FAIL` / `NOT RUN` describe runs that actually happened. A
mechanism proven only with fakes is reported as such; it is never presented as a
live acquisition. No checkpoint (CP-0…CP-7) is passed by this document — a human
reviews checkpoint evidence.

## Verdict legend

| Token | Meaning |
|-------|---------|
| **PASS** | The behaviour named in the criterion was observed in an executed test or command, cited below. |
| **PASS (mechanism) / NOT RUN (live)** | The app-side mechanism was observed (fake/temp fixture or signed HTTP round-trip), but the real-world path (lawful torrent, real media, real mpv or real qB deletion) was not run. |
| **PASS (pure policy) / NOT RUN (live wiring)** | Deterministic policy/integration tests (fake qB + temp SQLite through the production reconciliation path) were observed; no live qB, real media or real deletion was exercised. No wiring-level PASS is claimed. |
| **NOT RUN** | The criterion was not observed end to end; reason stated. No PASS is claimed. |

Live acquisition is **NOT RUN** throughout: ADR-0018 forbids bundling trackers,
indexers, catalogues or source-discovery code, and no lawful source was supplied.
The fake-server, temp-fixture and signed-HTTP tests exercise the same code paths
without acquiring or deleting real content.

## Matrix

| # | §14 criterion | Verdict | Evidence / honest reason |
|---|---------------|---------|--------------------------|
| 1 | A selected, authorized torrent can be submitted to qBittorrent through the application workflow. | PASS (mechanism) / NOT RUN (real source) | Live signed `POST /host/acquisition_add` with a synthetic magnet returned HTTP 200 and a path-free `DownloadItem`; qB WebUI `add` executed inside `core` on `acquire` (Task 4 fix report). No **authorized** torrent was acquired — the magnet was a synthetic infohash with no source. |
| 2 | Downloads remain confined to the configured media destination. | PASS | Live `docker inspect`: qB `/downloads` is a host bind `MODE=rw RW=True`; a file written by UID 10003 in qB appeared on the host and at `/downloads` in `core`; the `core` render was `MODE=ro RW=False`, `touch` → `Read-only file system` (`WRITE_EXIT=1`, `FILE_PRESENT=NO`); static test `compose_binds_qb_downloads_to_the_host_acquisition_root` (Task 2/3 reports). Confinement mechanism proven; not with real torrent bytes. |
| 3 | Available media appears in the frontend and can be played through the selected playback path. | NOT RUN | No lawfully acquired media existed to appear, and live mpv playback is NOT RUN (engine absent; live test `#[ignore]` — root `progress.md`, 2026-11-20 entry). Only the fake-transport Vitest proves a row renders after `download_add`; that is a UI presentation test, not real media or playback. |
| 4 | The user does not need to operate qBittorrent directly. | PASS | qB publishes no port (`docker inspect PORTS={}`, `PORTBIND={}`), is reachable only from `core` on `acquire`, and is never exposed to the renderer; credentials live in the host store and are injected into `core`, the WebUI cookie stays host-side; the renderer has named commands only and signed `/host/` routes stay outside the 24-command POST parity set. Verified by Task 2/3/4 boundary evidence + `route_parity` tests. |
| 5 | Starting an episode schedules the next two eligible episodes according to the chosen prefetch trigger. | PASS (logic) / NOT RUN (live) | `crates/acquisition/tests/prefetch.rs` — 10 tests: middle-of-season returns exactly two, final episode returns none, season boundary, per-series grouping, one-already-ready → one `acquire`, failed enqueue degrades, 90% cap + zero-limit fail-closed. Trigger on `playback_start` after policy checks; failures swallowed (`tracing::warn!`). Live prefetch NOT RUN: `source_for` is a stub returning `None` (no lawful per-episode source mapping; §15 gate), so production logs "no lawful source mapped" and enqueues nothing. |
| 6 | Ready or in-progress episodes do not create duplicate download tasks. | PASS | Orchestrator unit test: adding the same source twice calls `TorrentApi::add` once and returns the same id; frontend Vitest: duplicate add produces one merged row (`DownloadsPage.test.tsx`). Live duplicate check against qB with a real torrent NOT RUN. |
| 7 | Interrupted playback retains the episode and its resume state. | PASS | Observed resume round-trip (root `progress.md`, 2026-11-18 wiring): reopening a title resumed at the stored 15:00 mark and a checkpoint wrote 920 s, read back through `library_episodes`; `app-core` resume policy tests (`resume_position`) cover seek-to-end / mid-episode close / near-end skip. This is pre-existing B4 capability reused unchanged by the conversion. Resume through a **real mpv engine** NOT RUN (see #3). |
| 8 | Confirmed watched episodes become eligible for coordinated cleanup. | PASS (pure policy) / NOT RUN (live wiring) | Pure eligibility rule observed (`is_eligible` calls `app_core::domain::playback::is_complete` verbatim; 24 h grace, not playing, not a prefetch target) plus 14/14 cleanup policy tests. The B5 signal → signed `/host/acquisition_cleanup_confirm` → `cleanup_entries` → scheduled-pass wiring was NOT exercised end to end against a live core/qB: no real media or confirmed completion signal was supplied. No wiring PASS is claimed. |
| 9 | Cleanup does not remove files still needed by active playback or other retained episodes. | PASS (pure policy) / NOT RUN (live wiring) | Exclusion proofs observed on temp fixtures/fakes through the production reconciliation path (zero-deletion file-count proof, `null_media_id_sibling_is_always_kept_and_forces_keep_files`, still-needed sibling → `remove(hash, false)` with both files surviving, outside-root refusal, `live_playback_session_excludes_the_currently_playing_media`). No live qB container, real media or real deletion was exercised. |
| 10 | Application records reflect successful deletion or a recoverable failure state. | PASS (pure policy) / NOT RUN (live wiring) | `reconcile` now requires a positive, verified deletion count for `removed` — a zero-removal result maps to `failed` ("no files deleted; kept for still-needed episodes"), never "removed 0 file(s)" (AC-10). Observed in `zero_removal_never_reconciles_to_removed` and the production-wiring tests (`app_managed_grace_elapsed_completed_entry_is_actually_removed` → state `removed`; `zero_removal_is_never_reported_removed_through_production_wiring` → state `failed`), all on temp SQLite with a fake qB. Live deletion against qB NOT RUN. |
| 11 | Frontend clients do not receive unrestricted sandbox filesystem access or downloader credentials. | PASS | No published qB port; WebUI cookie and `MEDIAHAVEN_QB_TOKEN` stay host/core-side; `AcquisitionRow` and all renderer DTOs are path-free (only an acquisition **directory** setting, like `library_roots`, is exposed); named commands only; `/host/` routes outside the 24-command POST parity (`route_parity` green); `npm run check:bundle` reports no dev token / signed client in production bundle. |
| 12 | Unavailable upcoming episodes are represented accurately rather than promising seamless playback. | NOT RUN | Honest degradation is implemented — `source_for` returns `None` so no unavailable episode is enqueued or promised, and the buffer-pause notice (`bufferNotice`, message string only) is surfaced in the Downloads DTO — but there is no dedicated frontend or live test proving an "unavailable upcoming episode" is rendered accurately, and no per-episode source mapping or lawful source supplies real availability data. Reported NOT RUN, not PASS. |

## Full gate (observed by the controller — not re-run by this documentation task)

Recorded from the final Task 6 verification and the controller's consolidated
run. These results are attributed to those runs; this task did not re-execute
the suites.

| Check | Observed result |
|-------|-----------------|
| `cargo fmt --all -- --check` | clean, exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean, exit 0 |
| `cargo test --workspace` | **84 passed, 0 failed, 1 ignored** (`searching_10k_items_is_within_budget`); 24-command parity `command_lists_derive_from_one_source ... ok` |
| Desktop crate (extra) | `cargo test -p mediahaven-desktop` — **36 passed, 0 failed** |
| Task 6 cleanup suite | **14/14 pass** incl. zero-deletion file-count release blocker |
| Frontend `typecheck` / `lint` / `test` (Vitest) / `check:tokens` / `build` / `check:bundle` | **exit 0 / 0 / 0 / 0 / 0 / 0**; Vitest 6 files, **18 tests passed**; tokens 66 files, 0 violations |
| `git diff --check` | clean, exit 0 |

Note: two transient `LNK1104` linker file-locks occurred in the default target
directory during earlier task runs; a straight retry exited 0 with no code
change. Not a test failure.

## Checkpoint evidence (no checkpoint passed)

The conversion produces evidence relevant to **CP-5 (trustworthy under failure)**
and **CP-6 (workspace contained)** — data-loss exclusion proofs, qB boundary
inspection, no published port, credential handling. Those checkpoints remain
**OPEN**: a human must review the evidence and sign off. No agent marks a
checkpoint passed. CP-0…CP-4 and CP-7 are unaffected by this document.

## Deferrals / open decisions carried forward

- **Live acquisition + real-media playback/cleanup — NOT RUN** (no lawful source;
  mpv live test ignored). This is a §15 gate (R-01 source mechanism), not a test
  failure.
- **Per-episode source mapping** (`source_for` stub) — no lawful source-to-episode
  mechanism chosen; live prefetch stays a deliberate no-op.
- **Used-byte accounting stub** returns 0, so `bufferNotice` is `null` in
  production until real retained-byte accounting lands; the non-null path is
  unit-proven.
- **Frontend cleanup-status rendering** is not rendered (status persisted in
  `cleanup_entries` and returned as counts by the host pass).
- **Scheduler persistence** — the 1 h tick is in-process and not persisted across
  a `core` restart (courtesy minor; no unjustified deletion possible because all
  preconditions persist in SQLite).
- **`.torrent` support** accepts a bare file name, not uploaded bytes; hash
  resolution diff has no bounded poll for slow qB indexing.
- **LAN non-reachability of the WebUI** from a second machine — human evidence
  gate, NOT RUN.

## Verification performed by this documentation task

Recorded here for honesty; no source, config or test was changed by this task.

| Command | Exit | Result |
|---------|------|--------|
| `git diff --check` | **0** | clean; one pre-existing warning on `Cargo.toml` (CRLF→LF normalization, a file not edited by this task) |
| grep: 12 §14 criterion rows in this matrix | — | **12 matches** (criteria 1–12 all present) |
| grep: stale deferral text in `roadmap.md` | — | **0 matches** (no "whole download workstream is out of v1" remains) |
| grep: `### [~] P6-02` and rewritten `O-05` row in `roadmap.md` | — | both present (lines 1499 and 1742) |
| grep: conversion-complete section in `progress.md` | — | present (line 9) |
| grep: ADR-0017/0018/0019 index rows in `docs/decisions/README.md` | — | **3 matches**, all marked **Accepted** (lines 27–29); no change needed |

No source, configuration, test, checkpoint or ADR status was changed by this
documentation task; only the four files in its write ownership were touched.

## 2026-11-20 — final-review fix wave addendum

Criteria 8–10 were downgraded above from PASS-with-wiring to
**PASS (pure policy) / NOT RUN (live wiring)**. The final-review fixes also
added production-path integration tests through real SQLite with a fake qB
(no injected eligibility predicate) and removed the read-only `core` local-file
deletion; qB now owns deletion via `remove(hash, true)` and a zero-file result
can no longer be reported as `removed`. Exact commands, exit codes and the
updated gate results are recorded in
`.superpowers/sdd/2026-11-20-torrent-acquisition-conversion/final-fix-report.md`.
No checkpoint was passed.
