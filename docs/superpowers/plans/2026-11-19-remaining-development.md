# Remaining Development Master Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Take MEDIA_HAVEN from the current verified state (React UI + Rust API + Tauri host + queue + CI, no checkpoint passed) to a shippable v1: real playback, a live hardened container boundary, reliability/security evidence, and an installer.

**Architecture:** Three lanes with disjoint write surfaces run in parallel where possible. Lane A (container boundary, A6→A8→A9) needs the Docker engine and is now authorized. Lane B (playback, B2→B6) needs ADR-0015 accepted — a human gate. Lane C (reliability/security F-series and packaging G-series) is mostly independent. No lane changes the frozen renderer contract except through named commands already defined in `crates/contract`.

**Tech Stack:** Rust 1.99 / axum / sqlx(SQLite) / Tauri 2, React 19 + Vite 7, Docker Engine + Compose v5.7 (online), mpv via JSON IPC, distroless probe image. No new dependency without an ADR.

**Spec:** `Development.md` (binding contract), `roadmap.md` (checkpoints), `progress.md` (state), `docs/superpowers/plans/2026-11-18-backend-completion.md` (task map), `docs/decisions/*` (ADRs).

## Global Constraints

- Only the human partner accepts an ADR (`docs/decisions/README.md` §Rules) or passes a checkpoint (`roadmap.md` CP rules). An agent records `Proposed` / `NOT RUN` honestly.
- No new runtime, service, crate or dependency without an ADR and approval (`AGENTS.md`; `Development.md` §20).
- No path, token or host detail crosses into the renderer. DTOs stay display-only; `LibraryRoot.path` display-only is a known Minor to resolve, not to widen.
- Rust owns policy; React renders. No security rule in React.
- Docker/Compose is mandatory for v1; no host-side backend fallback. No container mounts the Docker socket; no container is privileged; images pinned by digest.
- No data loss: never delete source media; destructive operations need preview + explicit confirmation.
- Local-first: loopback-only binding, no telemetry, no library data leaves the machine without opt-in.
- `PASS` / `FAIL` / `NOT RUN` must describe an observed run in this repo on this machine. Checkpoints close on evidence, not on code compiling.
- Keep the Playwright plugin and all Playwright config untouched; do not change `vite.config.ts` to satisfy a check.
- No commit, push, install, or host-config change unless the current instruction grants it. Engine use IS granted for this plan's Lane A.
- Ponytail discipline: smallest change that fully works; no abstraction, wrapper, or "for later" code; finish every caller/test/fixture the change breaks.

## Gates still owed by the human (agent must ask, not decide)

1. **ADR-0015** (playback source resolution) — Proposed 2026-11-19. B2/B3 must not depend on it until accepted. Recommended shape: host-side `host_path_map(media_id) -> host path`, no path/token in any renderer-facing DTO.
2. **O-08** installer signing certificate; **O-01 residual** package/bundle ID — block packaging identity (P7-01).
3. **ADRs 0004, 0005, 0008, 0010** — Proposed; accept/revise before code leans on them.
4. **O-03** offline-indefinitely guarantee — due before CP-2.
5. **CP sign-offs** — a human reads each checkpoint summary; CP-5 data-safety gates cannot be waived by an agent.

Agent-decidable (no need to ask): build details, digest pinning, test design, doc wording, file layout within existing structure, how to wire already-accepted decisions (mpv/LGPL, TMDB opt-in, DPAPI, WSL2 distro).

---

# Lane A — Container boundary (A6 → A8 → A9 → P1-07/P1-08 → CP-6)

Engine is online; this lane is unblocked.

### Task A6 — Build and pin the `core` and `probe` images

**Files:** `infrastructure/containers/Dockerfile.core`, `Dockerfile.probe`, `.dockerignore`, `docs/security/boundary-evidence.md` (new), `progress.md`.

**Interfaces:**
- Consumes: existing Dockerfiles; compose requires `MEDIAHAVEN_CORE_IMAGE` and `MEDIAHAVEN_PROBE_IMAGE` as `<image>@sha256:<digest>`.
- Produces: two locally built images with recorded digests; `docker inspect` evidence; a documented build command.

- [ ] **Step 1:** Build the images with explicit tags:
  ```bash
  docker build -f infrastructure/containers/Dockerfile.core -t mediahaven-core:local .
  docker build -f infrastructure/containers/Dockerfile.probe -t mediahaven-probe:local .
  ```
  Record exact exit codes and full output tails in `progress.md`. If a build fails (likely first candidates: `rust:1.99` manifest load, distroless shared-object completeness, network pull), fix the Dockerfile minimally and re-run. Do not weaken security posture to make a build pass.
- [ ] **Step 2:** Smoke the binaries without the engine network:
  ```bash
  docker run --rm mediahaven-core:local /usr/local/bin/mediahaven-core --version || true
  docker run --rm --entrypoint ffprobe mediahaven-probe:local -version
  ```
  Confirm ffprobe starts (proves the copied `.so` set is complete). Record output.
- [ ] **Step 3:** Capture digests:
  ```bash
  docker image inspect mediahaven-core:local --format '{{index .RepoDigests 0}}'
  docker images --digests --format '{{.Repository}}:{{.Tag}} {{.Digest}}' | Select-String mediahaven
  ```
  Locally built images have no registry `RepoDigests`; use the content-addressable image ID (`sha256:...` from `docker image inspect --format '{{.Id}}'`) as the immutable pin for `MEDIAHAVEN_*_IMAGE`. Record both the ID and the limitation (no registry digest until pushed to a registry — note this honestly in boundary evidence).
- [ ] **Step 4:** `docker inspect` assertions on both images: declared `USER`, `Entrypoint`, no shell in probe, healthcheck present on core. Record output.
- [ ] **Step 5:** Update `docs/security/boundary-evidence.md` with commands + observed output. Update `progress.md`: A6 build PASS / FAIL per observed result. No checkpoint passed.

### Task A8 — Boundary evidence (auth matrix + isolation)

**Files:** `docs/security/boundary-evidence.md`, `infrastructure/containers/compose.yaml` (only if an assertion fails), `progress.md`.

**Interfaces:**
- Consumes: built images from A6; a per-install token; loopback port chosen by the agent for the test (e.g. 18081).
- Produces: observed evidence for the four-case auth matrix and isolation claims.

- [ ] **Step 1:** Start the workspace with a generated test token and the digest pins:
  ```bash
  $env:MEDIAHAVEN_API_TOKEN = <64 hex chars generated locally>
  $env:MEDIAHAVEN_CORE_IMAGE = 'mediahaven-core:local@sha256:<id>'
  $env:MEDIAHAVEN_PROBE_IMAGE = 'mediahaven-probe:local@sha256:<id>'
  $env:MEDIAHAVEN_API_PORT = '18081'
  docker compose -f infrastructure/containers/compose.yaml up -d --wait
  ```
  Record exit code. If `up` fails, read `docker compose logs` and fix the minimal cause (e.g. healthcheck timing, env interpolation).
- [ ] **Step 2:** Run the four auth cases against `http://127.0.0.1:18081`: no token → 401; wrong token → 401; replayed signature → 401; tampered body → 401; `/healthz` unauth 200 content-free; `/readyz` auth 200. Use the existing signed client logic or a small local script; record status codes verbatim.
- [ ] **Step 3:** Isolation: `docker inspect` both running containers for non-root, `cap_drop ALL`, `no-new-privileges`, read-only rootfs, exact mounts, no socket, not privileged, resource limits. Confirm `probe` has no route off `internal` (`docker exec ... wget` to a loopback/outside address must fail). Record.
- [ ] **Step 4:** Stop with `docker compose down`; confirm zero orphan `mediahaven-*` containers. Record.
- [ ] **Step 5:** Write all evidence to `docs/security/boundary-evidence.md` and `progress.md`. P1-08's auth-matrix evidence is now producible; do not mark the task `[x]` until the human checkpoint.

### Task A9-live — Mount scope on the live engine

**Files:** `docs/security/boundary-evidence.md`, `progress.md`.

- [ ] **Step 1:** With the workspace up, apply the supervisor's rendered compose override for one test media root (a temp dir with no personal data). `docker inspect` must show exactly one read-only media mount and no extra paths.
- [ ] **Step 2:** Attempt a write to the mounted path from inside `core` — it must fail (read-only). Attempt a write to an undeclared path — must fail (read-only rootfs). Record.
- [ ] **Step 3:** Remove the root, re-render, re-inspect — mount disappears. Record. Zero orphan containers after `down`.

---

# Lane B — Playback (B1 acceptance → B2 → B3 → B4/B5 → B6) — HUMAN GATE

### Task B1-gate — ADR-0015

**Files:** `docs/decisions/0015-playback-descriptor.md` (human accepts), then `crates/playback/*`.

**Interfaces:**
- Consumes: ADR-0015 as written (host resolves media id via `host_path_map()`; no path/token in any renderer DTO).
- Produces: the `Player` port and host wiring.

- [ ] **Step 1:** Present ADR-0015 to the human for acceptance. **If not accepted, this lane stops** — do not write playback code that depends on it. (Controller ruling may proceed only if the human delegates the decision explicitly.)
- [ ] **Step 2 — B2 mpv adapter:** Create `crates/playback/` with `Player::{open, play, pause, seek, stop, status}` behind a `Player` port; spawn mpv with `--input-ipc-server=<pipe> --idle=yes --hwdec=auto-safe`, argument vectors only (no shell), 5 s connect timeout, kill child on app exit/window close, open media read-only. Host wiring in `apps/desktop/src-tauri/src/player.rs`. Tests: launch/exit leak, missing-file legible error, IPC round-trip without a display.
- [ ] **Step 3 — B3 replace simulation:** Delete `SIMULATION_RATE` from `PlayerPage.tsx`; subscribe to `player://state`; seek reports engine completion; volume/subtitle/audio map to IPC; keyboard-only reachability; Escape restores focus. Vitest with a fake event stream.
- [ ] **Step 4 — B4 resume:** Checkpoints on pause, stop, every 10 s, exit; `ON CONFLICT DO UPDATE` one row per media; thresholds in config (`resume_min_seconds` 15, `resume_tail_seconds` 30); four exit-path tests + two-item concurrency.
- [ ] **Step 5 — B5 completion:** `is_complete` = watched ≥ 90 % AND position in last 60 s AND normal EOF event; user-stop/seek-to-end/queue-removal/app-close never complete. The five adversarial cases as explicit tests including a 4-second file. File-count proof: completion never deletes. **Release blocker.**
- [ ] **Step 6 — B6 matrix:** `tests/test-media/` fixtures (user-supplied/public-domain, gitignored) → `docs/testing/playback-matrix.md` with PASS/FAIL/UNTESTED per codec and GPU named; confirm the mpv build is LGPL-configured (O-09 verification).

---

# Lane C — Reliability, security, packaging (F + G) — mostly independent

### Task C1 — F3 traversal suite in CI

**Files:** `tests/security/` (new), `.github/workflows/ci.yml`, `progress.md`.

- [ ] **Step 1:** Move/extend `crates/media-library/tests/roots.rs` cases into `tests/security/traversal.rs`: `..`, absolute, UNC, junction/symlink, case tricks, Unicode normalisation, long paths, wildcard injection, cleanup-outside-root. Each asserts rejection **and** no read/write outside root.
- [ ] **Step 2:** Add a CI step running the suite. Verify red on a deliberately broken assertion, then green. Record.

### Task C2 — F2 safe mode and bounded retries

**Files:** `crates/app-core`, `crates/core`, `apps/desktop` (settings/status surface), tests.

- [ ] **Step 1:** Add a startup mode that disables optional services (metadata provider) when a dependency fails; exponential backoff with a hard cap (e.g. 5 attempts); expose an honest "safe mode" state to the UI. Test: retry count bounded; UI states mode; no row/file deletion.

### Task C3 — F5 diagnostics with redaction

**Files:** new `crates/diagnostics` (no new dependency — reuse `tracing` + serde), `progress.md`.

- [ ] **Step 1:** One-click bundle: logs, config shape, DB schema version, health snapshot. Redact tokens, home paths, filenames (filenames only with opt-in). Seeded bundle test asserts the sensitive values are **absent** after export. No network.

### Task C4 — F1/F4/F6 evidence

**Files:** `docs/testing/`, `progress.md`.

- [ ] **Step 1:** Socket/privilege inventory (`Get-NetTCPConnection` before/during/after): loopback-only, no 2375/2376, no firewall change, no elevation. Record.
- [ ] **Step 2:** Failure drills: disk-full write, power loss during scan/checkpoint/cleanup, removable drive removed mid-playback. Record observed outcomes and any data loss.
- [ ] **Step 3:** Interrupted-scan recovery was implemented 2026-11-19 (see `2026-11-19-scan-recovery.md`); record its evidence here and close the F1 code gap.

### Task C5 — D3/D4 metadata (needs TMDB key + egress consent)

**Files:** `crates/metadata/src/tmdb.rs`, cache, settings opt-in, compose egress flag, UI toggle, artwork route.

- [ ] **Step 1:** With the human-supplied TMDB key in the secret store (never the repo) and explicit egress consent: opt-in provider, off by default, only title/year sent, 30-day cache with size cap + eviction, attribution rendered, failures degrade to local data. Tests: zero requests with opt-in off (request counter), offline browsable, eviction, hostile image payloads.

### Task C6 — G-series packaging (needs O-08 + package ID)

**Files:** `infrastructure/packaging/windows/`, `.github/workflows/release.yml`, docs.

- [ ] **Step 1:** With package ID and signing decision supplied: Tauri bundler config (per-user install, no admin), version from one source, SBOM, third-party licences in-app, checksums. Two-build reproducibility check or documented non-determinism. Lifecycle matrix: install → first run → import → play → upgrade → uninstall, with file-count proof that uninstall never touches source media.

---

# Sequencing (dependency order)

```
Lane A: A6 → A8 → A9-live  ──► P1-07/P1-08 evidence ──► CP-1 (human)
Lane B: [B1 human gate] → B2 → B3 → B4/B5 → B6 ──► CP-3 (human)
Lane C: C1 + C2 + C3 + C4 (parallel) → CP-5 (human)
        C5 (needs key/consent) → CP-2
        C6 (needs O-08 + ID) → CP-7
CP-0 remains a separate human-evidence gate (measurements, hardware, threat model).
```

Lane A runs now. Lane C Tasks C1–C4 run in parallel with A (disjoint files). Lane B waits on the human gate. Lane C C5/C6 wait on their human inputs.

## Execution note (controller record)

Per the user's instruction, subagent-driven development with parallel execution is used. The one-agent-per-task rule exists to avoid write conflicts; here each parallel agent owns a disjoint file set, so parallel dispatch is safe. Ponytail discipline and the Playwright plugin remain active throughout. No ADR is self-accepted and no checkpoint is marked passed by an agent.
