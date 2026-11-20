# Downloaded Media Security Hardening Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ensure a file fetched by qBittorrent is inspected in the locked `probe` box and scanned by Windows Defender **before** mpv opens it on the host, so a hostile file cannot reach the desktop player unchecked.

**Architecture:** Add a two-stage gate between "download ready" and "playable": (1) `probe` (networkless, shell-less ffprobe container) opens the file and reports parse validity; (2) Windows Defender scans the specific file via `MpCmdRun.exe -ScanFile`. Both results are stored per download; `library_detail`/`playback_start` refuse to play anything not marked `clean`. mpv never opens an unverified file.

**Tech Stack:** Rust (std `Command`, existing `probe` container), SQLite (verification state), React (honest "scanning / blocked" UI state). No new dependency. Security guidance from the installed cybersecurity corpus skills (`analyzing-malicious-pdf-with-peepdf`-style untrusted-input handling, `performing-static-malware-analysis-with-pe-studio` fail-closed posture, `detecting-...` absence-assertion test discipline).

**Spec:** `docs/decisions/0018-qbittorrent-boundary.md` (untrusted media), `AGENTS.md` (validate at trust boundary), `AGENTS.md` non-negotiables (no data loss, local-first).

## Global Constraints

- Treat downloaded media as hostile input. Validate at the trust boundary; Rust owns policy, not React.
- No new dependency, crate, or container without an ADR. Reuse the existing `probe` image and Windows Defender (already on Windows 11).
- Scanning is best-effort detection, not a guarantee — never claim "malware-free", only "scanned / not flagged". Fail closed: if a scanner is unavailable or errors, the file is NOT playable and the UI says so honestly.
- No path in any renderer DTO; verification state is a display-only enum/string.
- No commit/push/install/host change unless this instruction grants it. Playwright/vite untouched.
- `PASS`/`FAIL`/`NOT RUN` describe observed runs; a scan that wasn't run is NOT RUN, not PASS.

## Task 1: probe parse-inspection gate

**Files:**
- Create: `crates/acquisition/src/inspection.rs` (probe invocation + result type)
- Modify: `crates/acquisition/src/lib.rs` (export), `crates/acquisition/src/orchestrator.rs` or core `/host/` routes (call on download-ready), `crates/persistence` (verification columns via migration `0005`), `crates/core/src/acquisition.rs` (result plumbing)
- Test: `crates/acquisition/tests/inspection.rs`

**Interfaces:**
- Consumes: `probe` container image (ffprobe), download file path from `file_mappings`.
- Produces: `enum InspectionState { Pending, Scanning, Clean, Suspicious, Unavailable }`; `inspect_with_probe(container_path) -> InspectionState`.

- [ ] **Step 1 — migration `0005_media_verification.sql`:** add `verification_state TEXT NOT NULL DEFAULT 'pending'` and `verification_detail TEXT` to `download_items`. Index on state.
- [ ] **Step 2 — failing tests:** (a) a valid media fixture → `Clean`; (b) a non-media/text file named `.mkv` → `Suspicious`; (c) probe container unavailable → `Unavailable` (not Clean — fail closed). Use an injectable `ProbeRunner` trait with a fake; live probe call NOT RUN (no engine) — record honestly.
- [ ] **Step 3 — implement `inspect_with_probe`:** invoke `docker run --rm --network=none --entrypoint ffprobe <probe-image> -v error -show_format <container_path>` via std `Command`, argument vectors only, bounded timeout. Non-zero exit or unparseable output → `Suspicious`/`Unavailable`. Store result in `download_items`. Never open the file on the host.
- [ ] **Step 4 — wire on download-ready:** after qB reports a file complete, call inspection before marking the item playable. `playback_start` and `library_detail` refuse items whose state is not `Clean`.
- [ ] **Step 5 — verify:** fmt/clippy/tests (target/verify), no new deps. Record exit codes.

## Task 2: Windows Defender scan gate

**Files:**
- Create: `crates/acquisition/src/defender.rs` (scan invocation)
- Modify: `crates/acquisition/src/lib.rs`, the same inspection pipeline from Task 1, migration columns if needed (`defender_state`), core routes, `apps/desktop/src/services/types.ts` (display state)
- Test: `crates/acquisition/tests/defender.rs`

**Interfaces:**
- Consumes: host-resolved file path from the A9 host map (host-internal), Windows `MpCmdRun.exe`.
- Produces: `defender_scan(host_path) -> DefenderState { NotScanned, Clean, Threat, Unavailable }`.

- [ ] **Step 1 — failing tests with injectable `Scanner` trait:** `Threat` when scanner reports a detection; `Clean` on clean result; `Unavailable` when Defender is missing/errors (fail closed — never Clean). Absence assertions: a threat value must not appear as playable.
- [ ] **Step 2 — implement `defender_scan`:** locate `MpCmdRun.exe` in `%ProgramFiles%/Windows Defender` or `%ProgramFiles(x86)%`; run `MpCmdRun.exe -ScanFile -ScanType 3 -File <path>` (argument vector, no shell), bounded timeout. Parse exit code + output for "threat detected". Non-Windows or missing binary → `Unavailable`. Record result.
- [ ] **Step 3 — combine gates:** an item is playable only if BOTH `InspectionState::Clean` AND `DefenderState::Clean`. Any other combination → blocked with a display-only reason string. Wire into `playback_start` refusal and the UI's honest blocked state.
- [ ] **Step 4 — verify:** fmt/clippy/tests, frontend typecheck/lint/Vitest/build/bundle, `git diff --check`. Live Defender scan of a real download NOT RUN (no lawful source) — synthetic clean/infected fixtures with the fake scanner prove the logic. No checkpoint passed.

## Sequencing

Task 1 -> Task 2 (shared verification columns and pipeline). Both use the same fail-closed enum pattern and absence-assertion tests. No new containers; probe already exists in compose.

## Risks

- Defender CLI output format varies by Windows version; fail closed to `Unavailable` and surface honestly rather than mis-parse.
- probe's ffprobe catches malformed media, not all exploits; this is defense in depth, not a guarantee. Documented honestly.
- Scanning a large file delays playability; run async on download-ready, not in the playback request.
