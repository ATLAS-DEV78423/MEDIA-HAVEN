# Backend Completion Master Plan

> **For the implementing agent:** this is a plan, not approval. Every task is
> small enough to finish and verify in one sitting. Tasks are executed in the
> order given by Dependency graph; do not start a workstream whose gate is open.
> After each task, record what actually happened in `progress.md` — PASS, FAIL
> or NOT RUN, never a guess. Read `AGENTS.md`, `Development.md`, `roadmap.md`
> and `progress.md` before the first task, in that order.

**Status:** draft for human review — no task in this plan has been executed.

## Goal

Take the backend from "runs on this laptop under `cargo run`, all reads work,
playback is simulated" to the v1 shape the contract already describes:

- the `core` API running inside the mandatory Docker boundary, fed by a
  per-install token from Windows Credential Manager;
- a Tauri host that owns the token, the container lifecycle and the media
  player, and exposes **named** commands — never a generic passthrough;
- real playback through mpv with real resume and real completion rules;
- a durable queue, a metadata/artwork pipeline, and a download adapter that
  stays off until a human signs off;
- reliability and security evidence (recovery, traversal suite, socket
  inventory, diagnostics) and a shippable Windows installer.

This plan covers **roadmap Phases 3, 5, 6, 7 and the unfinished parts of 1, 2
and 4** in the one order the contracts allow. It does not re-plan Phase 0
discovery measurement work; where a measurement is still missing it appears as
a gate, not as a substitute.

## Where the build actually is (observed 2026-11-18)

Facts, not intentions:

| Area | Observed state |
|---|---|
| Rust workspace | `app-core`, `core`, `media-library`, `persistence`. Builds; clippy and tests pass. |
| API | 26 routes: `/healthz` public + 25 token/HMAC-protected. Unknown fields rejected. |
| Persistence | SQLite with migrations, roots, media, watchlist, queue tables, settings, profiles. |
| Scanner | Real filesystem scan, stable ids, path-escape refusal, duplicates reported. |
| UI | React 19 + Vite on `:5173`, calls the API directly with a **development** token. |
| Playback | `playback_start` returns metadata only. `PlayerPage` runs a simulated clock at 4×; there is **no stream route and no engine**. |
| Queue | Routes exist; no UI, no caller, no tests beyond repository level. |
| Downloads | `downloads_list` exists; no provider, no actions, no writes. |
| Tauri host | Scaffolded 2026-11-18 (A2): window opens, one smoke command, no supervisor/token/command surface yet. |
| Docker | `infrastructure/containers/{compose.yaml,Dockerfile.core}` exist as **unrun artefacts**. The backend runs on the host, outside the required boundary. |
| CI | No workflow. |

Two documentation defects were found while writing this plan. The first is
fixed: ADR-0011's status line and Decision section disagreed; both were corrected
to `Proposed` on 2026-11-18, and the human partner then accepted TMDB the same
day, so ADR-0011 is now `Accepted` with an attributed acceptance record. The
second remains: `roadmap.md`'s current milestone still describes Phase 0 with no
backend.

## Target architecture

```
┌──────────────────────────── Windows host (standard user) ────────────────────────────┐
│                                                                                      │
│  Tauri process                                                                       │
│   ├── token: Windows Credential Manager  (never in the renderer, never on disk)       │
│   ├── supervisor: wsl.exe → docker compose up -d --wait → health/ready polling        │
│   ├── command surface: one named command per operation, signed host-side             │
│   └── player: mpv child process + JSON IPC  (ADR-0002: host placement)               │
│           ▲ host path, resolved by the supervisor from a container→host mount map     │
│           │ never leaves the Tauri process                                            │
│  WebView (React)  ── invoke("library_home") … ──▶  Tauri process                     │
│                        ◀── typed DTOs, no paths, no token ──                         │
└───────────────────────────────────────┬──────────────────────────────────────────────┘
                                        │ 127.0.0.1:PORT  bearer + HMAC-SHA256
┌───────────────────────────────────────▼──────────────────────────────────────────────┐
│ core container (read-only rootfs, non-root, cap_drop ALL, no-new-privileges)          │
│  API · policy · SQLite · queue · storage budgets · metadata cache                    │
│         │ internal network, no gateway                                                │
│  probe container — no token, no DB, ffprobe only                                     │
└──────────────────────────────────────────────────────────────────────────────────────┘
      media roots: read-only bind mounts, rendered per root by the supervisor
```

The single most important structural change is that **the renderer stops
talking HTTP**. ADR-0005 requires the capability surface to be named commands;
today the renderer holds a token and builds signed requests itself, which is
both a token leak into the WebView and a violation of that ADR.

## Tech stack

Unchanged from what is already pinned: Rust 2021 + `axum` + `sqlx` (SQLite) +
`serde`; React 19 + TypeScript + Vite 7; Vitest + Testing Library; Tauri 2
(for the host); mpv via JSON IPC (ADR-0002); Docker Engine in a dedicated WSL2
distro (ADR-0001 Part 2, provisional). No new crate, service or dependency is
added by this plan without the ADR named in the task that needs it.

## Global constraints

- Read `Development.md` §4–§9 and §14 before Task A1. `AGENTS.md` permission
  boundaries apply for the whole plan: no host changes, no runtime install, no
  admin, no external network calls without explicit consent.
- **No path, token or host detail crosses into the renderer.** DTOs stay
  display-only. A file path the UI can act on is a defect, not a feature.
- **Rust owns policy.** Thresholds, completion rules, colour assignment,
  validation and ordering live in Rust. React renders.
- **No new dependency without an ADR.** Each task that needs one names the ADR
  it is blocked on.
- Every destructive action needs a preview and explicit confirmation; nothing
  in this plan deletes user media.
- Tests are the evidence. `cargo fmt`, `cargo clippy --workspace --all-targets
  -- -D warnings`, `cargo test --workspace`, `npm run typecheck`, `npm run
  lint`, `npm test` and `npm run check:tokens` must pass at the end of every
  task that touches that side.
- Timestamps are UTC RFC-3339; SQL uses bound parameters only.
- A checkpoint is evidence, not a checkbox: an unrun test is NOT RUN.

### Explicitly not in this plan

Remote/headless streaming, mobile, multi-user/cloud sync, transcoding farms, AI
features, bundled content sources, automatic permanent deletion, custom themes,
a second dashboard. See `roadmap.md` "Deferred / future".

---

## Decisions required before the first line of code

These are gates. An agent may write the ADR and propose; only the human
partner accepts.

| ID | Decision | Why it blocks | Where it lands |
|---|---|---|---|
| **D-1** | Sequencing: containerise the backend **before** the remaining features — **resolved 2026-11-18**, the human partner chose container-first | Nothing; A1 can record the ADR | ADR-0013 (Accepted) |
| **D-2** | O-06 / O-07: dedicated WSL2 distro vs reusing the existing engine — **resolved 2026-11-18**, dedicated distro accepted, no reuse | Nothing; A4/A6 target the dedicated distro and must still detect and report an existing engine without touching it | ADR-0001 Part 2 acceptance record |
| **D-3** | O-04 metadata provider: **resolved 2026-11-18.** The human partner accepted TMDB as the network provider with local NFO as the offline fallback; ADR-0011 is `Accepted` with an attributed acceptance record | Nothing — D3 and D4 may proceed, subject to the opt-in rules below | Recorded in ADR-0011, the ADR index, `roadmap.md` P2-06 and O-04, and `progress.md` |
| **D-4** | O-05 download provider review: **deferred 2026-11-18** — the human partner deferred the whole download workstream out of v1 | WS-E is deferred, not blocked; do not build a substitute | Plan WS-E + `roadmap.md` O-05 |
| **D-5** | O-09 mpv licence: **resolved 2026-11-18** — LGPL build chosen; the shipped artefact must still be checked as LGPL-configured before distribution | The project licence claim can be made, conditional on that check | ADR-0002 acceptance record + P0-12 evidence |
| **D-6** | Queue autoplay: **resolved 2026-11-18** — off by default, the Up Next overlay offers the next episode | C3 implements the off default and does not auto-advance | Config default, documented |
| **D-7** | Token storage: **resolved 2026-11-18** — DPAPI via the `windows` crate; no `keyring`, no new dependency | Nothing; A3 implements DPAPI and amends ADR-0007's token mechanism | ADR-0014 (Accepted) |

D-1 was decided **container-first** on 2026-11-18 (ADR-0013), so the order in
the dependency graph stands as drawn: A4–A8 stay before B, and B2's path map is
built once. A "host first" variant does not apply.

---

## Dependency graph

```
A1 (sequencing ADR)
 └─ A2 Tauri shell
     ├─ A3 token provisioning            ├─ A4 supervisor lifecycle
     │    └─ A5 named command surface    │    └─ A6 container build + pin
     │         └─ A7 renderer transport  │         └─ A8 boundary evidence
     │              └─ A9 dev-mode path  │
     └───────────────────────────────────┴─ B1 playback descriptor (ADR)
                                                └─ B2 host path map + mpv launch
                                                     ├─ B3 controls over IPC
                                                     │    ├─ B4 resume policy
                                                     │    └─ B5 completion policy
                                                     ├─ B6 codec matrix
                                                     └─ C1 queue UI + API
                                                          ├─ C2 queue durability
                                                          └─ C3 Up Next (gated by D-6)
D2 NFO provider ─ D3 TMDB + cache (opt-in; unblocked 2026-11-18) ─ D4 artwork
E1 (legal gate) ─ E2 provider ─ E3 bounds ─ E4 control surface
F1 recovery ─ F2 safe mode ─ F3 traversal CI ─ F4 sockets ─ F5 diagnostics ─ F6 failure drills
G1 installer ─ G2 lifecycle ─ G3 signing ─ G4 docs
H1 CI; H2 contract drift; H3 dependency-graph assertions
```

---

# WS-A — Host boundary and container workspace

Roadmap: P1-07, P1-08, P4-08. Gates: D-1, D-2, D-7.

### A1 — Record the sequencing decision — **COMPLETE (2026-11-18, ADR-0013)**

**Goal:** one accepted ADR that fixes whether the boundary precedes the rest of
the backend.
**Files:** create `docs/decisions/0013-backend-completion-sequencing.md`;
update `docs/decisions/README.md` index.
**Content:** the observed state (backend on host), the two orderings, the cost
of each, the chosen order, and the trigger that would revisit it.
**Acceptance:** ADR exists, indexed, status honest (`Proposed` until the human
accepts); `roadmap.md` current-milestone section links to it.
**Tests:** none (documentation gate). Human review required.
**Do not:** mark it Accepted yourself.

### A2 — Scaffold the Tauri host — **COMPLETE (2026-11-18)**

**Goal:** a window opens, loads the built UI, and closes without orphans.
**Files:** create `apps/desktop/src-tauri/{Cargo.toml,tauri.conf.json,build.rs,
src/main.rs,src/lib.rs,capabilities/default.json}`; add `tauri` and
`tauri-build` to the workspace as needed; add `apps/desktop/package.json` script
`tauri:dev`.
**Steps:** pin Tauri 2 and the CLI version; `devUrl` = `http://localhost:5173`;
`frontendDist` = `../dist`; window title `MEDIA_HAVEN`; one `get_app_info`
command returning version and OS build for the smoke test only.
**Acceptance:** `npm run tauri:dev` opens a window rendering the real UI;
closing it leaves no `mediahaven-core` or `mpv` process (checked in Task
Manager); cold start recorded in `progress.md`.
**Tests:** manual launch and process-list check; record the number, do not
write "fast".
**Risks:** WebView2 absent; MSVC toolchain missing. Report, do not install
without consent.

**Done 2026-11-18:** created `apps/desktop/src-tauri/` (workspace member
`mediahaven-desktop`) with the `MEDIA_HAVEN` window, `devUrl` 5173, a CSP limited
to self, the artwork protocol and the loopback API, `core:default` as the only
capability, and `get_app_info` plus its unit test. The window was launched
against the running Vite server: title `MEDIA_HAVEN`, responding, WebView child
present with live connections to `127.0.0.1:8080`, and zero processes left after
stopping it. Cold-start timing was **NOT** measured, and the window was not
inspected visually. The renderer still talks HTTP directly, which A5/A7 fix.

### A3 — Per-install token provisioning

**Goal:** a 256-bit token generated once, stored in Windows Credential Manager,
retrievable only by the host process.
**Files:** `apps/desktop/src-tauri/src/secret.rs`; ADR-0014 for the mechanism
(D-7).
**Interface:**
`fn load_or_create_token(service: &str, account: &str) -> Result<SecretToken>`
— never `Debug`, never serialisable, zeroised on drop.
**Steps:** generate from the OS CSPRNG on first run; store; read on subsequent
runs; on read failure surface a `credential_mismatch` state rather than
regenerating silently (regenerating would orphan the running container's token).
**Acceptance:** the token is absent from the repository, from `%APPDATA%` files,
from logs, and from the renderer (assert by searching the built bundle and a
captured log).
**Tests:** unit test that a second call returns the same token; a redaction test
that a deliberately logged token does not appear in output.
**Risks:** silently rotating the token and locking the user out of their own
library.

### A4 — Supervisor lifecycle

**Goal:** the host can start, wait for, health-check and stop the workspace.
**Files:** `apps/desktop/src-tauri/src/supervisor.rs`,
`apps/desktop/src-tauri/src/wsl.rs`.
**Interface:** `Supervisor::{ensure_running, wait_ready, stop, status}`;
`ensure_running` runs `wsl.exe -d <distro> docker compose -f <rendered>
up -d --wait`; `wait_ready` polls `/readyz` with the token.
**Steps:** no shell-string construction — argument vectors only; bounded
timeout (30 s start, 10 s ready) with backoff; every failure maps to one of the
legible states in ADR-0005 §5; never invoke `docker.exe`, never enable a TCP
daemon endpoint.
**Acceptance:** with the engine absent, the app shows the setup explanation and
starts no backend; with it present, `readyz` is reached and the UI loads; on
quit, `down` runs and no container remains.
**Tests:** manual start/stop/quit cycle; a forced-failure test (wrong distro
name) asserting a legible error and zero containers started.
**Risks:** orphan containers on crash — Task A8 must prove cleanup.

### A5 — Named command surface

**Goal:** the renderer calls named commands; the host signs and forwards.
**Files:** create `crates/contract/` (operation list + request/response types,
single source); `apps/desktop/src-tauri/src/commands/*.rs`;
`apps/desktop/src/services/api.ts` replaced by a generated client over
`invoke`.
**Interface:** one command per existing route — `library_home`,
`library_movies`, `library_shows`, `library_detail`, `library_episodes`,
`library_search`, `library_roots`, `library_root_add`, `library_root_remove`,
`library_scan`, `library_watchlist`, `watchlist_add`, `watchlist_remove`,
`watchlist_reorder`, `queue_list`, `queue_add`, `queue_remove`,
`queue_reorder`, `downloads_list`, `settings_get`, `settings_set`,
`playback_start`, `playback_stop`, `playback_checkpoint`.
**Steps:** derive the TS request/response types from `crates/contract` in one
generation step committed to `apps/desktop/src/services/generated.ts`; the host
signs `timestamp\nMETHOD\npath\nsha256(body)` exactly as the API validates;
errors map to the closed code set, never raw backend text.
**Acceptance:** a test asserts the command list equals the route list — no
missing command, no extra command, and **no** `http_request`/`api_request`/
`docker_*` command exists.
**Tests:** command-vs-route parity test; a test that an unknown command name is
rejected by the Tauri builder.
**Risks:** a convenience passthrough creeping in. That is the failure ADR-0005
exists to prevent.

### A6 — Build and pin the container images

**Goal:** `core` and `probe` images build reproducibly and are referenced by
digest.
**Files:** `infrastructure/containers/Dockerfile.core` (exists, but has never
been built — treat its correctness as unproven until the first build),
new `infrastructure/containers/Dockerfile.probe`, `.dockerignore`.
**Steps:** verify the build actually completes before claiming anything; record
the built digests; `compose.yaml` keeps digest-only references; the probe image
contains ffprobe and nothing else of consequence.
**Acceptance:** `docker build` succeeds locally *with consent for using the
engine*; both images run as their declared UIDs; `docker inspect` shows
`cap_drop: ALL`, `no-new-privileges`, read-only rootfs, the declared mounts and
no others.
**Tests:** build + inspect assertions captured to `progress.md`.
**Risks:** multi-stage build of the whole workspace inflating the image; a
probe image with a shell and a package manager left in it.

### A7 — Renderer transport swap

**Goal:** the UI works with the host, and still works in the browser for
development without weakening the production path.
**Files:** `apps/desktop/src/services/transport.ts` (new),
`apps/desktop/src/services/api.ts` (dev-only implementation),
`apps/desktop/src/main.tsx`.
**Steps:** pick `invoke` when `window.__TAURI_INTERNALS__` exists, else the
signed dev client; assert in a test that the production bundle contains no dev
token string and no direct `fetch` to `:8080`.
**Acceptance:** every screen loads through `invoke`; the dev token appears only
in the dev build.
**Tests:** bundle-scan test; the existing Vitest suite re-pointed at a fake
`invoke`.
**Risks:** two transports diverging in error handling. Share the error mapping,
do not duplicate it.

### A8 — Boundary evidence

**Goal:** the four-case auth matrix and the isolation claims, observed.
**Files:** `docs/security/boundary-evidence.md` (evidence log),
`tests/security/auth_matrix.rs` (host-side or integration).
**Checks:** no token → 401; wrong token → 401; replayed signature → 401;
tampered body → 401; `/healthz` unauthenticated and content-free; `/readyz`
authenticated; undeclared mount write fails; declared path write succeeds;
`probe` has no route off `internal`; no orphan container after quit; no
listener on `0.0.0.0`.
**Acceptance:** each check recorded with the command and its output.
**Tests:** the auth matrix must be automated, not a checklist someone ticked.

### A9 — Media root mount rendering

**Goal:** adding a library root adds exactly one read-only mount and nothing
else.
**Files:** `apps/desktop/src-tauri/src/mounts.rs`; the compose override writer;
`docs/architecture/docker-workspace.md` §4 update.
**Steps:** container path prefix `/media/root-<n>`; host path resolved from the
stored root; refuse roots that are not directories, refuse reparse points,
refuse a root nested inside another root; rewrite the override atomically and
recreate only `core`.
**Acceptance:** with two roots, `docker inspect` shows exactly those two mounts;
a root removed from settings disappears from the next inspect; the host path map
is persisted for playback (B2).
**Tests:** mount-scope test; nested-root test; rename-a-root test.
**Risks:** a mount that silently widens (`/` or a drive root). Explicitly refuse.

---

# WS-B — Playback

Roadmap: P3-01 … P3-05. Gates: A5, A9, D-5, D-6.

### B1 — Playback descriptor and path mapping ADR

**Goal:** decide and record how the host learns which file to open without
exposing a filesystem path to the renderer.
**Files:** create `docs/decisions/0015-playback-descriptor.md`.
**Proposed shape:** `playback_start` returns the session **plus** an opaque
`sourceToken`; the host resolves `sourceToken` through its own mount map to a
host path and hands it to mpv; neither the token nor the path is returned to the
renderer. Host-side command: `playback_open(mediaId)` → `EngineHandle`.
**Rejected in the ADR:** returning a path in the DTO (P4-06 forbids it);
streaming bytes through the API (unnecessary on a single machine and it would
reintroduce decode latency that ADR-0002 rejected).
**Acceptance:** ADR states the containment rule and the failure mode when a
file moved between container and host views.
**Tests:** none (decision).

### B2 — mpv adapter and launch

**Goal:** a real file plays in a real window from the app.
**Files:** create `crates/playback/src/{lib.rs,mpv.rs,ipc.rs,events.rs}`;
host wiring in `apps/desktop/src-tauri/src/player.rs`.
**Interface:** `Player::{open, play, pause, seek, stop, status}`; events as a
typed enum consumed by the host and emitted to the UI as `player://state`.
**Steps:** spawn mpv with `--input-ipc-server=<pipe>`, `--idle=yes`, extra
`--hwdec=auto-safe`, no shell; connect to the pipe; 5 s connect timeout; kill
the child on app exit and on window close; never write to the media file (open
read-only via mpv, and assert no write handle appears in the adapter).
**Acceptance:** a seeded fixture plays; a missing file produces a legible error;
quitting mid-playback leaves no `mpv` process.
**Tests:** launch/exit leak test; missing-file test; an IPC round-trip test that
does not need a display (pause/seek state reported back).
**Risks:** engine update changing IPC behaviour; orphaned processes.

### B3 — Replace the simulated player

**Goal:** every control drives the engine, not a timer.
**Files:** `apps/desktop/src/pages/Player/PlayerPage.tsx` and its CSS module;
`apps/desktop/src/services/player.ts`.
**Steps:** delete `SIMULATION_RATE`; subscribe to `player://state`; seek reports
completion from the engine; volume, subtitle and audio track selection map to
IPC properties; keyboard-only reachability for every control; Escape returns
focus to the originating card.
**Acceptance:** the clock and the scrubber agree with the engine after a seek;
no control mutates local state only.
**Tests:** a Vitest test with a fake player event stream; a manual keyboard
walkthrough recorded as a step list.
**Risks:** controls that look responsive while the engine ignores them.

### B4 — Resume policy in Rust

**Goal:** stop, crash or window-close all resume where the user was, and the
policy lives in one place.
**Files:** `crates/app-core/src/domain/playback.rs`,
`crates/persistence/src/sqlite/library_repo.rs`, host checkpoint calls.
**Steps:** checkpoints on pause, stop, every 10 s while playing, and on exit;
one row per media id (`ON CONFLICT DO UPDATE`); resume thresholds in config
(`resume_min_seconds` default 15, `resume_tail_seconds` default 30); a corrupt
or future timestamp is ignored, not trusted.
**Acceptance:** resume survives pause, stop, window close and a simulated kill;
a 3-second position does not prompt resume; two items keep separate progress.
**Tests:** four exit-path integration tests + a two-item concurrency test.
**Risks:** writing on every tick and thrashing the disk; a checkpoint from a
different title overwriting another.

### B5 — Completion policy

**Goal:** "watched" is a multi-signal backend decision and seeking to the end is
not one of the signals.
**Files:** `crates/app-core/src/domain/completion.rs` (new),
`crates/core/src/routes.rs` (`playback_checkpoint` may persist `completed`),
config defaults.
**Rule:** complete only when (watched_duration ≥ 90 % of duration) **and**
(position within last 60 s) **and** a normal end-of-file event was observed.
User-stop, seek-to-end, queue removal and app-close never complete.
**Acceptance:** the five adversarial cases from P3-04 are explicit tests, and
completion never deletes anything (file-count test).
**Tests:** dedicated domain suite with short durations, including a 4-second
file.
**Risks:** false completion feeding future retention. Treat as release blocker.

### B6 — Real codec and subtitle matrix

**Goal:** honest support claims from real playback, not from mpv's docs.
**Files:** `tests/test-media/` fixtures (public-domain, small), results in
`docs/testing/playback-matrix.md`.
**Acceptance:** one row per fixture with PASS/FAIL/UNTESTED and the GPU named;
ASS styling either works or is documented as unsupported; the mpv licence of
the shipped build recorded (D-5).
**Tests:** scripted playback matrix; results transcribed, never summarised as
"works".

---

# WS-C — Queue and Up Next

Roadmap: P3-06. Gate: D-6.

### C1 — Queue screen and mutations

**Goal:** the queue exists in the product, not only in the API.
**Files:** `apps/desktop/src/pages/Queue/*` (new), route in `App.tsx`, nav
entry, `services/api.ts` additions already covered by A5.
**Steps:** render entries; add from detail and cards; remove; reorder (move
up/down keyboard-first, drag optional and not required); empty state; honest
"nothing is downloading" copy.
**Acceptance:** add → reorder → restart → order preserved; remove deletes zero
files (file-count test); duplicate add is suppressed by the backend, not by the
UI.
**Tests:** Vitest for the screen against a fake transport; repository test for
duplicate suppression; file-count assertion.

### C2 — Durable queue semantics

**Goal:** a crash mid-reorder leaves a consistent order and nothing advances on
its own.
**Files:** `crates/persistence/src/sqlite/library_repo.rs` (transactional
reorder), `crates/app-core/src/domain/queue.rs`.
**Steps:** single transaction for reorder; positions renumbered densely;
`queue_reorder` rejects unknown or duplicated ids; history table for completed
and removed entries.
**Acceptance:** forced-kill recovery test; reorder crash consistency test; no
automatic advance.
**Tests:** the three above.

### C3 — Up Next and autoplay

**Goal:** the next episode is offered, and autoplay obeys the configured
default.
**Files:** player Up Next overlay; `settings.playback.autoplayNext` already
exists — wire it.
**Acceptance:** with autoplay off, the overlay appears and waits; with it on,
the next episode starts after the completion signal only; an empty queue never
autoplays.
**Tests:** two Vitest cases; one domain test that completion, not the overlay,
triggers advance.

---

# WS-D — Library metadata and artwork

Roadmap: P2-06. Gate: **D-3 (O-04) is closed.**

Current decision state, so no task can misread it: **TMDB is accepted as the
network provider, with local NFO as the offline fallback** (ADR-0011
`Accepted`, 2026-11-18, accepted by the human partner). This unblocks D3 and D4.
The acceptance carries three obligations that are part of the work, not
optional polish: network use is **opt-in and off by default**, only search terms
(title, year) are ever sent, and TMDB attribution plus cache limits apply. D2
still lands first so the offline path exists before any network call.

### D1 — Record the O-04 decision — **COMPLETE (2026-11-18)**

**Goal:** an accepted answer to O-04 that the implementation can cite.
**Done:** the human partner answered that TMDB was acceptable. ADR-0011 now
reads `Accepted` with a dated acceptance record naming the human partner as the
decider and the agent as the scribe; the ADR index row, the README open-decisions
paragraph, `roadmap.md` P2-06 and O-04, and `progress.md` were updated to match.
**What this task did not do, and any agent still must not do:** invent an
acceptance, add a TMDB client, add a network provider setting, or enable egress
without the opt-in and evidence D3 requires.
**Acceptance:** met — one unambiguous status across ADR, index and roadmap,
attributable to the human partner.
**Tests:** none (decision gate).

### D2 — Local NFO reader and metadata model — **COMPLETE (2026-11-18)**

**Goal:** useful metadata with the network off.
**Files:** create `crates/metadata/src/{lib.rs,nfo.rs,model.rs}`; port trait in
`crates/app-core/src/ports/metadata.rs`; migration-free reuse of existing
columns.
**Steps:** parse `<filename>.nfo` and `movie.nfo` beside the media; XML parsing
with a bounded size (256 KB) and refused entity expansion; map to the existing
metadata columns; never fetch anything.
**Acceptance:** a fixture tree with NFO files displays titles, synopsis, year,
genres; a hostile NFO (external entity, 10 MB, control characters) is refused
without panic.
**Tests:** parser unit tests with hostile inputs; integration test over a temp
tree.

**Done 2026-11-18:** `crates/metadata` (`NfoMetadataProvider`, `nfo.rs`,
`model.rs`) plus the `MetadataProvider` port in `app-core`. The parser is
hand-written and bounded (256 KiB, no DTD/entity, no control characters,
UTF-8 only), so no XML dependency was added. Scanned values reach the existing
`media_items` columns through `MediaUpsert.metadata`, and `COALESCE` keeps
stored metadata when a later scan finds no sidecar. Evidence: 8 metadata tests
and 1 new persistence test pass; clippy is clean.

### D3 — Optional TMDB provider behind explicit opt-in — **unblocked 2026-11-18**

Cites ADR-0011 (`Accepted`). It stays opt-in: accepting the provider does not
mean the app may call it silently, and the off-by-default rule is an acceptance
criterion, not a preference.

**Goal:** richer metadata only when the user says yes, and never a silent
network call.
**Files:** `crates/metadata/src/tmdb.rs`, cache in
`crates/persistence`/`crates/metadata/src/cache.rs`, settings field for opt-in
(backend patch + UI toggle), compose egress flag.
**Steps:** opt-in names exactly what is sent (title, year — never a path);
API key from a secret store, never the repository; 30-day cache with a size cap;
attribution rendered where the data appears; failures degrade to local data.
**Acceptance:** with the network off, the library is fully browsable; with
opt-in off, zero requests leave the machine (assert by a request counter);
cache eviction works when the cap is exceeded.
**Tests:** offline integration; a provider-failure fallback; eviction test.
**Risks:** filenames leaking; terms violation; cache growth.

### D4 — Artwork serving and sanitisation

**Goal:** real posters when available, placeholders always.
**Files:** `crates/core` artwork route **or** host-side cache read — choose the
side when D3's cache shape is known, and record the choice in this plan before
writing the route; `apps/desktop` artwork component. Local artwork/NFO posters
proceed under D2 without this task.
**Acceptance:** no artwork file is executed or rendered as HTML; images are
size-capped and type-checked before display; missing artwork keeps card
dimensions.
**Tests:** hostile image payload tests; a 0-byte and a text file named `.jpg`.

---

# WS-E — Downloads — **DEFERRED 2026-11-18 (O-05)**

Roadmap: P6-02, O-05. The human partner deferred the download workstream out of
v1. This is a deferral, not a refusal of a specific provider: nothing here is
built, `downloads_list` stays read-only, no provider adapter, no control
surface, and no setting is added that implies downloads work. Reopening it
needs a new decision and its own ADR; do not implement a quiet substitute.

### E1 — Sign-off and provider ADR
Record the decision, the provider interface, the directory boundary and the
prohibitions (no bundled trackers, indexers or catalogues).

### E2 — Provider adapter behind a trait
No hard-coded source; disabled by default; per-install credentials only.

### E3 — Bounds and containment
Per-task and total size limits; directory escape tests; never execute a
downloaded file; partial files are not "available".

### E4 — Control surface
Loopback-only, named commands, pause/resume/cancel; fail-closed on tunnel loss.

---

# WS-F — Reliability and security

Roadmap: P5-01 … P5-07, plus the recovery gap left open by P2-02.

### F1 — Interrupted-scan recovery
Persist scan state (`scanning` + root id + cursor path); on start, resume or
mark interrupted honestly; a test kills the scan mid-run and asserts no duplicate
rows and an accurate status.

### F2 — Safe mode and bounded retries
Start without optional services; exponential backoff with a hard cap; a test
asserts the retry count is bounded and the UI states the mode.

### F3 — Traversal suite in CI
Extend `crates/media-library/tests/roots.rs` into `tests/security/`: `..`,
absolute paths, UNC, junction, symlink, case tricks, Unicode normalisation, long
paths, wildcard injection. Each test asserts rejection **and** that no read
occurred outside the root. CI runs it on every push.

### F4 — Socket and privilege inventory
Record `Get-NetTCPConnection` before/during/after a session; assert loopback
only, no `2375/2376`, no firewall change, no elevation. The inventory is
evidence, not prose.

### F5 — Diagnostics with redaction
Seeded bundle containing a token, a home path and a filename must contain none
of them after export; no network; opt-in for filenames. Test asserts absence,
not that a filter exists.

### F6 — Failure drills
Disk-full write, power loss during scan/checkpoint/cleanup, removable drive
removed mid-playback. Each recorded with the observed outcome and any data loss.

---

# WS-G — Packaging and release

Roadmap: P7-01 … P7-05.

### G1 — Installer
Tauri bundler, per-user install, version from one source, SBOM, third-party
licence view. Two builds compared or the non-determinism documented.

### G2 — Lifecycle matrix
Install → first run → import → play → upgrade → uninstall. Row counts and a
resume spot-check after upgrade. Uninstall offers a choice about app data and
never touches media (file-count proof).

### G3 — Signing and supply chain
Certificate decision (O-08) is human; scanners fail the build on high severity;
checksums published; no secret in the repo or logs.

### G4 — User-facing docs
Setup, troubleshooting with real messages, privacy statement matching observed
network behaviour, known limitations (codecs, hardware, untested tiers).

---

# WS-H — CI and contract drift

### H1 — CI for both halves
Rust: fmt, clippy `-D warnings`, test. Node: typecheck, lint, test, token check,
build. Prove each job red on a deliberate break, then green.

### H2 — Contract drift test
The command list, the route list and the generated TS types derive from one
source; a divergence fails the build. This is the test ADR-0005 names.

### H3 — Dependency-graph assertion
`app-core` and `persistence` contain no HTTP server, container-runtime or
Windows-API dependency; the host contains no SQL. A test or a CI grep asserts
it.

---

## Sequencing and checkpoints

| Step | Work | Gate before starting | Evidence that closes it |
|---|---|---|---|
| 1 | D-1…D-7 decisions (all answered 2026-11-18; ADR-0013/0014 accepted, WS-E deferred) | none | Human sign-off recorded |
| 2 | A1–A2 | D-1, D-2 (both accepted 2026-11-18) | Window renders the UI; no orphans |
| 3 | A3–A5 | D-7, D-2 | Token absent from disk/bundle/logs; command parity test |
| 4 | A6–A9 | Engine available with consent | `docker inspect` evidence; auth matrix; mount scope |
| 5 | B1–B3 | A5, A9, D-5 | A real file plays; controls drive the engine |
| 6 | B4–B5 | B2 | Resume across 4 exit paths; 5 adversarial completion tests |
| 7 | **CP-3** | B1–B6 | Playback evidence + resume + completion + matrix |
| 8 | C1–C3 | D-6 | Queue survives restart; no auto-advance; zero deletion |
| 9 | D2 (local NFO) | none | NFO metadata displays offline; hostile NFO refused |
| 10 | D3–D4 | D1 closed 2026-11-18 (human acceptance of TMDB) | Offline session browsable; zero requests with opt-in off; attribution and eviction tested |
| 11 | F1–F6 | CP-3 | Recovery, traversal CI, socket inventory, redaction, drills |
| 12 | **CP-5** | F1–F6 | Security suite green; inventory; drills recorded |
| 13 | A6–A9 hardened + E (if approved) | CP-5 | `docker inspect` captured; provider sign-off or deferral |
| 14 | **CP-6** | workspace verified | No orphans; overhead measured; upgrade preserves data |
| 15 | G1–G4 | CP-5 | Installer + lifecycle matrix + docs |
| 16 | **CP-7** | full matrix | Release gates in `Development.md` §17 |

CP-3 is the first real product gate: until a file plays, the UI is a catalogue.
CP-5 is the point after which packaging is meaningful. **If D-1 chooses
"container later", steps 4 and 13 collapse into one step after step 12 and
CP-6's evidence is gathered at the end — record that deviation, do not pretend
the earlier order applied.**

## Evidence rules for the whole plan

- Every task ends with a `progress.md` entry: what changed, files, commands,
  actual output, what was not run.
- A checkpoint fails if any listed evidence is NOT RUN, unless the item is
  explicitly deferred with a reason and a human has read it.
- Never write PASS for a command you did not run in this repository, on this
  machine, at this commit.
- Screenshots are not evidence of behaviour; logs, test output and recorded
  measurements are.
- Every status line in `roadmap.md` changes only when its acceptance criteria
  are observed — not when the code compiles.

## First executable slice (if the human approves D-1 as recommended)

1. A1 + D-7 ADR drafts, hand to the human for acceptance.
2. A2 Tauri scaffold, window opens, no orphans. Record cold start.
3. A3 token provisioning with the redaction test.
4. A5 command surface with the parity test, still forwarding to the host-run
   backend so the swap can be verified before the container exists.
5. A4 supervisor, verified against a deliberately absent engine first.
6. A6 images, then A8 boundary evidence.

That order means every step is observable on this machine, and the two risky
parts (token, command surface) are proven before containers are involved.

## Risks introduced by this plan

| ID | Risk | Mitigation |
|---|---|---|
| R-22 | Renderer keeps direct HTTP access "just for development" and ships | A7 bundle-scan test; dev token only in dev build |
| R-23 | A generic passthrough command appears for convenience | A5 parity test; ADR-0005 rejection list |
| R-24 | Container→host path mapping resolves a path outside the declared roots | A9 refuses non-directories and reparse points; B2 resolves only through the map |
| R-25 | Token rotation on a read error locks the user out | A3 fails closed with `credential_mismatch`, never regenerates silently |
| R-26 | Completion logic misfires and later deletes media (existing R-05) | B5 adversarial suite is a release blocker |
| R-27 | Plan executed against unaccepted ADRs | D-1…D-7 gates; no task names an ADR as accepted |
