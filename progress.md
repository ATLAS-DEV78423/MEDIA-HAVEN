# Progress

## Last updated

2026-11-15 (session: frontend build + cinematic design system)

## Current state

The repository holds the approved engineering specification (`Development.md`), the
checkpoint-driven roadmap, the documentation baseline and the agent tooling.

**The frontend is now built.** `apps/desktop` is a runnable Vite + React +
TypeScript application implementing the Apple TV-style interface from
`Development.md` §7.4, against a frozen typed command contract
(`src/types/media.ts`) and a development-only mock transport
(`src/services/mock/`). It runs in a browser at `http://localhost:5173/` via
`npm run dev`. The backend does not yet exist; the frontend's data-dependent
acceptance criteria remain open until the backend lands (P4-08).

**No Rust, no Tauri shell, no database.** The host↔container boundary (P1-08) is
still a design. The mock transport returns fixture objects and implements no
policy, so the mandatory container boundary (ADR-0001) is not bypassed.

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
