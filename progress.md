# Progress

## Last updated

2026-11-15 (session: Docker-mandatory architecture revision)

## Current state

The repository holds the approved engineering specification (`Development.md`), the
checkpoint-driven roadmap, the documentation baseline and the agent tooling. **No
application code exists yet** — no Rust workspace, no `apps/desktop` Tauri project, no
database, no container images.

This session revised the architecture and documentation to make **Docker/Compose a
mandatory v1 dependency**, selected a proposed Windows runtime distribution, resolved
the playback-placement question, replaced the placeholder Compose file with the real
`core`/`probe` workspace, and reconciled the roadmap with the new boundary. Phase 0 has
still not started: **no CP-0 acceptance criterion is satisfied yet**, because every one
of them requires an observed measurement, not a document.

## Active task

- **ID:** Documentation revision — Docker mandatory for v1 (no roadmap task)
- **Goal:** Make Docker a mandatory v1 dependency across the architecture, ADRs, threat
  model, Compose configuration, roadmap and progress documentation; investigate and
  recommend the headless runtime distribution; re-evaluate playback/GPU placement.
- **Status:** Complete — awaiting human review
- **Branch/commit:** `master`, baseline commit + push to
  `https://github.com/ATLAS-DEV78423/MEDIA-HAVEN.git` — **authorised by the human
  partner in this session** (`Development.md` §15.2 permission obtained).

## Completed this session

### Architecture and decisions

- **`docs/decisions/0001-windows-container-runtime.md` — rewritten.** Part 1 (Docker is
  mandatory in v1) stays **Accepted** and is not reopened. Part 2 now carries the
  verified licence position, the isolation argument and the observed reference-machine
  topology, and recommends **Docker Engine (rootful) in a dedicated WSL2 distro** over
  Docker Desktop. Status: distribution remains **Proposed** pending P0-10/P0-13/P0-14.
- **`docs/decisions/0002-playback-engine.md` — rewritten.** Playback placement
  **decided in principle**: native Windows host, an intentional exception to
  "everything backend-ish lives in Docker". Corrected the claim that WSL2 offers no
  GPU access to containers — `nvdec` *can* work under WSL2 with an NVIDIA GPU and the
  NVIDIA Container Toolkit; the accurate argument is that there is no cross-vendor
  hardware *video-decode* path, and no path at all to the display or audio hardware.
  Added the `--gpu-context=d3d11` requirement for `d3d11va`, and the verified mpv
  licence position (GPLv2+ by default, LGPLv2.1+ only with `-Dgpl=false`).
- **`docs/decisions/0005-host-container-boundary.md` — amended.** Added the NAT-mode
  requirement and the explicit exclusion of WSL2 mirrored networking, which would make
  the published port LAN-reachable.
- **`docs/decisions/0006-filesystem-and-volume-policy.md` — amended.** Corrected the
  claim that a named volume is "only reachable through `core`": WSL exposes distro
  filesystems to Windows via `\\wsl$`, so the volume is a container-containment
  boundary, not a confidentiality boundary (R-15).
- **`docs/decisions/0003-modular-monolith.md` — annotated.** Added an addendum
  recording that ADR-0001 fired this ADR's own revisit trigger, and pointing to
  ADR-0008. The decision itself was not rewritten.
- **`docs/decisions/0008-modular-monolith-deployment-boundary.md` — created.** It was
  referenced by the ADR index but did not exist. Reconciles "one modular monolith"
  with "two deployment units" across the host/container boundary.
- **`docs/decisions/README.md` — index updated** with accurate statuses and the two
  decisions that are already binding.

### Architecture, security and configuration

- **`docs/architecture/docker-workspace.md` — revised.** Added §1.1 (the two verified
  Windows facts: `\\wsl$` exposure and AppArmor's absence), §5.1 (the observed
  reference-machine baseline), removed mirrored networking mode as a fallback,
  corrected the AppArmor row from "not assumed" to "observed absent", and extended the
  residual-risk table to R-18.
- **`infrastructure/containers/compose.yaml` — replaced.** The legacy
  `download-client` scaffold (which was explicitly "NOT ENABLED" and predated the
  mandatory-Docker decision) is gone. The new file implements `core` and `probe` per
  the workspace document: non-root with distinct UIDs, `cap_drop: ALL`,
  `no-new-privileges`, read-only root filesystems, declared tmpfs and named volumes,
  CPU/memory/PID limits, a loopback-only published port, an `internal: true` network
  with no gateway for `probe`, digest-pinned images supplied as **required** variables
  so Compose fails closed, and a real health check on `core`.
- **`infrastructure/containers/README.md` — rewritten** to describe the new workspace.
- **`docs/security/threat-model.md` — extended.** Added the runtime actors and entry
  points 9–10, abuse cases A19–A25, trust boundaries 6–7, and residual risks R-08 …
  R-21. Corrected A1, which still referred to containerising "when Phase 6 exists".
- **`docs/security/compose-security-checklist.md` — rewritten** for the new workspace,
  including the NAT-mode check, the UID-separation check, the `\\wsl$` rule and
  runnable verification commands.

### Contract, roadmap and operations

- **`Development.md` — revised.** §1.5 (playback on the host), §2.1 (diagram and
  trust-boundary statement), §2.2 (two-unit deployment), §2.3 (runtime distribution
  proposed, NAT-mode requirement), new §2.3.1 (what the boundary does and does not
  buy), §2.4/§2.4.1/§2.5, §3 (stack table), §4.4 (playback OPEN DECISION narrowed),
  §6.1 (`\\wsl$` exposure), §8.2 (NAT mode, shared-engine detection, limits on all
  containers), §9.1 (4 GB is no longer a candidate tier), §13.4 (Phase 0/1/6 task
  lists), §18 (open decisions renumbered and corrected — item 3 was previously
  missing entirely).
- **`roadmap.md` — revised.** Added P0-10 … P0-14 (the task IDs the ADRs referenced
  but which did not exist), rewrote CP-0's evidence list, added **P1-07** (stand up
  the required workspace) and **P1-08** (host↔container boundary), restructured
  Phase 6 from five tasks to three, and rebuilt the risk register as R-01 … R-21 plus
  O-01 … O-09.
- **`docs/setup/first-run.md` — revised.** Added the headless-operation contract, the
  WSL2 networking-mode check, the shared-runtime detection, and two new preflight
  checks.
- **`docs/operations/backup-recovery-uninstall.md` — corrected.** The database lives
  in the named volume `core-data`, not in `%LOCALAPPDATA%`; the restore path and the
  uninstall rules were updated to match ADR-0006.
- **`docs/architecture/overview.md`, `process-model.md`, `docs/README.md`,
  `docs/superpowers/plans/00-phase-0-discovery.md`,
  `docs/superpowers/specs/2026-11-15-media-haven-design.md` — updated** for
  consistency.

### A structural defect found and fixed

The roadmap had a **dependency inversion**: Phase 2 onward is backend work, and the
backend *is* the `core` container, but the workspace was only built in Phase 6. That
meant shipping product features on a security boundary that had never been
exercised. The workspace and its host boundary now land in **P1-07/P1-08**, and Phase
6 hardens and verifies what they established. Phase 6's task IDs were consolidated
from five to three (P6-01 … P6-03); every cross-reference in the threat model, ADRs,
spec and `Development.md` was updated.

## Current implementation facts

### Repository state
- No build system yet. There is nothing to compile.
- `git init` has run; the working tree is entirely untracked and **no commit exists**.
- The `origin` remote is set to `https://github.com/ATLAS-DEV78423/MEDIA-HAVEN.git`.
- No dependencies are installed for the application (no `package.json`, no
  `Cargo.toml`).

### Decisions in force
- Desktop shell: Tauri 2. Backend: Rust. Frontend: React + TypeScript (Vite).
- Persistence: SQLite with migrations. Local-first, no mandatory accounts.
- **Docker/Compose is mandatory for v1.** The backend and the media-inspection
  workload run in containers. There is no host-side backend fallback.
- The Windows runtime distribution is **Proposed**: Docker Engine in a dedicated WSL2
  distro (ADR-0001 Part 2).
- **Playback runs on the Windows host** — an intentional, documented exception
  (ADR-0002). The backend stays the authority on what to play.
- One product surface. Internal containers are an implementation detail.
- MVP is direct HDMI playback on one Windows laptop. Remote streaming deferred.
- No AI runtime dependency after setup.
- Automatic permanent deletion is OFF by default.

### Not yet decided (see `roadmap.md` → Risks and open decisions)
- Windows runtime distribution: dedicated WSL2 distro vs Docker Desktop (O-06,
  O-07) — needs P0-10 evidence and human acceptance.
- Playback engine and its licence (O-09) — needs P0-12.
- Host↔container transport and auth scheme (ADR-0005), volume policy (ADR-0006),
  secret provisioning (ADR-0007), deployment boundary (ADR-0008) — all **Proposed**.
- Project name, branding, package ID (O-01) — neutral placeholder in use.
- Metadata provider (O-04); download provider legal review (O-05).

### Reference machine (observed, read-only, 2026-11-15)
- Windows 11 Pro build 26300, 16 GB RAM.
- Intel Core i7-13700H (14C/20T), **Intel Iris Xe** (Xe-LP — expected to have no
  AV1 hardware *decode*; AV1 decode requires Intel Arc or later; to be confirmed by
  P0-12).
- WSL2 present; default distribution `OmniRushBox` (stopped); `docker-desktop` distro
  **running**.
- Docker Engine **29.9.3**, kernel `6.18.58.1-microsoft-standard-WSL2`, cgroup v2,
  storage driver `overlayfs`.
- `docker info` reports `SecurityOptions: [name=seccomp,profile=builtin
  name=cgroupns]` — **AppArmor absent**, seccomp and cgroup namespaces present.
- WSL2 VM memory ceiling observed at 8 073 883 648 bytes (8 GB), 20 CPUs visible.
- **12 unrelated containers already running on the shared engine**, drawing on the
  same 8 GB budget. This is the concrete evidence behind the dedicated-distro
  recommendation and behind R-16.

### Known working commands
- Preflight (read-only): `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/windows/preflight.ps1`
- Compose parse check (see `docs/security/compose-security-checklist.md`).
- Nothing else. There is no build or test command yet.

## Verification

| Check | Result | Notes |
|---|---|---|
| Rust formatting | NOT RUN | No Rust code exists yet |
| Rust tests | NOT RUN | No Rust code exists yet |
| Frontend checks | NOT RUN | No frontend project exists yet |
| Build | NOT RUN | No build system yet |
| E2E | NOT RUN | No runnable app yet |
| Compose file resolves | **PASS** | `docker compose config --quiet` → exit 0, with placeholder digests and token |
| Compose security posture resolves | **PASS** | Resolved config shows `read_only: true` on both services, `cap_drop: ALL`, `no-new-privileges:true`, `cpus`/`memory`/`pids` limits, `internal: true` on the internal network, and `host_ip: 127.0.0.1` on the single published port |
| No privileged / socket / host-networking in compose | **PASS** | `grep -nE "privileged\|/var/run/docker.sock\|network_mode"` → no findings |
| All images digest-pinned | **PASS** | Every `image:` is `<image>@sha256:…`; digest is a required variable so Compose fails closed |
| Documentation consistency | **PASS** | ADR index, roadmap task IDs, `Development.md` §13.4/§18, threat-model references and the Phase 0 plan cross-checked; no dangling `P0-1x`/`P6-0x` references remain |
| Reference-machine runtime facts | **PASS** (read-only) | OS/RAM/CPU/GPU, `wsl -l -v`, `docker context ls`, `docker info`, `docker ps`, `docker images` inspected; nothing modified |
| Upstream documentation research | **PASS** (read-only) | Docker Desktop licensing and WSL2 backend docs, Docker Desktop Windows install/system-requirements page, Microsoft WSL networking and GPU-compute docs, mpv `options.rst` and `README.md` fetched and quoted |
| Threat model reviewed by a human | **NOT RUN** | CP-0 requires this; it has not happened |
| CP-0 evidence items | **NOT RUN** | 0 of the CP-0 criteria are satisfied by this session |

## Blockers and failures

- **No blocker to the documentation work.** The documentation revision is complete.
- **CP-0 is not passed and must not be treated as passed.** Every criterion requires
  a measurement or a human sign-off. A document that says "recommended" is not
  evidence.
- **`pids_limit` alongside `deploy.resources.limits` is rejected by Compose v2**
  (`can't set distinct values on 'pids_limit' and 'deploy.resources.limits.pids'`).
  Resolved by moving the PID limit inside `deploy.resources.limits`. Recorded because
  the same mistake is easy to make in the real `compose.yaml`.
- **The reference machine has a shared Docker engine with 12 unrelated running
  containers.** This is not a MEDIA_HAVEN fault, but it means any P0-10/P0-14
  measurement on this machine must separate the shared case from the isolated case,
  and it is the concrete justification for a dedicated distro.
- **The reference machine has no NVIDIA GPU.** The `nvdec` path cannot be tested
  here. That is a hardware-availability fact, not evidence that `nvdec` fails on
  NVIDIA hardware, and the ADR is written to say so.

## Known risks / regressions

- None in code, because there is no code.
- **Documentation risk:** the acceptance criteria in `roadmap.md` are prose. They only
  become verifiable when the Phase 1 test harness exists. Until then a task must not
  be marked complete on the basis of "the code was written" — or, as this session
  demonstrates, on the basis of "the documentation was written".
- **Unreviewed-architecture risk:** ADRs 0002, 0005, 0006, 0007 and 0008 are all
  **Proposed**. An agent may write them and recommend; only the human partner may
  accept them.
- **Tooling risk:** `.agents/skills` and `.claude/` are gitignored, so a fresh clone
  will not have the skills or the design hooks. `README.md` documents the reinstall
  command.
- **Uncommitted-work risk:** resolved — the human partner authorised the baseline
  commit and push, so the work is under version control. The repository had **no
  commits at all** before this session.

## Next actions

1. **Review this architecture revision.** Read `docs/decisions/0001`, `0002` and
   `0008`, `docs/architecture/docker-workspace.md` and the revised `roadmap.md`.
   Confirm or correct the runtime-distribution recommendation and the playback
   exception.
2. **Accept or revise ADR-0008** (modular monolith across host and container), which
   is the only ADR with no human review and a missing predecessor.
3. **Start Phase 0** with P0-10 (measure the runtime distributions) and P0-13 (prove
   LAN non-reachability) — these two unblock the runtime decision and the security
   claim respectively.
4. Acceptance criteria: an ADR records the runtime distribution with the human's
   acceptance; the compose file and its checklist pass; and the CP-0 evidence list in
   `roadmap.md` has observed values behind each box.
5. Tests to run: none yet — this is still the discovery phase.

## Decisions made

- **Docker is mandatory for v1** — reaffirmed by the product owner and now consistent
  across every document. Recorded in ADR-0001 Part 1.
- **Runtime distribution: dedicated WSL2 Docker Engine recommended** (ADR-0001
  Part 2). Status **Proposed**; the human partner accepts.
- **Playback on the Windows host** (ADR-0002). Status **Proposed**; placement
  recommended, engine and licence still open.
- **WSL2 mirrored networking excluded** from the publication path (R-18). Basis:
  Microsoft's own documentation lists LAN reachability as a mirrored-mode benefit.
- **Phase 6 restructured** to three tasks, with the workspace brought forward to
  P1-07/P1-08. Basis: the dependency inversion described above.
- **Git:** the human partner authorised an initial baseline commit and a push to the
  configured `origin`. The baseline is committed on `master` and pushed. Subsequent
  commits still need permission per `Development.md` §15.2, unless the human says
  otherwise in the current instruction.
- `memorix` remains **not adopted** — unchanged from the previous session; it would
  need a decision record because it changes how agents read project context.
- No other product or architecture decision has been made by an agent.

## Open questions

1. **Runtime distribution:** dedicated WSL2 distro (recommended) or Docker Desktop?
   O-06 / O-07. Needs P0-10 evidence before acceptance.
2. **libmpv licence (O-09):** LGPL or GPL? This constrains the project's own licence
   and blocks CP-0.
3. **Project name and branding** (O-01) — needed before the installer and package IDs
   are fixed.
4. **Is ADR-0008 accepted as written?** It is the only ADR with no human review and it
   had no predecessor.

## Agent handover

- **Task:** Documentation revision — Docker mandatory for v1 (complete).
- **Current state:** Architecture, ADRs, threat model, Compose configuration,
  roadmap and operations documentation are consistent with the mandatory-Docker
  decision. Phase 0 has not started; no CP-0 criterion is satisfied.
- **Files changed:**
  - `Development.md`
  - `roadmap.md`
  - `progress.md` (this file)
  - `infrastructure/containers/compose.yaml`, `infrastructure/containers/README.md`
  - `docs/architecture/docker-workspace.md`, `docs/architecture/overview.md`,
    `docs/architecture/process-model.md`
  - `docs/decisions/0001`, `0002`, `0003`, `0005`, `0006`, `0007`, `0008`,
    `docs/decisions/README.md`
  - `docs/security/threat-model.md`, `docs/security/compose-security-checklist.md`
  - `docs/setup/first-run.md`, `docs/operations/backup-recovery-uninstall.md`
  - `docs/README.md`
  - `docs/superpowers/plans/00-phase-0-discovery.md`,
    `docs/superpowers/specs/2026-11-15-media-haven-design.md`
  - `CHANGELOG.md`, `README.md`
- **Commands run:** `docker compose config --quiet` and the resolved-config
  inspection (exit 0); the checklist's static greps (no findings); a fail-closed check
  with the digest variable unset (exit 1, as intended); read-only host inspection
  (`Get-CimInstance`, `wsl -l -v`, `docker context ls`, `docker info`, `docker ps`,
  `docker images`); `webfetch` of Docker/Microsoft/mpv primary documentation;
  `grep`/`Select-String` cross-reference sweeps across `docs/`; `git add --dry-run` and
  a credential-literal scan of all tracked content.
- **Tests:** PASS for the compose parse, the compose security posture, the
  fail-closed behaviour and the documentation cross-reference sweep. **NOT RUN** for
  everything else — no Rust, frontend, build or E2E exists.
- **Uncommitted changes:** none — the baseline is committed and pushed.
- **Known hazards:** the whole work is uncommitted and therefore fragile. A mirrored
  `.wslconfig` on the host would silently invalidate the loopback-isolation claim —
  preflight must check it. The reference machine's shared Docker engine is running
  12 unrelated containers; do not run `docker compose down` against anything that is
  not ours.
- **Do not repeat:** `pids_limit` next to `deploy.resources.limits` (Compose v2
  rejects it); claiming WSL2 has no GPU access at all (it has compute access, and
  `nvdec` can work under WSL2 on NVIDIA); claiming a named volume is unreachable
  from Windows (it is reachable via `\\wsl$`); treating mirrored networking mode as a
  harmless fallback (it exposes the published port to the LAN).
- **Next safe action:** ask the human partner to (a) confirm or correct the runtime
  recommendation and the playback exception, and (b) decide whether to commit and push
  the baseline to the configured `origin`.
