# Progress

## Last updated

2026-11-18 (session: backend completion master plan written; frontend rebuilt on a single token system and wired to the real backend; D2 local NFO metadata, A2 Tauri host scaffold, and A3 token provisioning + A4 supervisor landed)

## Current state

The repository holds the approved engineering specification (`Development.md`), the
checkpoint-driven roadmap, the documentation baseline and the agent tooling.

**The frontend is now built.** `apps/desktop` is a runnable Vite + React +
TypeScript application implementing the Apple TV-style interface from
`Development.md` §7.4. It runs in a browser at `http://localhost:5173/` via
`npm run dev`. The backend does not yet exist; the frontend's data-dependent
acceptance criteria remain open until the backend lands (P4-08).

> **Superseded 2026-11-18:** the earlier component set, typed command contract,
mock transport and Stitch/Tailwind pages were replaced by a full redesign on a
single token system. The UI now reads fixture data from `src/data/catalog.ts`
and keeps local state in `src/state/AppState.tsx`; there is no transport seam
in the tree at the moment. The session report at the end of this file records
what was verified and what remains open.

**Rust workspace and Tauri host now exist.** `crates/{app-core,core,media-library,
metadata,persistence}` build, and `apps/desktop/src-tauri` opens the MEDIA_HAVEN
window. The host↔container boundary (P1-08) is still a design: the Tauri host has
no supervisor, token or command surface yet, and the renderer still calls the API
over HTTP with the development token.

Phase 0 has **not** started: no CP-0 acceptance criterion is satisfied, because
every one of them requires an observed measurement, not a document.

## Active task

- **ID:** P4-01 … P4-07 — Frontend (TV experience, frontend-first)
- **Goal:** Build the full frontend against a frozen contract and a development
  mock, per ADR-0009.
- **Status:** Implemented and verified locally; data-dependent acceptance
  criteria remain open pending the backend (P4-08).
- **Branch/commit:** `master`, tracking `origin/master`.
  - `4d28f79` — docs: make Docker mandatory for v1 and resolve the runtime architecture
  - `d826523` — chore: pin line endings with .gitattributes
  - `56dc4f9` — docs: record the baseline commits and push in progress.md
  - `c86ffd4` — docs: add the missing watchlist feature to the specification
  - `c9a1675` — feat: build the full frontend against a frozen command contract
  - `8e49047` — style: apply the MEDIA HAVEN cinematic design system

## Completed this session

### Frontend (`apps/desktop`)

- Scaffolded Vite + React 19 + TypeScript + React Router 7 + Vitest + Testing
  Library + ESLint 9 (flat config). 278 packages installed; no Rust or Tauri
  shell.
- Design tokens in `src/styles/tokens.css` — the only place a literal may live.
  Plus `global.css` and `motion.css`. `npm run check:tokens` enforces it.
- A frozen typed command contract in `src/types/media.ts` (DTOs, command
  surface, errors, helpers) — the single source of truth.
- A transport seam (`src/services/transport.ts`) that picks the mock in
  development and the Tauri host in production; the mock is excluded from the
  production bundle (verified by the build).
- A named command layer (`src/services/commands.ts`) — no generic passthrough
  (ADR-0005).
- A development mock returning a deterministic fixture library
  (`src/services/mock/`), with abstract, stable cover art and every state the
  UI must handle.
- `FocusManager` with the shelf navigation model, overlay stack and focus
  restoration — the highest-regression area in the app.
- Primitives: `Artwork`, `MediaCard`, `EpisodeCard`, `CollectionCard`,
  `ProgressBar`, `StatusPill`, `SkeletonCard`, `MediaShelf`, `HeroFeature`.
- `MediaDetails` overlay (seasons, episodes, play/queue/watchlist) and the
  `Player` surface with controls, scrubber, subtitle/audio selects, idle fade
  and up-next.
- Pages: `Home`, `Movies`, `Shows`, `Library`, `Watchlist`, `Downloads`,
  `Search`, `Settings`, `Setup`. Every page has loading, empty, error and
  offline states.
- A focus-navigation test suite (8 tests) covering horizontal movement, edge
  clamping, vertical movement with column preservation, unequal-shelf clamping,
  and overlay focus restoration.

### Verified this session

| Check | Result | Notes |
|---|---|---|
| TypeScript (`tsc --noEmit`) | **PASS** | Strict mode, including `noUncheckedIndexedAccess` |
| ESLint | **PASS** | `--max-warnings 0`, flat config |
| Vitest | **PASS** | 8/8 |
| Token discipline (`check-tokens.mjs`) | **PASS** | 48 UI files, 0 violations |
| Vite build | **PASS** | 80 modules, 318 KB JS / 36 KB CSS (gzipped 98 KB / 6.5 KB) |
| Dev server | **PASS** | `http://localhost:5173/` — Home renders with nav rail, hero and shelves, no console errors |
| Mock excluded from production bundle | **PASS** | `createMockTransport` is not present in `dist/` |

### Not verified

- Contrast ratios (no automated check wired yet — P4-01).
- Dropped-frame numbers (no profiling run — P4-05).
- A keyboard-only walkthrough of every screen (P4-03).
- Any data-dependent acceptance criterion (P4-08 — needs the backend).

## Current implementation facts

### Repository state
- Frontend: `apps/desktop` — Vite 7 + React 19 + TypeScript 5.9 + React Router 7,
  Vitest 3 + Testing Library, ESLint 9 flat config. 278 packages in
  `node_modules` (gitignored).
- `apps/desktop/src-tauri/` is still empty. The Tauri shell and the
  host↔backend boundary (P1-08) are backend-side work.
- No Rust workspace, no database, no container images.

### Decisions in force
- Desktop shell: Tauri 2 (not yet scaffolded). Backend: Rust. Frontend: React + TypeScript (Vite).
- Persistence: SQLite with migrations. Local-first, no mandatory accounts.
- **Docker/Compose is mandatory for v1.** The backend and the media-inspection
  workload run in containers. There is no host-side backend fallback.
- The Windows runtime distribution is **Proposed**: Docker Engine in a dedicated
  WSL2 distro (ADR-0001 Part 2).
- **Playback runs on the Windows host** — an intentional, documented exception
  (ADR-0002). The backend stays the authority on what to play.
- **Frontend-first delivery** (ADR-0009): the interface is built against a frozen
  contract with a development-only mock. The mock implements no policy.
- One product surface. Internal containers are an implementation detail.
- MVP is direct HDMI playback on one Windows laptop. Remote streaming deferred.
- No AI runtime dependency after setup.
- Automatic permanent deletion is OFF by default.

### Reference machine (observed, read-only, 2026-11-15)
- Windows 11 Pro build 26300, 16 GB RAM.
- Intel Core i7-13700H (14C/20T), Intel Iris Xe (Xe-LP).
- WSL2 present; default distribution `OmniRushBox` (stopped); `docker-desktop`
  distro running.
- Docker Engine 29.9.3, kernel `6.18.58.1-microsoft-standard-WSL2`, cgroup v2,
  storage driver `overlayfs`.
- `docker info` reports `SecurityOptions: [name=seccomp,profile=builtin
  name=cgroupns]` — AppArmor absent, seccomp and cgroup namespaces present.
- WSL2 VM memory ceiling observed at 8 073 883 648 bytes (8 GB), 20 CPUs visible.
- 12 unrelated containers already running on the shared engine.

### Known working commands
- Frontend dev server: `cd apps/desktop && npm run dev` (port 5173)
- Typecheck: `npm run typecheck`
- Lint: `npm run lint`
- Tests: `npm test`
- Token check: `npm run check:tokens`
- Build: `npm run build`
- Preflight (read-only): `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/windows/preflight.ps1`
- Compose parse check: see `docs/security/compose-security-checklist.md`
- Nothing else. No Rust or Tauri build yet.

## Verification

| Check | Result | Notes |
|---|---|---|
| Rust formatting | NOT RUN | No Rust code exists yet |
| Rust tests | NOT RUN | No Rust code exists yet |
| Frontend typecheck | **PASS** | Strict mode, `noUncheckedIndexedAccess` on |
| Frontend lint | **PASS** | `--max-warnings 0` |
| Frontend tests | **PASS** | 8/8 — focus navigation and overlay restoration |
| Token discipline | **PASS** | 48 UI files, 0 violations |
| Frontend build | **PASS** | 80 modules, 318 KB JS / 36 KB CSS |
| Dev server render | **PASS** | Home renders with nav rail, hero and shelves; no console errors |
| Mock excluded from production bundle | **PASS** | `createMockTransport` absent from `dist/` |
| Compose file resolves | **PASS** | `docker compose config --quiet` → exit 0 |
| Compose security posture | **PASS** | `read_only`, `cap_drop`, `no-new-privileges`, limits, `host_ip: 127.0.0.1`, `internal: true` |
| Compose fails closed without a digest | **PASS** | exit 1 with a clear "required variable" error |
| Compose static greps | **PASS** | No `privileged` / socket / `network_mode` findings; all images pinned |
| Documentation consistency | **PASS** | 55 tasks, 8 phases, 8 checkpoints cross-checked; no dangling task refs |
| Reference-machine runtime facts | **PASS** (read-only) | OS/RAM/CPU/GPU, `wsl -l -v`, `docker context ls`, `docker info`, `docker ps`, `docker images` |
| Upstream documentation research | **PASS** (read-only) | Docker Desktop licensing and WSL2 backend docs, Microsoft WSL networking and GPU-compute docs, mpv manual and README |
| Contrast ratios | NOT RUN | No automated check wired yet (P4-01) |
| Dropped-frame numbers | NOT RUN | No profiling run (P4-05) |
| Keyboard walkthrough of every screen | NOT RUN | P4-03 |
| CP-0 evidence items | **NOT RUN** | 0 of the CP-0 criteria are satisfied by this session |

## Blockers and failures

- **No blocker to the frontend work.** The frontend is built and verified.
- **CP-0 is not passed and must not be treated as passed.** Every criterion requires
  a measurement or a human sign-off.
- **The dev server is running** at `http://localhost:5173/`. Stop it with Ctrl+C in
  that terminal when you are done testing.
- **The frontend's data-dependent acceptance criteria are open** (P4-08): they need
  the real backend, which does not exist yet.
- **`apps/desktop/src-tauri/` is empty.** The Tauri shell and the host↔backend
  boundary (P1-08) are backend-side work. The frontend's transport seam is already
  shaped so the swap is a one-file change.

## Known risks / regressions

- None in the frontend's verified checks.
- **Unreviewed-architecture risk:** ADRs 0002, 0005, 0006, 0007, 0008 and 0009 are
  all **Proposed**. An agent may write them and recommend; only the human partner
  may accept them.
- **Tooling risk:** `.agents/skills` and `.claude/` are gitignored, so a fresh
  clone will not have the skills or the design hooks. `README.md` documents the
  reinstall command.
- **Mock drift risk:** if the mock grows policy, filesystem access or a database,
  the mandatory boundary (ADR-0001) is breached. The lint and typecheck do not
  catch this; review does. ADR-0009 records the revisit trigger.
- **Inline styles:** `WatchlistPage` and `DownloadsPage` use a few inline styles
  that reference tokens (no literals). They are honest but inconsistent with the
  CSS-module convention; a follow-up should move them into modules.

## Next actions

1. **Test the frontend in the browser** at `http://localhost:5173/`. Walk the nav
   rail, the hero, the shelves, the details overlay, the player, search, watchlist,
   library, settings and setup. Check keyboard-only navigation and the focus ring.
2. **Accept or revise ADR-0009** (frontend-first, contract-driven), plus ADRs 0002,
   0005, 0006, 0007 and 0008.
3. **Start Phase 0** with P0-10 (measure the runtime distributions) and P0-13
   (prove LAN non-reachability) — these two unblock the runtime decision and the
   security claim respectively.
4. Acceptance criteria: an ADR records the runtime distribution with the human's
   acceptance; the compose file and its checklist pass; and the CP-0 evidence list
   in `roadmap.md` has observed values behind each box.
5. Tests to run: the frontend suite (already green) plus the Phase 0 measurements.

## Decisions made

- **Docker is mandatory for v1** — reaffirmed by the product owner and now
  consistent across every document. Recorded in ADR-0001 Part 1.
- **Frontend-first delivery** — the product owner chose to build the full frontend
  before the backend. Recorded in ADR-0009. The frontend is built against a frozen
  contract with a development-only mock; the mock implements no policy.
- **Runtime distribution: dedicated WSL2 Docker Engine recommended** (ADR-0001
  Part 2). Status **Proposed**; the human partner accepts.
- **Playback on the Windows host** (ADR-0002). Status **Proposed**; placement
  recommended, engine and licence still open.
- **WSL2 mirrored networking excluded** from the publication path (R-18). Basis:
  Microsoft's own documentation lists LAN reachability as a mirrored-mode benefit.
- **Phase 6 restructured** to three tasks, with the workspace brought forward to
  P1-07/P1-08. Basis: the dependency inversion described above.
- **Watchlist added** as a first-class feature (Development.md §4.5.1, P2-07).
- **Git:** repository initialised, remote configured. Commit and push authorised by
  the human partner in this session; three commits on `master`, all pushed and
  verified in sync with `origin/master`.
- `memorix` remains **not adopted** — unchanged; it would need a decision record.
- No other product or architecture decision has been made by an agent.

## Open questions

1. **Runtime distribution:** dedicated WSL2 distro (recommended) or Docker Desktop?
   O-06 / O-07. Needs P0-10 evidence before acceptance.
2. **libmpv licence (O-09):** LGPL or GPL? This constrains the project's own
   licence and blocks CP-0.
3. **Project name and branding** (O-01) — needed before the installer and package
   IDs are fixed.
4. **Is ADR-0009 accepted as written?** It records the frontend-first sequencing
   and the contract+mock approach.
5. **Are ADRs 0002, 0005, 0006, 0007 and 0008 accepted as written?**

## Agent handover

- **Task:** Frontend build — complete. Dev server running at
  `http://localhost:5173/`.
- **Current state:** Frontend implemented and verified (typecheck, lint, 8/8 tests,
  token check, build, dev-server render). Backend does not exist; P4-08 open. Phase 0
  not started; CP-0 not satisfied.
- **Files changed:** see the commit `c86ffd4` and this session's uncommitted
  frontend work, now committed.
- **Commands run:** `npm install` (278 packages), `npm run typecheck` (pass),
  `npm run lint` (pass), `npm test` (8/8 pass), `npm run check:tokens` (0
  violations), `npm run build` (pass, 80 modules), `npm run dev` (running on 5173),
  browser tab opened and verified (no console errors, nav rail + hero + shelves
  render).
- **Tests:** PASS for typecheck, lint, 8/8 frontend tests, token check, build.
  **NOT RUN:** Rust, Tauri, E2E, contrast, dropped frames, keyboard walkthrough.
- **Uncommitted changes:** none — committed and pushed.
- **Known hazards:** the dev server is still running (stop with Ctrl+C). The
  frontend's data-dependent acceptance criteria are open until the backend lands.
  Inline styles in `WatchlistPage` and `DownloadsPage` should move into CSS modules
  in a follow-up.
- **Do not repeat:** `pids_limit` next to `deploy.resources.limits` (Compose v2
  rejects it); claiming WSL2 has no GPU access (it has compute access and `nvdec`
  works on NVIDIA); claiming a named volume is unreachable from Windows (it is
  reachable via the WSL share); treating mirrored networking mode as a harmless
  fallback (it exposes the published port to the LAN).
- **Next safe action:** test the frontend in the browser, then accept or revise
  ADR-0009 and the other Proposed ADRs, then start Phase 0 with P0-10 and P0-13.

---

## Backend foundation (2026-11-17)

The Rust backend foundation has been implemented per
`docs/superpowers/plans/2026-11-16-backend-foundation.md` (Tasks 1–6). The
`core` binary now serves the frontend's frozen command contract from SQLite
over an authenticated HTTP API, so the UI's data-dependent acceptance criteria
can finally be exercised.

### What shipped (commits `6a809f0`, `6322beb`, plus uncommitted Task 4–6 work)

**Task 1 — Workspace scaffold**
- `Cargo.toml` workspace root with members `app-core`, `media-persistence`
  (dir `crates/persistence`), `mediahaven-core` (dir `crates/core`).
- `rust-toolchain.toml` pins channel `1.99.0` with rustfmt + clippy.
- Crate manifests with workspace-inherited versions and dependency declarations.

**Task 2 — app-core domain and ports**
- `crates/app-core/src/domain/media.rs`: `MediaKind`, `MediaAvailability`,
  `MediaSummary`, `HomeScreen`, `MediaShelf`, `ShelfId`, `LibraryScreen`,
  `BrowseQuery`, `SearchQuery`, `SearchResults`, `AppErrorCode`, `AppError`
  with serde derives producing the exact camelCase JSON the frontend expects.
- `crates/app-core/src/ports/library.rs`: `LibraryRepository` trait.
- `crates/app-core/src/application/library.rs`: `LibraryService<R>` delegating
  to the repository.
- `deny_unknown_fields` added to `BrowseQuery`, `SearchQuery` and (later)
  `SettingsPatch`, `PlaybackStartRequest`, `PlaybackSettingsPatch`,
  `StorageSettingsPatch` per ADR-0005 §4.

**Task 3 — persistence (SQLite + migrations)**
- Migration `crates/persistence/migrations/0001_init.sql` with `media_items`,
  `media_files`, `library_roots`, `watchlist_items`, `queue_items` tables,
  foreign keys, indexes, UTC timestamps. `cast` column renamed to
  `cast_members` (SQL keyword collision); `watchlist_items.media_id` UNIQUE
  for idempotent add.
- `SqliteLibraryRepository` implementing `LibraryRepository`.
- 7/7 tests pass; clippy clean.

**Task 4 — core HTTP server**
- `crates/core/src/{state,auth,routes,server,lib,main}.rs`.
- `/healthz` (unauth) → `{"status":"ok"}`; `/readyz` (auth) → `{"status":"ready"}`.
- 20 named command routes mirroring the frontend contract.
- Auth middleware per ADR-0005 §2: Bearer token + HMAC-SHA256 signature over
  `timestamp\nMETHOD\npath\nhex(sha256(body))`, ±30 s window, constant-time
  compare. Dev token `dev-only-not-for-production` gated behind
  `MEDIAHAVEN_DEV=1`.
- Body limit 1 MiB; CORS loopback-only.
- `crates/core/tests/auth.rs`: 11 auth-matrix tests, all passing.

**Task 5 — Seed + frontend HTTP transport**
- `crates/core/src/seed.rs`: fixture data inserted when `MEDIAHAVEN_SEED=1`;
  `run()` calls `seed_if_requested`. Smoke verified Home returns
  `Continue Watching` + `Recently Added Movies` shelves.
- `apps/desktop/src/services/httpTransport.ts`: WebCrypto HMAC transport.
- `apps/desktop/src/services/transport.ts`: HTTP transport selected when
  `VITE_BACKEND_URL` is set; mock remains the default.
- `apps/desktop/src/vite-env.d.ts`: typed `VITE_BACKEND_URL` / `VITE_BACKEND_TOKEN`.
- Frontend: typecheck + build (82 modules) + `check:tokens` (0 violations) pass.

**Task 6 — Dockerfile for core**
- `infrastructure/containers/Dockerfile.core`: multi-stage (rust:1.99 builder →
  debian:bookworm-slim runtime), non-root UID 10001, healthcheck on `/healthz`.
- `.dockerignore` created.
- Docker smoke: image builds successfully; container starts and
  `curl http://127.0.0.1:18081/healthz` returns `{"status":"ok"}` (PASS).
  (Dockerfile builder base upgraded from `rust:1.86` to `rust:1.99` to
  satisfy dependency MSRVs.)

### Verification

| Check | Result | Notes |
|---|---|---|
| `cargo test --workspace` | **PASS** | All crate tests + 11 auth-matrix tests |
| `cargo clippy --workspace --all-targets -- -D warnings` | **PASS** | Clean |
| Frontend typecheck (`tsc --noEmit`) | **PASS** | Strict mode |
| Frontend build (`npm run build`) | **PASS** | 82 modules |
| Frontend token check (`check:tokens`) | **PASS** | 0 violations |
| Docker build (`Dockerfile.core`) | **PASS** | Image builds |
| Docker smoke (`/healthz`) | **PASS** | `{"status":"ok"}` |

### Not tested
- End-to-end browser smoke with `VITE_BACKEND_URL` set against the running
  backend (Task 5 Step 4) — not executed in this session.
- `cargo fmt --check` — not run (no CI yet).
- Docker container security posture (`docker inspect` assertions for non-root,
  cap_drop, no-new-privileges, read-only rootfs) — not asserted; image runs
  as UID 10001 per Dockerfile but inspect-level verification was not performed.

### Known hazards
- The Dockerfile's `MEDIAHAVEN_BIND=0.0.0.0:8080` binds all interfaces inside
  the container, which is expected for the container's published port but must
  not be used for a host-native run without `MEDIAHAVEN_DEV=1`.
- `Cargo.lock` was modified (new dependencies); ensure it is committed.
- Uncommitted design assets (`.workbuddy-ai/`, `.impeccable/`,
  `OTT Platform UI (Community)/`, `PRODUCT.md`, `docs/design/tv-ui/`) are
  out of backend scope and remain uncommitted.

### Next safe action
- Commit Task 4–6 changes.
- Update `roadmap.md` task statuses (P1-01, P1-05, P1-08 partially
  satisfied; backend foundation plan complete).
- Run end-to-end browser smoke with `VITE_BACKEND_URL` set to confirm the
  frontend renders seeded backend data.

---

## Static TV prototype redesign

`docs/design/tv-ui` was restyled as a separate, local-only design-review surface
for the supplied Home references. The prototype uses its existing seeded
catalogue and artwork; no reference screenshots, external content services, or
account UI were added. The React product app and roadmap phase status were not
changed.

### Verification

| Check | Result | Evidence |
|---|---|---|
| Home at 1920×1080 | **PASS** | Browser measured centered 795px navigation, 626px hero, Up Next at y=578, following shelf heading at y=915; screenshot inspected. |
| Home at 1280×720 | **PASS** | Browser measured centered 679px navigation, 502px hero, Up Next at y=470; screenshot inspected. At this height Up Next captions reach the lower viewport edge; following shelf is not expected to show. |
| Keyboard/remote navigation and focus | **PASS** | Arrow navigation, keyboard-visible focus ring, Movies overlay Escape restoration. |
| Discover/Search/details/player paths | **PASS** | Discover routing, Search for seeded Dune, title details, player entry, Escape restoration to details and prior Search result. |
| Reduced motion | **PASS** | Emulated `prefers-reduced-motion: reduce`; screen animation duration observed as 0.00001s. |
| Browser errors and local assets | **PASS** | No page/console errors or failed requests reported by the browser harness. |
| Account/sign-up UI scan | **PASS** | No matching copy in rendered page or prototype/product/surface brief. |
| JavaScript syntax and diff whitespace | **PASS** | `node --check` on the four prototype JS files; `git diff --check` on scoped files. |
| React app build / backend acceptance | **NOT RUN** | Out of scope; this change affects only the static prototype and its design documentation. |

The 1920×1080 composition now shows the next shelf heading and the top of its
artwork cards. At 1280×720 the hero and Up Next shelf remain visible, but the
next shelf does not appear in the first viewport. This is a visual reference
prototype only, not product-roadmap completion.

---

## Backend Phase 2 — library (partial)

### Completed this session

**media-library crate** (`crates/media-library`, new workspace member)
- `path_policy`: canonicalise root, symlink/reparse refusal, containment check,
  extension allowlist, title sanitisation (control chars dropped, truncation,
  fallback).
- `metadata`: size, extension, title from filename (no ffprobe; technical
  metadata deferred).
- `scanner`: iterative, depth-capped DFS; honours cancellation flag; skips
  locked/disappearing files; applies extension allowlist; computes SHA-256
  hash up to 1 GiB.

**Persistence**
- Migration `0002_library_roots_path_and_media_tracking.sql`: `path` on
  `library_roots`, `root_id` on `media_items`, `path` on `media_files`.
- `LibraryRepository` extended: `root_add`, `root_remove`, `root_update`,
  `media_replace_for_root`.
- `load_roots` now returns `path`.

**app-core**
- `LibraryRoot.path` field; `MediaUpsert` domain type.
- `LibraryService` methods for root management and media replacement.

**core HTTP**
- New routes: `/library_root_add`, `/library_root_remove`, `/library_scan`.
- `/library_scan` scans a root synchronously, persists results, updates root
  status/count, returns `{ scanned, cancelled, errors }`.

### Verification
| Check | Result |
|---|---|
| `cargo build --workspace` | PASS |
| `cargo test --workspace` | PASS (all) |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| media-library scanner tests (6) | PASS |

### Not tested
- End-to-end scan against a real folder via HTTP (not executed).
- Symlink/reparse-point refusal on Windows (unit-level containment tested,
  OS-level reparse not).
- Root removal file-count assertion (backend deletes rows only; no files
  touched by design).

---

## React route follow-up (2026-11-17)

Implemented and verified a focused route slice in `apps/desktop`:
- Added an Anime page and navigation entry; it includes only movies and shows
  explicitly tagged with the `Anime` genre and explains the empty state.
- Added routed Details pages with Back navigation to the prior location (or Home
  on direct entry). Player remains an overlay above Details.
- Added a direct Search navigation link while preserving the global search field.
- Added route regression tests for Search, Anime filtering/empty state, Details
  navigation, and Player overlay behavior; added jsdom scroll API shims used by
  the existing tests.

Verification from `apps/desktop`: `npm test -- --run` (13/13 passed),
`npm run typecheck`, `npm run lint`, `npm run check:tokens` (52 files, 0
violations), and `npm run build` all passed. `git diff --check` passed. The
initial npm invocation from the repository root failed because there is no root
`package.json`; rerunning from `apps/desktop` succeeded.

This is a partial frontend route slice, not completion of Phase 4: no visual
acceptance or full keyboard-only walkthrough was performed, backend-dependent
acceptance remains pending P4-08, and roadmap task statuses were not changed.
Unrelated Rust/backend and static-prototype work remains in the working tree and
was not altered by this route follow-up.

---

## Backend Phase 2 — library completion (2026-11-17)

Completed the remaining Phase 2 backend work per
`docs/superpowers/plans/2026-11-17-phase-2-backend-completion.md` (Tasks 1–6).
No frontend changes; no schema/migration changes.

### What changed

- **Task 1 — scanner stable id + kind** (`crates/media-library/src/scanner.rs`,
  `lib.rs`): `ScannedFile` now carries `id: String` (SHA-256 hex of the
  canonical absolute path) and `kind: MediaKind` (non-Option). Added
  `pub fn stable_id` and `pub fn classify_kind` (video + audio → Movie;
  artwork extensions → None → skipped). Kept the `is_media_extension`
  allowlist gate. `classify_kind` lowercases internally.
- **Task 2 — persistence upsert/prune** (`crates/persistence/src/sqlite/library_repo.rs`):
  `media_replace_for_root` now upserts by stable id (ON CONFLICT DO UPDATE),
  rejects empty ids up front, runs in one transaction, bulk-deletes previous
  `media_files` rows via subquery, and prunes stale ids in 500-id chunks.
  Fixes the cascade-delete bug where `Uuid::new_v4()` was substituted for
  empty ids, wiping watchlist/queue/progress rows on rescan.
- **Task 3 — `library_scan` rewiring** (`crates/core/src/routes.rs`): sets
  `id`/`kind` from scanned files; response now
  `{"scanned","cancelled","errors":[strings],"duplicates":[{"hash","paths"}]}`.
  Duplicates computed in the route via a borrowed `BTreeMap<&str, Vec<&str>>`
  over `&items`, groups with fewer than 2 paths dropped. `errors` are
  `"{path}: {message}"`.
- **Task 4 — scanner acceptance tests** (`crates/media-library/tests/roots.rs`,
  new): 8 tests — path escape, artwork skip, hostile filenames, vanishing file,
  cancel/rescan idempotency, duplicate hashes, unique 64-hex ids, empty root.
- **Task 5 — persistence acceptance tests** (`crates/persistence/tests/repository.rs`,
  +10): cascade preservation (watchlist/queue/progress/availability), prune,
  no media_files accumulation, empty-id rejection, LIKE metacharacter escaping,
  escape-clause behavioural proof, UTC round-trip, root-index EXPLAIN, and an
  `#[ignore]` 10k search budget test.
- **Task 6 — deviations + docs**: ADR-0010 (library entity deviations, proposed),
  ADR-0011 (metadata provider selection, proposed), README index rows, roadmap
  notes, this entry.

### Verification (observed)

| Check | Result |
|---|---|
| `cargo test -p media-library --test roots` | PASS (8/8) |
| `cargo clippy -p media-library --all-targets -- -D warnings` | PASS |
| `cargo test -p media-persistence` | PASS (14/14, 1 ignored) |
| `cargo test -p media-persistence -- --ignored` | PASS (1/1, 1.39 s wall) |
| `cargo clippy -p media-persistence --all-targets -- -D warnings` | PASS |

### Not tested (NOT RUN)

- Full workspace gate (re-run in this pass): `cargo build --workspace` PASS;
  `cargo test --workspace` PASS (all suites, 1 ignored); `cargo clippy --workspace
  --all-targets -- -D warnings` PASS; `cargo fmt --check` PASS.

---

## Architectural decisions recorded (2026-11-18)

Provisional approvals applied: ADR-0002 (mpv via JSON IPC, abstract interface, no libmpv in main app), ADR-0001 Part 2 (dedicated WSL2 + Docker Engine, no Docker Desktop), ADR-0006 (volume policy), ADR-0007 (secrets), ADR-0011 (TMDB primary + abstraction). O-01 (MEDIA_HAVEN internal name, branding configurable). CP-2 remains OPEN — no sign-off. ADR statuses updated to Accepted (provisional); README index updated. Roadmap P3-01 and P2-06 dependencies unblocked; P1-08 unblocked on runtime distribution. Implementation tasks now able to proceed: P1-08, P2-06 (crates/metadata), P3-01 (crates/playback with mpv JSON IPC).

---

## Backend Phase 3 — checkpoint persistence (2026-11-17)

Added a `playback_checkpoint` checkpoint path across the stack:
- `PlaybackCheckpointRequest` domain type (mediaId, positionSeconds, durationSeconds?).
- `LibraryRepository::playback_checkpoint` + `LibraryService::playback_checkpoint`.
- `/playback_checkpoint` route with finite/non-negative validation.
- Persistence impl writes progress_position/duration/updated_at transactionally;
  returns `MediaMissing` for unknown media.
- Test `playback_checkpoint_writes_progress` (persistence) verifies write + rejection.
Gates: persistence 15/15 (1 ignored), core 11/11, clippy clean. Player engine
integration and frontend command remain out of scope.

---

## Backend Phase 3 — completion/resume domain (2026-11-17)

Added pure playback decision logic to `app-core` (no player engine, no I/O):
`PlaybackConfig` (thresholds with documented defaults), `is_complete` and
`resume_position`. Tests (7) cover seek-to-end, mid-episode close, normal
completion, short watch, tiny/near-end resume skip, and resume return.
Files: `crates/app-core/src/domain/playback.rs`, `domain/mod.rs`, `lib.rs`.
Gates: `cargo test -p app-core` PASS, clippy PASS. Player integration,
persistence of checkpoints and P3-01 engine remain gated on CP-2/ADR-0002.
- End-to-end HTTP scan against a real folder (no curl smoke).
- Windows reparse-point refusal at OS level: `path_escape_is_refused` created
  the symlink successfully on this host, so the privileged-skip branch was NOT
  exercised; the assertion path ran instead.
- `cargo fmt --check` — PASS (run after `cargo fmt`; no diff).

### Risks / open decisions

- `collections` remains unimplemented (ADR-0010, proposed).
- Audio is stored as `Movie` (schema CHECK); no series synthesis.
- Stable ids derive from paths; moving a root resets per-item state.
- Metadata provider (P2-06) blocked on O-04; no artwork cache.
- No commit or push was made; working tree is dirty.

---

## Frontend redesign — Apple TV Liquid Glass (2026-11-18)

Human-directed full UI replacement after the Stitch/Tailwind attempts were
rejected. Implemented from the attached Apple TV-style brief: liquid glass,
tvOS focus behaviour, 60/80 pt safe insets, 40 pt gutters, generous row gaps.

### What changed

- Rebuilt `apps/desktop/src` on one token layer (`src/styles/tokens.css` plus
  `src/styles/base.css`). Tailwind utility markup, the Google-hosted fonts and
  all remote `aida-public` artwork URLs are gone. Space Grotesk and Roboto
  Condensed are self-hosted from `public/fonts`; the eight stills from
  `docs/design/tv-ui/assets` were copied to `public/artwork`.
- New primitives: floating glass `Nav`; auto-rotating full-bleed `Hero`;
  `Shelf` with centred large titles; `MediaCard` (16:9, resume progress);
  authored SVG `Icon` set; `Controls` (button, chip, progress, toggle,
  segmented); `Surfaces` (page header, glass panel, field row, empty state);
  `CategoryTile`.
- Screens: launch gate and profile selection (add/remove with confirmation),
  Home, Movies, TV Shows, Music, Anime, Library, My List, Search (live results,
  recent searches), Details (backdrop plus glass sheet, seasons/episodes/tracks,
  More Like This), Player (simulated transport, seek, skip intro, next episode,
  captions, full screen), Downloads (storage plus destructive remove behind an
  inline confirm), Settings (preferences persist), Setup (three-step wizard).
- Local state in `src/state/AppState.tsx` (profiles, My List, resume positions,
  preferences, downloads, recent searches) persisted to `localStorage` key
  `mediahaven.ui.v1`. No transport, no Rust commands, no network calls.
- Spatial focus in `src/hooks/useSpatialFocus.ts`: arrow keys move real DOM
  focus by geometry, Enter/Space activate, the player owns arrows while open.

### Verified this session

| Check | Result | Notes |
|---|---|---|
| `npx tsc --noEmit` | **PASS** | strict, including `exactOptionalPropertyTypes` |
| `npx eslint . --max-warnings 0` | **PASS** | no suppressions |
| `npm run check:tokens` | **PASS** | 49 UI files, 0 violations |
| `npm run build` | **PASS** | 85 modules; 338 kB JS / 69 kB CSS |
| `npx vitest run` | **PASS** | 2/2 (`src/App.test.tsx`: launch gate, profile entry) |
| Dev server | **PASS** | `http://localhost:5173/`, no console errors |
| DOM layout probes | **PASS** | no horizontal overflow at 739×700 or 1600×900; nav pill fully visible; hero CTA inside hero; secondary ink ≥ 4.5:1 on black |
| Manual visual review | **PARTIAL** | Search, Settings, Library, Downloads, Details and Player screens reviewed in the browser; Home capture was unreliable in the preview compositor; 1920×1080 not reviewed |

### Not verified / limitations

- Playback is simulated with artwork and a timed transport. There is no media
  engine, and captions exist only for the sample series.
- The tvOS Top Shelf extension is not implemented; this is a web build.
- No real library scan, no Rust command layer, no persistence beyond
  `localStorage`. Fixture catalogue lives in `src/data/catalog.ts`.
- Keyboard walkthrough across every screen was not completed; arrow focus was
  exercised on Home only, and the search field was exercised directly.
- Tailwind/PostCSS dev dependencies and their config files remain in the repo
  but are no longer imported. Removing them needs an approved `npm uninstall`.

### Risks

- Hero legibility depends on layered scrims; only the bundled artwork has been
  checked.
- Eight local stills are reused across rows; real imports will change the
  composition and may need wider artwork handling.
- A regression test suite for the new components does not exist yet beyond the
  two shell smoke tests.

### Next recommended action

Review the rebuilt UI at 1920×1080 on the TV, then either (a) restore the typed
command contract and mock transport as the data seam for P4-08, or (b) add
checkpoint coverage for this redesign to `roadmap.md` before more UI work.

---

## UI consolidation and backend wiring (2026-11-18)

### Removed

- `Sitch final coalation, all ui files/` (untracked Stitch bundle).
- `docs/design/tv-ui/` (static prototype, its copy of the artwork and tools).
- `docs/superpowers/plans/2026-11-17-static-tv-ui-redesign.md` and
  `2026-11-18-stitch-cinematic-ui-conversion.md`.
- The Music screen, album/track UI and the Music nav tab: the schema only
  stores movie/series/episode.
- Tailwind, PostCSS and Autoprefixer dev dependencies and both config files;
  the UI is CSS Modules over the token system.

### Wired to the real backend

- `apps/desktop/src/services/api.ts` — one signed client for every named
  operation (HMAC-SHA256 over timestamp, method, path and body; ADR-0005).
  `/readyz` is a signed GET, everything else a signed POST. No fixture or mock
  fallback exists.
- `apps/desktop/src/services/types.ts` mirrors the Rust DTOs;
  `services/adapt.ts` maps them to the view model and drops fields the backend
  does not have (rating, match, tagline).
- `state/AppState.tsx` loads `home`, `movies`, `shows`, `watchlist`, `roots`,
  `settings` and `downloads` after `/readyz`, exposes mutations, and keeps only
  UI preferences (reduce motion, skip intro, recent searches, active profile)
  in `localStorage`.
- Screens now read that snapshot: Home (hero + backend shelves + My List),
  Movies, TV Shows, Anime (genre filter), Details (detail + per-season
  episodes + watchlist), Player (`playback_start`, checkpoint every 5s and on
  exit), Search (debounced `library_search`), Library (roots/counts), Watchlist,
  Profiles (backend settings document), Settings (playback + diagnostics),
  Setup (root add + real scan), Downloads (read-only backend state).
- The app gates on `/readyz`: loading screen, or an explicit offline panel with
  the backend origin, the command to start it and a Retry button. It never shows
  invented titles when the service is down.
- `crates/core/src/seed.rs` expanded to 5 films, 3 series, 11 episodes,
  watchlist/queue entries and 3 profiles for the three seeded accounts, and now
  updates the sample root's item count.
- Fixed a resume bug found by the wiring: `playback_start` with a null start
  position used to overwrite the stored checkpoint with zero. It now falls back
  to `progress_position`; `repository.rs` asserts reopening resumes at 120 s.
- `SettingsPatch` in the persistence test was missing the `profiles` field
  (compile failure); the test now sets `profiles: None`.
- Docs: `docs/design/design-system.md` rewritten for the shipped tokens and
  components; `apps/desktop/README.md` replaced with real run instructions, the
  data flow and the honest limits. `docs/design/navigation-spec.md` still
  describes the focus model accurately.

### Verified

| Check | Result |
|---|---|
| `cargo fmt --check` | **PASS** (after `cargo fmt`) |
| `cargo clippy --workspace --all-targets -- -D warnings` | **PASS** |
| `cargo test --workspace` | **PASS** (run with `CARGO_TARGET_DIR=target/test`, see risks) |
| `npx tsc --noEmit` | **PASS** |
| `npx eslint . --max-warnings 0` | **PASS** |
| `npm run check:tokens` | **PASS** (53 files, 0 violations) |
| `npx vitest run` | **PASS** 2/2 (launch gate, profile entry) against a mocked API |
| `npm run build` | **PASS** (338 kB JS / 74 kB CSS) |
| Browser against live service | **PASS** — `/readyz` + seven boot operations 200; profiles, hero, shelves and My List from SQLite; details + season switch; no console errors; no horizontal overflow at 748 px |
| Playback persistence | **PASS** — reopened at the stored 15:00 mark, checkpoint wrote 920 s, read back through `library_episodes` |
| Watchlist round-trip | **PASS** — remove + add returned 200 and the list changed in the UI and on disk |
| Settings write | **PASS** — autoplay off via the UI read back as `false` from `/settings_get`, then restored |
| Search | **PASS** — “dune” returned exactly one result from `library_search` |
| Scan flow | **PASS** — `library_root_add` → `library_scan` (0 files, empty temp dir) → `library_root_remove`, all 200 |
| Startup count | **PASS** — all seven requests answered after a cold backend restart |

### Not verified / limitations

- Video playback is still simulated (4× clock over artwork). Sessions and
  checkpoints are real; there is no media engine.
- Downloads are read-only: the backend reports `enabled: false` and exposes no
  mutation, so the screen shows the disabled state.
- No music surface (schema has no album kind).
- Artwork still comes from the bundled local set; `ArtworkRef` is always empty
  until a metadata provider lands (ADR-0011).
- Per-profile isolation does not exist: profiles are stored, but progress and
  the watchlist are install-wide.
- Queue endpoints exist but no screen consumes them.
- Profile add/remove via the UI was not exercised (the provider path is the same
  `settings_set` call verified for playback settings).
- Visual review at 1920×1080 was not repeated after wiring; checks were DOM
  geometry and the preview at ~750 px.
- The preview click helper resolves stale coordinates in this panel, so several
  interactions were driven with DOM `.click()` rather than a real pointer.

### Risks

- The production token path is not implemented: there is still no Tauri shell
  to inject `VITE_MEDIAHAVEN_TOKEN`, and the dev fallback must never ship.
- CORS accepts loopback origins only, by design.
- The seed uses real film/TV titles with synthetic metadata; it is dev data
  behind `MEDIAHAVEN_SEED=1`.
- On this machine the MSVC linker intermittently fails with `LNK1104` when
  relinking test binaries in `target/debug` (file lock, likely AV). Using
  `CARGO_TARGET_DIR=target/test` works; normal builds are unaffected.
- `crates/core/src/seed.rs` is now the only source of demo content; deleting
  `.mediahaven-dev/` and re-running with `MEDIAHAVEN_SEED=1` rebuilds it.

### Running it

```sh
MEDIAHAVEN_DEV=1 MEDIAHAVEN_SEED=1 \
MEDIAHAVEN_DATA_DIR="$PWD/.mediahaven-dev" \
cargo run -p mediahaven-core      # 127.0.0.1:8080

cd apps/desktop && npm run dev    # 127.0.0.1:5173
```

## 2026-11-18 — Profile colours are real values, assigned by the backend

- `settings_set`/`settings_get` run `normalise_profiles`: a stored hex is
  lower-cased and kept; anything else (the empty string the UI sends, or an
  older non-colour value) is assigned from `PROFILE_PALETTE` by position. The
  palette mirrors the `--tone-*` tokens, so the design is unchanged. There is
  no format-specific migration: the UI can only produce hex, so no other
  format needs special handling.
- The UI no longer builds or sends CSS variables: profile creation posts
  `avatarColor: ""`. No colour literal exists in the UI source, and
  `check:tokens` still reports 53 files, 0 violations.
- Regression test `profiles_store_real_colours_not_css_variables` covers
  assignment, replacement of an invalid value and hex normalisation.
- Review pass: dropped the `var(--tone-N)` parsing and the UI's acceptance of
  that format, since the UI can no longer produce it; an unrecognised value is
  simply treated like an empty one. Palette assignment is positional, so a
  profile's stored hex is never recomputed once written.

Verified: `cargo fmt --check`, `cargo clippy --workspace --all-targets`,
`cargo test -p media-persistence profiles_store_real_colours`, `tsc`, eslint,
`check:tokens`, vitest (2/2). Against the live API: a profile posted with `""`
read back `#82755f`; an invalid colour was replaced with a palette value; the
original three profiles were restored unchanged.

Verified through the dialog in the running app: "Add profile" → name "Verify" →
"Create profile" stored `#82755f` in the backend settings, the new card rendered,
and "Manage profiles" → "Remove Verify" → "Remove profile" left exactly the
original Alex/Sam/Kids profiles (`#7e93a8`, `#a88e7e`, `#8aa88a`) with no console
errors; the active profile was reselected as Alex afterwards.

Not verified: visual review of the assigned avatar colours beyond the created
card rendering in the profile grid.

## 2026-11-18 — Backend completion master plan (planning only)

No implementation in this session. Plan written to
`docs/superpowers/plans/2026-11-18-backend-completion.md`.

What the audit found, stated plainly because earlier sections of this file are
stale:

- The Rust backend **does exist** and is wired to the frontend: 25 protected
  routes plus `/healthz`, SQLite persistence, scanner, watchlist, queue and
  settings. Earlier text in this file that says "the backend does not yet
  exist" / "No Rust, no database" is superseded.
- It runs on the **host** (`cargo run -p mediahaven-core`), not inside the
  mandatory Docker boundary. `infrastructure/containers/` holds artefacts that
  have never been run.
- `apps/desktop/src-tauri/` does not exist, so a production build has no token
  source and every request would fail with `missing_token`. The renderer talks
  HTTP directly with a development token, which ADR-0005 does not allow as the
  shipped shape.
- Playback is simulated in the UI (`SIMULATION_RATE = 4`); there is no engine
  and no stream route. Queue routes have no UI. Downloads are read-only.
- Document defect fixed 2026-11-18: ADR-0011's status line read "Accepted
  (provisional) — TMDB primary" while its Decision section said no provider was
  chosen. Both now say **Proposed, no provider approved**; TMDB is recorded as
  the recommended candidate only. The index row, the README open-decisions
  paragraph and the P2-06 dependency line were corrected to match. Nothing was
  marked accepted. (Superseded the same day: the human partner then accepted
  TMDB — see "O-04 accepted" below. This entry records only what was true before
  that answer.)
- Superseded: the earlier line in this file claiming provisional approval of
  "ADR-0011 (TMDB primary + abstraction)" does not describe a human decision
  and conflicts with the ADR body; treat it as an error, not as approval.
- Still open: `roadmap.md`'s current milestone describes Phase 0 with no
  backend. It was left untouched because changing a milestone is a human
  decision, and it is listed as a gate in the plan.

Verified this session: nothing new was built; the plan's own claims about
existing routes were re-checked earlier in the session by calling every read
path through the app's signed client (all answered). No metrics are claimed.

Not done: no roadmap status changed, no ADR accepted, no dependency installed,
no code written.

## 2026-11-18 — O-04 accepted: TMDB metadata provider

Documentation and decision only; no code, no dependency, no network call.

- The human partner accepted TMDB when asked to choose between "accept TMDB
  (with local NFO fallback)" and "local NFO only", answering "i have no problem
  with tmdb". O-04 is therefore **accepted**, not inferred by the agent.
- `docs/decisions/0011-metadata-provider-selection.md` is now `Accepted`
  (2026-11-18). It gained an acceptance record naming the human partner as the
  decider, and the Decision section now states the actual decision: TMDB primary,
  local NFO offline fallback, behind a `MetadataProvider` port. The obligations
  are written into the decision rather than left implicit: network use opt-in and
  off by default, only title/year sent, attribution rendered, key from the
  secret store, cache size cap with eviction, artwork sanitised.
- `docs/decisions/README.md`: index row is `Accepted`; the metadata provider was
  removed from the open-decisions paragraph.
- `roadmap.md`: P2-06 dependency line now points at the accepted ADR instead of
  an open choice; the O-04 register row records the acceptance and its
  conditions.
- `docs/superpowers/plans/2026-11-18-backend-completion.md`: decision D-3 marked
  resolved, D1 marked COMPLETE, D3 and D4 unblocked, dependency graph and the
  WS-D preamble updated. D2 (local NFO) still lands first so the offline path
  exists before egress.

Still open and unchanged: D-1 (container-first vs host-first sequencing),
D-2/O-06/O-07 (runtime distribution), D-4/O-05 (downloads legal sign-off),
D-5/O-09 (mpv licence), D-7 (token storage mechanism). No task from the plan has
started. Nothing was committed.

Verified: nothing executable changed, so no test run. The four documents were
re-read after editing to confirm the status, the decision and the roadmap all say
the same thing; that check was by inspection, not by a script.

## 2026-11-18 — O-04 accepted, remaining gates answered, Task D2 implemented

### Decisions recorded (all by the human partner, not the agent)

Asked one question per open gate; the answers:

| Gate | Answer | Landed in |
|---|---|---|
| D-1 sequencing | **Containerise first** | ADR-0013 (Accepted) |
| D-2 / O-06 / O-07 | **Dedicated WSL2 distro + Docker Engine; no reuse of the existing engine** | ADR-0001 Part 2 acceptance record |
| D-5 / O-09 | **LGPL mpv build** | ADR-0002 acceptance record |
| D-7 | **DPAPI via the `windows` crate; no `keyring`** | ADR-0014 (Accepted), amends ADR-0007 for the token |
| D-6 autoplay | **Off by default; Up Next offers the next episode** | Plan D-6 / C3 |
| D-4 / O-05 downloads | **Whole workstream deferred out of v1** | Plan WS-E marked deferred; roadmap O-05 |
| O-02 | **Windows 11 x64 only** | Development.md platform line; roadmap O-02 |
| O-01 naming | **MEDIA HAVEN is the project and app name** | Roadmap O-01 |

Evidence gathered for the runtime question, read-only: `docker --version` reports
29.9.3, `C:\\Program Files\\Docker` does not exist, and WSL lists a stopped
`docker-desktop` distro plus `OmniRushBox`. The machine therefore has *an* engine,
which is why O-07 needed an explicit answer.

### Task D2 — local NFO metadata (implemented)

New crate `crates/metadata` with:

- `NfoMetadataProvider` implementing a new `MetadataProvider` port
  (`crates/app-core/src/ports/metadata.rs`). One method, `lookup(path) ->
  Option<MetadataRecord>`; `Ok(None)` means no sidecar, `Err` means one was
  refused.
- A hand-written, bounded parser (`nfo.rs`): reads `<stem>.nfo` then `movie.nfo`
  beside the file, refuses >256 KiB, any `<!DOCTYPE>`/`<!ENTITY>`, non-UTF-8,
  control characters and unterminated elements; decodes only the five XML
  entities plus numeric references; caps title (512), synopsis (4000), genre
  (64 × 32) and drops impossible years/runtimes. No new dependency, no network,
  no writes, no symlink following.
- `MediaUpsert.metadata: Option<MetadataRecord>`; the scanner fills it, the
  routes let a sidecar title win over the filename, and
  `media_replace_for_root` writes year/synopsis/runtime/genres with `COALESCE`
  so a rescan without a sidecar cannot blank stored values.

### Commands and actual results

- `cargo fmt --all` — exit 0.
- `cargo clippy --workspace --all-targets -- -D warnings` — exit 0, no warnings.
- `cargo test -p media-metadata` — **8 passed**, 0 failed.
- `cargo test -p media-library` — **17 passed** across scanner/roots suites, 0
  failed.
- `cargo test -p media-persistence` — **17 passed, 1 ignored** (the pre-existing
  `searching_10k_items_is_within_budget`), including the new
  `scanned_sidecar_metadata_is_stored_and_survives_a_bare_rescan`.
- Linker note: the first two persistence runs failed with `LNK1104` on the test
  `.exe` in `target/review-test`; a fresh `CARGO_TARGET_DIR=target/review-persist`
  linked and passed. This is the known Windows file-lock issue, not a code
  failure.

### Not tested / not done

- No end-to-end check through the running app or UI: the scanner route and API
  were exercised only at crate level. No fixture library was scanned in the live
  app, so "NFO metadata appears on screen" is **NOT RUN**.
- Frontend unchanged; no TS typecheck or Vitest run was needed for this change.
- TMDB client, artwork cache, UI opt-in and egress flag are **not** started —
  those are D3/D4.
- Nothing committed.

### Task A2 — Tauri host scaffold (implemented)

`apps/desktop/src-tauri/` now exists:

- `Cargo.toml` — package `mediahaven-desktop`, lib target
  `mediahaven_desktop_lib`, workspace member; `tauri` 2.12.7 and `tauri-build`
  2.7.6 resolved through the workspace dependency table.
- `tauri.conf.json` — window titled `MEDIA_HAVEN` (1280×720, min 960×540),
  `devUrl` `http://localhost:5173`, `frontendDist` `../dist`, a CSP that allows
  only self, the local artwork protocol and the loopback API.
- `capabilities/default.json` — the `main` window gets `core:default` only. No
  filesystem, shell, dialog or HTTP plugin permission is granted.
- `src/lib.rs` — `get_app_info()` returning name, version, OS and arch for the
  smoke test; one unit test asserts the platform and version.
- `src/main.rs` — binary entry point, `windows_subsystem = "windows"` in
  release builds.
- `icons/` — generated from `apps/desktop/app-icon.png` (1024², monochrome
  play mark) with `npx tauri icon`; the source PNG is kept. Android/iOS icon
  folders were deleted: this product is Windows-only (O-02).
- `apps/desktop/package.json` gains `@tauri-apps/cli` 2.12.5 as a dev dependency
  and the `tauri` / `tauri:dev` scripts.

### Commands and actual results

- `npm install` in `apps/desktop` — exit 0, 2 packages added.
- `npx tauri --version` — `tauri-cli 2.12.5`.
- `cargo check -p mediahaven-desktop --all-targets` — exit 0.
- `cargo clippy --workspace --all-targets -- -D warnings` — exit 0.
- `cargo fmt --all` — exit 0.
- `cargo test -p mediahaven-desktop` — 1 passed (`app_info_reports_this_platform`).
- `cargo build -p mediahaven-desktop` — exit 0, `target/debug/mediahaven-desktop.exe`.
- Frontend untouched but checked: `typecheck` 0, `lint` 0, `check:tokens` 0
  violations across 53 files, `build` 0.
- Launch, against the Vite dev server already listening on `localhost:5173`:
  the process appeared with `MainWindowTitle: MEDIA_HAVEN`, `Responding: True`,
  ~29 MB working set, an `msedgewebview2.exe` child, and three established
  connections from that WebView to `127.0.0.1:8080` — the UI script ran and
  called the API, so the window loaded the real interface, not an error page.
  Stopping the app left **0** `mediahaven-desktop` processes. No
  `mediahaven-core` or `mpv` child was started by the host.

### Not tested / not done

- No screenshot or visual inspection: the window was observed through its
  title, responsiveness and the WebView's live API connections, **not** by
  looking at it.
- Cold start to first paint was **NOT measured**. The plan asks for a recorded
  number; the ~15 s pause above is a wait, not a measurement.
- The renderer still speaks HTTP directly to `127.0.0.1:8080` with the
  development token. Moving it behind named Tauri commands is A5/A7 and is
  **not** done; the CSP therefore still allows that loopback origin.
- The build was a debug `cargo build`, not `tauri build` or a bundle: signing,
  installer and `bundle.active` are out of scope here (WS-G).
- No supervisor, no token, no WSL2/Compose work: that is A3 and A4.

### Next

Task A3 (per-install token via DPAPI, ADR-0014) or A4 (supervisor lifecycle).
A3 must land before any command surface forwards to a containerised backend.

---

## 2026-11-18 — Tasks A3 and A4 implemented

### Task A3 — per-install token provisioning (ADR-0014)

New `apps/desktop/src-tauri/src/secret.rs`:

- `SecretToken` holds 32 bytes, deliberately **not** `Debug` and **not**
  `Serialize`, and zeroises its buffer on `Drop`. `as_hex()` returns a fresh
  bearer string; `as_bytes()` is read-only.
- `load_or_create_token(service, account)`: generate 256 bits from the OS CSPRNG
  (`BCryptGenRandom`), protect with DPAPI (`CryptProtectData`) under the current
  user, write to `%APPDATA%/MEDIA_HAVEN/<service>-<account>.blob` atomically
  (temp file + rename). Later calls unprotect and return the **same** token.
- Fail closed: a corrupt/unprotectable blob yields `SecretError::CredentialMismatch`
  and is never silently regenerated (R-25 — regenerating would orphan the
  running container's token).
- No `keyring`, no new dependency beyond the `windows` crate (ADR-0014). The
  `windows` crate was added to the workspace dependency table at version 0.62
  with the `Win32_Security_Cryptography` and `Win32_Foundation` features, and
  to the desktop crate; `thiserror` was added to the desktop crate for error
  types.

### Task A4 — supervisor lifecycle

New `apps/desktop/src-tauri/src/wsl.rs` and `supervisor.rs`:

- `Wsl::run(distro, args, timeout)` invokes `wsl.exe -d <distro> …` with
  **argument vectors only** — never a shell string, never `docker.exe` on
  Windows, no TCP daemon endpoint (ADR-0005 §3). A hard deadline with
  `try_wait` + kill covers a hung `wsl.exe`.
- `decode_output()` handles the real-world detail that `wsl.exe` writes UTF-16LE
  to **stdout**; reading only stderr would lose the whole error.
- `Supervisor::{ensure_running, wait_ready, stop, status}`: `compose up -d --wait`
  (30 s), loopback-port readiness poll (10 s, 250 ms backoff), `compose down`,
  and a live `SupervisorStatus` derived from the port. Failures map to the
  legible states in ADR-0005 §5 with user-safe messages (no path/token leak).
- The authenticated `/readyz` check is explicitly left to A5/A7, which owns
  HMAC signing; building it twice would violate ADR-0013's one-migration goal.

### Commands and actual results

- `cargo fmt --all` — exit 0.
- `cargo check -p mediahaven-desktop --all-targets` — exit 0.
- `cargo clippy -p mediahaven-desktop --all-targets -- -D warnings` — exit 0,
  no warnings (after fixing two clippy lints in the UTF-16 decoder).
- `cargo test -p mediahaven-desktop` — **12 passed, 0 failed**. New tests:
  token stable across calls, 256-bit hex shape, corrupt blob fails closed,
  non-Debug/redaction contract, drop path, UTF-16/UTF-8 decoding, nonexistent
  distro is a legible failure, closed-port NotReady/Stopped, and no path/token
  in status messages.

### Not tested / not done

- The DPAPI blob was exercised on this Windows host (round-trip, corrupt-blob,
  second-call). A **second Windows user** was not used to prove cross-profile
  unprotection fails — ADR-0014 requires that check and it is **NOT RUN**.
- No live `wsl … docker compose up` cycle: no compose file was rendered for the
  dedicated distro and the engine was not started (would need consent and A6).
  `wait_ready` was tested against a deliberately closed port only.
- `lib.rs` now declares the three modules and re-exports their public types, but
  no Tauri `invoke` command uses the token/supervisor yet — the window still
  talks HTTP directly. That is A5/A7.
- Nothing committed; the working tree's other uncommitted files are pre-existing.

### Review of A3/A4 and fixes (same session)

A read-only review found real defects; the cheap, verifiable ones were fixed and
re-verified. Remaining items are coupled to later tasks and are recorded, not
silently dropped:

Fixed and re-tested (17 desktop tests pass, clippy clean, fmt clean):
- `APPDATA` absent no longer falls back to `.` — `blob_path` now fails closed
  with `profile_unavailable`, so a token can never be written into the repo.
  Service/account are charset-validated so they cannot escape the blob dir.
- Zeroisation on drop now uses `write_volatile` + `compiler_fence` (a plain loop
  is a dead store LLVM may elide), and every error path zeroes its plaintext
  buffer before returning.
- `CRYPTPROTECT_UI_FORBIDDEN` set (no DPAPI prompt in a headless process);
  null/zero-length output blob guarded before `from_raw_parts`.
- Blob write fsyncs, cleans up its temp file on failure.
- The misleading no-op "redaction" test was replaced with a source-level guard
  proving `SecretToken` has no `Debug`/`Serialize` derive or manual impl.
- `compose up` is now **detached** (`-d`, no `--wait`): `--wait` blocks until the
  healthcheck's 40 s `start_period`, which exceeds this task's 30 s hard kill.
  Readiness stays in `wait_ready` (ADR-0005 §6).
- `stop()` failures now map to `stop_failed`, not `start_failed`.
- Engine output is redacted (32+ hex-char runs → `[redacted]`) before it can
  reach a user-facing state, pre-empting a future env-echo leak.

Deferred, with reasons (not claims of PASS):
- **C1 — token is not passed to `core`.** `compose.yaml` requires four vars
  including `MEDIAHAVEN_API_TOKEN`; `ensure_running` sets none, so a real
  `up` would fail. Injecting the token must happen without placing it in `argv`
  (process list) or a file — that is A6/A9's rendered compose override plus a
  `WSLENV`/in-distro env path, and needs engine consent. The token and supervisor
  are therefore correctly separated but not yet connected. **NOT RUN.**
- **C2 fully** — `wait_ready` still checks a TCP connect, not an authenticated
  `/readyz`; the signed probe belongs to A5/A7 which owns HMAC signing.
- **I3 — pipe draining.** `wsl.exe` stdout/stderr are not drained until exit;
  a very chatty child could fill the pipe and block until the timeout. The
  three commands used here emit little output; accepted for now, revisit if a
  compose error is observed to hang.
- **Second-Windows-user cross-profile DPAPI test** and any live `docker compose`
  cycle: need a second profile / engine consent. **NOT RUN.**

Research handoffs received: the A5 route map (24 real commands, no phantoms;
`/healthz`+`/readyz` are probes to exclude from parity; exact signing string and
12 error codes) and the A6 readiness report (Dockerfile.core likely broken since
A2 added the Tauri workspace member that `.dockerignore` excludes; no digests;
no Dockerfile.probe; engine consent required).

### Task A5 — named command surface (implemented and independently verified)

New `crates/contract` (`mediahaven-contract`) is the single source for the 24
renderer commands, the `Command` enum, the exact signing primitives
(`body_digest`/`canonical_message`/`sign`) and a `ScanOutcome` struct pinning the
`library_scan` inline-JSON shape. `apps/desktop/src-tauri/src/commands/` holds
all 24 `#[tauri::command]` functions plus a signed loopback `TcpStream`
forwarder; `lib.rs` registers all of them and no passthrough. Errors map to the
closed 12-code `AppErrorCode` set with fixed messages — backend text never
crosses the boundary.

New `apps/desktop/src/services/generated.ts` holds the `CommandName` union and
the 31 mirrored DTOs, marked as generated from `crates/contract`; automated
codegen is explicitly deferred to a future ADR (no codegen dependency added).
`api.ts`/`main.tsx`/`transport.ts` are untouched — that is A7.

Verification done this turn (not just reported by the implementing agent):
- `cargo fmt --all -- --check` → 0.
- `cargo clippy -p mediahaven-contract -p mediahaven-desktop --all-targets
  -- -D warnings` → 0, no warnings.
- `cargo test -p mediahaven-contract -p mediahaven-desktop` → contract 4/4,
  desktop 17/17.
- HMAC fixed vector in `parity.rs` re-computed independently with Python and
  matched byte-for-byte.
- TS type parity checked by script: all 31 `types.ts` exports present in
  `generated.ts`, plus `CommandName`.
- Confirmed from `crates/core/src/state.rs`/`server.rs` that the backend keys
  HMAC with the token **string** bytes, so the forwarder signing
  `bearer.as_bytes()` is correct — provided A6 injects the exact `as_hex()`
  string into `MEDIAHAVEN_API_TOKEN`.
- Parity test asserts the command list equals the 24 real renderer routes,
  forbids `http_request`/`api_request`/`docker_*`/probes, and that the enum
  round-trips with no duplicates.

Not done (out of A5 scope): A7 renderer transport swap, automated TS codegen,
a Tauri-builder "unknown command rejected" test, chunked-response decoding.

### Task A7 — renderer transport swap (implemented and verified)

The renderer now reaches the backend only through named Tauri commands in the
shipped host; the signed HTTP client is a dev-only fallback.
- New `services/errors.ts` (apiBase/ApiFailure/describeError, moved verbatim),
  `services/devClient.ts` (the only home of the dev token and `fetch`),
  `services/transport.ts` (`call()` — Tauri global → DEV-guarded dynamic import
  → `backend_unreachable`), and `services/transport.test.ts` (4 tests).
- `services/api.ts` is now a thin facade with the same method names and return
  types; each method calls a named command with snake_case outer args (Rust
  parameter names) and camelCase inner DTOs. `ready()` uses `settings_get`
  because `/readyz` is deliberately not a named command (A5).
- New `scripts/check-bundle.mjs` + `check:bundle` script. It scans only `.js`
  chunks (not sourcemaps) and asserts the dev token and HMAC signing markers
  are absent. **A subagent had changed `vite.config.ts` (disabled the preload
  polyfill, stripped sourcemap sources) to pass an over-broad `fetch(` grep;
  that was reverted** — changing production behaviour to satisfy a test is
  backwards. The narrow check proves the same thing without touching the build.

### A5 forwarder review — one real defect fixed

A read-only review of `commands/renderer.rs` confirmed all 24 request bodies
and the HMAC math are correct, but found the error mapping was by HTTP status
alone: 404 covers both `not_found` and `media_missing`, and 503 covers five
codes with different retryability. The forwarder now deserializes the backend's
exact `code` from the JSON body and falls back to the status map only if the
body is unparseable. New test `backend_code_is_preferred_over_status`. Backend
message text is still discarded (fixed messages only).

Verification this turn (all exit 0):
- Frontend: typecheck, lint, test (6/6), check:tokens, build, check:bundle.
- Rust: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo test --workspace` (desktop now 18 tests).

### A6 — file-level work done (build itself NOT RUN, engine consent pending)

- `Dockerfile.core`: now copies the Tauri member manifest so Cargo can load the
  workspace. **The subagent's first fix (manifest only) was proven wrong by a
  local simulation**: Cargo also resolves the declared lib target path at
  manifest-load time and failed with "can't find library mediahaven_desktop_lib".
  Fixed by adding `infrastructure/containers/desktop-lib-stub.rs`, copied to
  `apps/desktop/src-tauri/src/lib.rs` in the image. `.dockerignore` un-ignores
  the one manifest path.
- `Dockerfile.probe` (new): ffmpeg-builder stage → distroless `cc-debian12:nonroot`
  runtime with only ffprobe/ffmpeg + their `*.so*`, `USER 10002:10002`, no shell,
  no apt, no healthcheck, no secrets. Distroless `cc` (not `static`) because
  Debian's ffprobe is dynamically linked — the subagent corrected this; both
  base tags still need digests recorded at build time.
- Verified WITHOUT the engine: `cargo metadata --no-deps --offline` (exit 0) and
  `cargo build --release -p mediahaven-core --offline` (exit 0, 1m06s) against a
  reconstructed build context containing only root Cargo.toml/lock, `crates/`,
  the copied Tauri manifest and the lib stub. This proves the Dockerfile's
  copy set is sufficient. `docker compose config --quiet` with no env → exit 1
  with the four "required variable" errors (fail-closed, as intended).
- NOT RUN: `docker build`, digest recording, `docker inspect`, probe smoke —
  all need engine consent. Base-image digests, the `.dockerignore` negation
  against a real context, and distroless library completeness remain build-time
  gates.

### Task C1 — token injection into core (implemented, verified structurally)

The DPAPI token now reaches `core` at `compose up`, closing the 401 blocker —
in code, not yet on a live engine.
- `secret.rs`: `TOKEN_SERVICE`/`TOKEN_ACCOUNT` ("mediahaven"/"core-api") and
  `install_token()` are the single identity; `renderer.rs` deleted its private
  copies and imports them, so forwarder and supervisor cannot drift.
- `wsl.rs`: `run_with_env()` sets child-only env via `Command::env()` (no
  `std::env::set_var`, no global mutation); `run()` is a zero-env wrapper.
- `supervisor.rs`: `ensure_running()` loads the token, passes
  `MEDIAHAVEN_API_TOKEN=<as_hex()>` plus `WSLENV=MEDIAHAVEN_API_TOKEN` in the
  child env. The token is **never in argv** (not visible in the process list).
  `as_hex()` is an unzeroised `String` — documented `shortcut:` (upgrade if a
  zeroising hex buffer is added).
- Verified without an engine: 21 desktop tests (incl. token-in-env / not-in-
  argv / no-global-env-mutation, shared-identity source guard, env-vector
  shape), `cargo clippy --workspace --all-targets -- -D warnings` (0),
  `cargo fmt --all`.

NOT RUN: that `wsl.exe` actually forwards `MEDIAHAVEN_API_TOKEN` into the
distro and that `${MEDIAHAVEN_API_TOKEN}` resolves in compose. That is the
engine-side validation. Known WSLENV caveat: a value exported inside the distro
overrides the injected one — must be checked before relying on it.

### Next

A6 engine build (needs explicit consent) → A8 boundary evidence → A9 mount
rendering. The code path is complete; only the live engine validation remains.
