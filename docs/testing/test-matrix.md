# Test matrix

Derived from `Development.md` §11 and §17. This file is the operational version:
what runs, where, and what counts as evidence.

**Rule:** `NOT RUN` is a distinct result from `PASS`. Recording a test as passed
when it was not executed is a reporting defect and, per `AGENTS.md`, a stop-the-
line event for the task.

**Observed on 2026-11-19 at HEAD `7d48507`:** Rust unit/integration suites PASS
(107 tests, 1 ignored), frontend component suites PASS (11/11), token/bundle
checks PASS, CI workflow present but no GitHub Actions run recorded. The
Security, E2E and Performance layers below are specified but **NOT RUN** as
cross-cutting suites; the path-escape and auth cases currently live in
`crates/media-library/tests/roots.rs` and `crates/core/tests/auth.rs`.

## Layers

| Layer | Location | Runs on | Purpose |
|---|---|---|---|
| Unit | beside the code | every commit | Domain rules, validation, sanitisers, parsers |
| Integration | `tests/integration` | every commit | SQLite, filesystem behaviour, scanner, config, migrations |
| Security | `tests/security` | every commit (CI) | Path escape, boundary enforcement, redaction |
| Component/UI | `apps/desktop/src/**/*.test.tsx` | every commit | States, focus behaviour, token usage |
| E2E | `tests/e2e` | pre-release, and nightly once stable | Real app: import → browse → play → resume → uninstall |
| Performance | `tests/performance` | per checkpoint | Idle RAM/CPU, startup, library scale, frame drops |

## What must be tested at each layer

### Domain (unit)
- Completion logic: every signal and every adversarial combination (seek-to-end,
  close, queue-removal, short file, normal watch).
- Storage policy exclusions: source media, playing, unfinished, unverified,
  outside root.
- Path policy: traversal, absolute, UNC, reparse point, case tricks, Unicode
  normalisation, long paths.
- Metadata sanitiser: control characters, RTLO, extreme length, malformed image.

### Persistence (integration)
- Migration from a prior-version fixture; failed migration rollback.
- Foreign-key enforcement.
- UTC round-trip across DST.
- Atomic write on interrupted write.
- Index usage via `EXPLAIN QUERY PLAN`.

### Library (integration)
- Scan cancellation and resume without duplicates.
- Nested roots do not double-index.
- Locked and disappearing files are skipped, not fatal.
- Removing a root deletes zero files (file-count proof).

### UI (component)
- Every state: empty, loading, offline, missing artwork, error.
- Focus restoration after every overlay type.
- Shelf traversal including unequal-length shelves.
- Card scaling does not reflow neighbours.
- Token-only styling (literal grep gate).

### E2E (real app)
- First run: preflight → storage choice → import → browse.
- Playback: play, seek, pause, exit, resume.
- Restart recovery after a forced kill.
- Keyboard-only traversal of every screen.
- Uninstall with both data-retention answers.

## Fixtures

| Fixture | Purpose | Constraint |
|---|---|---|
| `tests/fixtures/library-small` | Happy-path browsing | No personal data |
| `tests/fixtures/library-10k` | Scale and search timing | Generated, not committed if large |
| `tests/fixtures/hostile-names` | Filename sanitiser | Generated filenames only |
| `tests/fixtures/dup-set` | Duplicate detection | Identical byte content |
| `tests/test-media` | Playback codec matrix | User-supplied or freely licensed only |

**Never commit copyrighted media.** `tests/test-media` is gitignored except for a
README describing what to place there. Test fixtures contain no personal data,
no real filenames from anyone's library, and no credentials.

## Hardware matrix

| Profile | RAM | Expected support | Status |
|---|---|---|---|
| Minimum experiment | 4 GB | Local playback only; containers disabled | UNTESTED |
| Entry | 8 GB | Library + playback; optional services gated | UNTESTED |
| Recommended | 16 GB | Full MVP | UNTESTED |
| Higher | 32 GB+ | Future headless/streaming | Out of scope |

OS: Windows 11 x64 required. Windows 10 x64 and ARM64 are **not claimed** until
tested (open decisions O-02 in `roadmap.md`).

Each row needs the exact machine recorded: CPU, RAM, GPU, Windows build.

## Playback matrix

Populated by P0-03 and P3-05. Cells are PASS / FAIL / **UNTESTED** — never
blank, never implied.

| Container / codec | Audio | Subtitles | Hardware decode | Result |
|---|---|---|---|---|
| MP4 / H.264 | AAC | SRT external | UNTESTED | UNTESTED |
| MP4 / H.264 | AAC | SRT embedded | UNTESTED | UNTESTED |
| MKV / H.264 | AC3 | ASS | UNTESTED | UNTESTED |
| MKV / HEVC | EAC3 | ASS | UNTESTED | UNTESTED |
| WebM / VP9 | Opus | SRT | UNTESTED | UNTESTED |
| MKV / AV1 | FLAC | none | UNTESTED | UNTESTED |

## Security test suite (`tests/security`)

Must cover, each asserting rejection **and** that no read/write occurred outside
the root:

- `..` traversal in file and folder names.
- Absolute paths and UNC (`\\server\share`).
- Junction / symlink / reparse-point escape.
- Case-sensitivity and 8.3 short-name tricks.
- Unicode normalisation and zero-width characters.
- Long paths beyond `MAX_PATH`.
- Wildcard/glob injection into cleanup targets.
- Cleanup rooted outside managed storage.
- Diagnostic bundle absence test for tokens, home paths and filenames.

## Definition of done for any task

- [ ] Acceptance criteria in `roadmap.md` observed, not assumed.
- [ ] Unit tests for new domain logic.
- [ ] Integration test where I/O is involved.
- [ ] Regression test for a bug fix.
- [ ] Security implications reviewed (paths, ports, secrets, permissions).
- [ ] Formatting, lint, build green.
- [ ] Results recorded in `progress.md` with PASS / FAIL / NOT RUN.

## Release gates

Mirrors `Development.md` §17. Product, reliability, security, performance and
documentation gates must all pass. No gate may be waived by an agent.
