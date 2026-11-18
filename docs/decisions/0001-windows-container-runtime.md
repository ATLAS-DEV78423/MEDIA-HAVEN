# ADR-0001 — Docker is the mandatory v1 backend runtime; engine hosted in a dedicated WSL2 distro

- **Status:** Accepted (mandatory-in-v1 decision) / **Accepted (2026-11-18)** (runtime distribution) — dedicated WSL2 Linux environment running Docker Engine directly; no Docker Desktop dependency. The human partner confirmed the distribution on 2026-11-18; P0-10/P0-13/P0-14 measurements are still required before release (see Verification).
- **Date:** 2026-11-16 (evidence refreshed 2026-11-15 against primary documentation and the reference machine)
- **Supersedes:** the earlier "Proposed — optional, evidence-gated" version of this ADR
- **Deciders:** product owner (mandatory), architecture (distribution)
- **Related:** ADR-0002, ADR-0005, ADR-0006, ADR-0007, ADR-0008, `docs/architecture/docker-workspace.md`

---

## Context

The first draft of this ADR treated Docker as an optional, evidence-gated phase-6
concern, on the reasoning that a local HDMI appliance gains little from containerisation.

The product owner has overturned that framing: the security baseline for v1 **is** an
isolated Docker workspace in which the backend and approved media-processing services run
within controlled filesystem and network boundaries. Docker is therefore a v1
requirement, not a deployment preference.

What remains genuinely open is *which engine distribution* hosts those containers on
Windows. Two candidates, both plausible, with different cost profiles.

## Decision

**Part 1 — accepted and not re-litigated:** Docker is mandatory in v1. The backend does
not run as a bare Windows process or a bare WSL2 process. It runs in containers, under the
controls in `docs/architecture/docker-workspace.md`.

**Part 2 — proposed, pending measurement:** run **Docker Engine (rootful) inside a
dedicated WSL2 Linux distribution**, managed entirely from Windows via `wsl.exe`, rather
than Docker Desktop.

Rationale, in priority order:

1. **No licence dependency, and the cliff is sharper than "large companies".**
   Docker Desktop is free for personal use, education, non-commercial open-source
   projects and small businesses (fewer than 250 employees **and** less than $10M annual
   revenue). Everything else requires a paid subscription — and Docker's own
   documentation states that "paid subscriptions are also required for government
   entities", regardless of size. A product whose third principle is "no mandatory paid
   services" cannot make a paid desktop licence a hard dependency for a subset of its
   users, and a government-adjacent user base is a plausible audience for a local media
   appliance. Docker Engine is Apache-2.0 with no such condition.
2. **One less layer to trust and update.** Option B has: Windows → WSL2 VM → containers.
   Option A has: Windows → WSL2 VM → Docker Desktop's own Electron app, backend service,
   update mechanism and telemetry → containers. Each of those is code running with
   privileged host integration that MEDIA_HAVEN does not audit. Docker's own
   documentation also states that WSL's filesystem is readable *and writable* from the
   Windows host (`\\wsl$`), so Desktop's additional layers are not invisible to the
   boundary we are trying to establish.
3. **Predictable resource ceiling, and a genuinely separate budget.** A dedicated
   distro with an explicit `.wslconfig` memory cap and no Electron layer has fewer
   variable consumers. Docker Desktop's GUI being closed does not stop
   `com.docker.backend`, its helper processes or its `docker-desktop` distro from
   staying resident — on the reference machine the `docker-desktop` distro was still
   `Running` with the GUI closed. More importantly, Option B gives MEDIA_HAVEN **its
   own daemon and its own VM**: on the reference machine, 12 unrelated containers were
   already running on the shared engine, drawing on the same 8 GB ceiling. A shared
   engine means an unrelated project's `docker compose down` is indistinguishable from
   ours, and its memory use competes with ours.
4. **Deterministic engine version.** We pin the engine, not whatever the desktop
   updater installed last Tuesday. Reproducible bug reports.

The honest costs of Option B, accepted rather than hidden:

- We own installation, update and repair of the engine and the distro. Docker Desktop
  gives that away for free.
- No GUI troubleshooting surface; diagnosis is `docker` CLI, `journalctl` and our own
  logs.
- Distro lifecycle (`wsl --unregister`) is a destructive operation we must never
  perform silently, because it would delete the data volume.
- Users who already run Docker Desktop will have a second engine unless we detect and
  reuse it. Detection is required; silent reuse of a desktop-managed engine is not.
- A dedicated distro needs `systemd=true` in `/etc/wsl.conf` for `dockerd` to be
  managed by systemd, and the compose plugin installed as a separate step.
- Docker Desktop's *per-user* install mode is genuinely good and must be acknowledged:
  it needs no administrator rights, installs no privileged system service, and reduces
  the attack surface. Option A's best case is better than Option B's best case on
  those two axes. Option A's worst case — an all-users install with a privileged
  service, a paid licence and an unaudited privileged helper — is what we refuse.

### Option comparison (the evidence behind the decision)

| Criterion | A: Docker Desktop, GUI closed | B: Engine in dedicated WSL2 distro |
|---|---|---|
| RAM, idle, GUI closed | Desktop backend + helper processes + `docker-desktop` distro. Measured figure required (P0-10). | `dockerd` + `containerd` + one distro VM. Measured figure required (P0-10). |
| RAM ceiling control | `.wslconfig` caps the VM; Desktop's own processes are outside that cap | `.wslconfig` caps the whole VM, which contains everything |
| CPU overhead | WSL2 + Desktop's proxies/filesharing helpers | WSL2 only |
| Disk | Desktop install (~1.5–2 GB typical) + distro + images | distro (~300–500 MB) + images |
| Startup | Desktop service starts at login by default | Nothing starts until the app starts the distro |
| Shutdown | Requires `docker desktop stop` or Windows service control | `wsl --terminate <distro>` |
| Reliability | Mature, well-tested on Windows | Mature engine, but WSL2-specific edge cases are ours to handle |
| Windows integration | Excellent: named-pipe context, GUI, dashboards | Good: everything through `wsl.exe`, no named pipe needed |
| Security/isolation | Desktop runs privileged host integration (filesharing, proxying) we do not audit | No extra host integration beyond WSL2 itself |
| Isolation from other workloads | **Shares one daemon and one VM budget with every other project on the machine** (observed: 12 unrelated containers on the reference machine) | Own daemon, own VM; unrelated workloads cannot stop or starve ours |
| Maintenance | Vendor-managed updates | Ours |
| Install ease | One installer, reboot, done | `wsl --install`, distro import, engine install, `systemd=true`, compose plugin |
| Crash/restart recovery | Self-healing service | Start on demand; we detect and restart |
| Licence | Free for personal/education/small business/non-commercial OSS; **paid otherwise, and paid for government entities regardless of size** | Apache-2.0, unconditional |
| Fit for MEDIA_HAVEN | Works, but adds a paid-licence cliff and an unaudited privileged layer | Matches the "free, minimal, auditable" posture — **recommended** |

### Rejected

- **Bare WSL2 process, no container.** Fails the mandatory-isolation requirement.
- **Windows containers mode.** Image ecosystem for the required tooling is poor and the
  isolation model differs from every reference we have.
- **Remote/cloud Docker host.** Contradicts local-first and offline-first.
- **Rootless Docker in WSL2.** Attractive on paper; complicates bind-mount ownership of
  read-only media and adds failure modes for no boundary gain here, because the WSL2 VM —
  not the daemon — is the real boundary in both cases. Revisit only if a concrete need
  appears.
- **Docker Desktop in per-user mode as the shipped default.** It is the best-shaped
  Option A and it still carries the licence cliff for government and large-enterprise
  users, and it still shares one daemon and one VM budget with every unrelated project
  on the machine.

## Consequences

**Positive**
- One mandatory dependency, one documented boundary, one place to audit.
- No licence cliff for any user class.
- Backend compromise is bounded by a read-only media mount and a networkless parser.
- MEDIA_HAVEN's containers cannot be stopped or starved by unrelated workloads.

**Negative / accepted**
- Installation now requires WSL2 and a distro. The user must consent; we may not install
  silently. First-run friction increases, and that is a real product cost.
- We own engine/distro maintenance.
- The app cannot function if Docker is unavailable. This must degrade to a clear,
  actionable error state rather than a crash, and is a hard acceptance criterion.
- Resource floor rises by roughly the VM and engine cost, on a machine class where 8 GB
  is plausible. Mitigated by `.wslconfig` caps and on-demand start; measured in P0-14.
- A user who already runs Docker Desktop ends up with two engines. The preflight detects
  and reports this; it never silently reuses a desktop-managed daemon.

## Evidence

Verified against primary documentation and the reference machine on 2026-11-15:

| Claim | Source | Result |
|---|---|---|
| Docker Desktop licence tiers | docs.docker.com/desktop/setup/install/windows-install/ | Free for personal, education, non-commercial OSS, and <250 employees AND <$10M revenue. Paid subscription required otherwise, **and required for government entities** |
| Docker Desktop does not need to be running as a GUI for containers to work | Docker Desktop WSL2 backend docs | Containers are managed via CLI/Compose; the GUI is not required for normal operation |
| Closing the Desktop GUI leaves the engine resident | Observed on reference machine | `docker-desktop` distro `Running`, `com.docker.backend` family resident, engine serving |
| Docker Desktop WSL2 VM memory ceiling | Observed (`docker info`) | 8 073 883 648 bytes (8 GB) |
| AppArmor availability under WSL2 | Observed (`docker info` → `SecurityOptions`) | `name=seccomp,profile=builtin`, `name=cgroupns` — **AppArmor absent** |
| WSL2 filesystem reachable from Windows | docs.docker.com/desktop/features/wsl/ | "WSL's file system is accessible from the Windows host `\\wsl$`… Windows processes can read and modify files within WSL" — recorded as R-15 |
| Mirrored networking exposes WSL to the LAN | learn.microsoft.com/windows/wsl/networking | Listed as a mirrored-mode *benefit*; mirrored mode therefore excluded from the publication path — R-18 |
| WSL2 GPU access is compute-only | learn.microsoft.com/windows/wsl/tutorials/gpu-compute | CUDA (NVIDIA) and DirectML only; no video-decode API — see ADR-0002 |
| Docker Engine install in WSL2 | learn.microsoft.com (GPU page) references `curl https://get.docker.com \| sh` | Feasible; `systemd=true` required in `/etc/wsl.conf` for `dockerd` under systemd |
| Reference machine topology | Observed | Windows 11 Pro build 26300, 16 GB, i7-13700H, Intel Iris Xe; Engine 29.9.3, kernel 6.18.58.1-microsoft-standard-WSL2, cgroup v2, overlayfs; 12 unrelated containers already running on the shared engine |

Still **unmeasured** — see Verification below.

## Verification (required before this ADR moves to fully Accepted)

1. **P0-10** — measure idle and active RAM/CPU/disk for both options on this machine; the
   measured numbers go in this ADR, replacing every qualitative claim in the table above.
2. **P0-13** — prove the host can reach the published port, and prove the LAN cannot.
3. **P0-14** — baseline total added footprint with `core` + `probe` running.

If P0-10 shows Option B materially worse on this hardware, this ADR is revisited with
numbers rather than re-argued from preference.

## Acceptance record (2026-11-18)

The human partner accepted the runtime distribution above when asked directly: a
dedicated WSL2 distro running Docker Engine, rather than reusing the engine already
installed on this machine. Recorded here by the agent at the human's direction; the
acceptance is of the *target*, not evidence that it works. P0-10/P0-13/P0-14 still
have to produce the numbers before this is relied upon or shipped.

Observed on the reference machine at acceptance time, and relevant to the install
decision: a Docker CLI (29.9.3) is already present, `C:\Program Files\Docker` does not
exist, and WSL reports a stopped `docker-desktop` distro alongside `OmniRushBox`. So
the machine already reaches *an* engine; the accepted decision is nonetheless to run
the product's own dedicated distro rather than depend on that one.
