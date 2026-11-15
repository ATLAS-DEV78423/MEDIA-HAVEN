# MEDIA_HAVEN — Docker Workspace (Isolated Backend)

**Status:** Proposed architecture — awaiting CP-0 approval
**Owner:** architecture
**Supersedes:** the "Docker is optional" framing that appeared in earlier drafts of
`Development.md` §2.3 and `docs/architecture/overview.md`
**Related:** ADR-0001, ADR-0002, ADR-0005, ADR-0006, ADR-0007, ADR-0008

---

## 1. What the workspace is, and what it is not

The MEDIA_HAVEN backend runs inside a Docker workspace on the same Windows machine.
The workspace is the **security baseline for v1**, not a deployment convenience.

| It is | It is not |
|---|---|
| The boundary that holds the backend, its database and untrusted media parsing | A remote server |
| A place where the user's media is mounted **read-only** by default | A place where media is copied or reorganised |
| A set of non-root, capability-stripped containers | A privileged sidecar with host access |
| Reached over a single loopback API with a per-install token | Exposed to the LAN, the internet or the Docker socket |
| Started and stopped by the host application | Left running permanently in the background |

**Honest framing:** Docker gives MEDIA_HAVEN a *process and filesystem* boundary, not
a *security guarantee*. On Windows the container runs on a shared Linux kernel inside a
WSL2 virtual machine. The VM is a stronger boundary than the container; the container
protects the host from the backend's filesystem mistakes and the backend from the host's
filesystem contents. It does **not** protect against a kernel exploit, a hostile Docker
daemon, or an attacker who already runs code as the same Windows user. Section 8 lists
what remains after the controls in this document.

### 1.1 Two Windows facts that constrain the whole design

Both were verified against primary documentation and the reference machine, not assumed.

**Fact 1 — the WSL2 filesystem is readable and writable from Windows.** Docker's own
documentation states that "WSL's file system is accessible from the Windows host
`\\wsl$`, meaning Windows processes can read and modify files within WSL. This
behavior is not specific to Docker Desktop, but rather a core aspect of WSL itself."

Consequence: a **named volume is not a secret store**. The database inside
`core-data` can be read and edited by any Windows process running as the same user,
via `\\wsl$\<distro>\var\lib\docker\volumes\...`. This does not defeat the design —
the database is the user's own watch history, not a credential — but it does defeat the
weaker claim that the volume is "unreachable except through `core`". The real
guarantee is narrower and still meaningful: **no other container can reach the
volume**, and the original media is protected by read-only mounts, not by volume
obscurity. ADR-0006 is corrected accordingly.

**Fact 2 — AppArmor is not available on the WSL2 kernel.** `docker info` on the
reference machine reports `SecurityOptions: [name=seccomp,profile=builtin
name=cgroupns]` — seccomp and cgroup namespace isolation are present, AppArmor is
**not**. AppArmor therefore cannot be listed as an active control on Windows. §6
states the substitution honestly instead of implying a defence that is not there.

---

## 2. Container inventory

Two containers in v1. Each has exactly one responsibility. No container exists because
it was technically possible.

### 2.1 `core` — the backend

| Property | Value |
|---|---|
| Responsibility | HTTP API, domain logic, policy decisions, SQLite database, job queue, storage policy |
| Runs as | dedicated non-root user, fixed UID/GID |
| Root filesystem | `read_only: true` |
| Writable paths | `/var/lib/mediahaven` (database), `/var/cache/mediahaven` (artwork), `/tmp` (tmpfs, size-capped) |
| Media access | all library roots mounted **read-only** at `/media/<root-id>` |
| Network | outbound to the egress network only; inbound only on the loopback API port |
| Parses untrusted files | **no** — it hands that to `probe` |
| Privileges | `cap_drop: ALL`, `no-new-privileges:true`, no `privileged` |

Rationale: everything that makes decisions or holds state lives in one place, so there is
one audit surface for authorisation and one place where the storage policy is enforced.

### 2.2 `probe` — media inspection

| Property | Value |
|---|---|
| Responsibility | `ffprobe` metadata extraction, thumbnail and trickplay generation — the only component that opens untrusted media bytes |
| Runs as | dedicated non-root user, different UID from `core` |
| Root filesystem | `read_only: true` |
| Writable paths | `/var/lib/mediahaven/probe-tmp` (tmpfs), `/var/lib/mediahaven/staging` (shared with `core`, one direction) |
| Media access | read-only, same mounts |
| Network | `network_mode: none` effective — attached only to an `internal: true` network with no gateway |
| Privileges | `cap_drop: ALL`, `no-new-privileges:true` |

Rationale: file-format parsers are the highest-risk code in any media application.
`probe` is the only container that feeds attacker-influenced bytes to a C library, so it
gets no credentials, no database, no outbound network and a throwaway filesystem. If it
is compromised, the blast radius is a container that cannot reach anything.

`probe` is invoked through the job queue and can be **not running** outside of import
work. Starting it on demand is a Phase 6 task; in v1 it may stay resident if the measured
cost of starting it per job is worse.

### 2.3 Deliberately absent

| Not a container | Why |
|---|---|
| Playback engine | Cannot reach the Windows GPU or audio stack reliably — see ADR-0002 |
| Frontend | Tauri needs native window, compositor and GPU access on Windows |
| Database server | SQLite is a file; a DB container would add a network hop and a second auth surface for no isolation gain |
| Download client | Legally gated and out of v1 scope (O-05) |
| Reverse proxy | Nothing to proxy; one loopback port is the whole API |
| Watchdog / supervisor | The host supervisor (a Windows process) already owns lifecycle |

---

## 3. Network boundary

```
Windows host                                    WSL2 VM
┌──────────────────────────────┐   ┌──────────────────────────────────────┐
│ Tauri shell (Rust)           │   │  docker network: mediahaven_frontend │
│  ├─ UI (WebView2/React)      │   │   (bridge, internal: false)          │
│  ├─ playback engine (mpv)    │   │   ┌────────────┐                     │
│  └─ host supervisor          │   │   │   core     │ :8080               │
│       │                      │   │   └─────┬──────┘                     │
│       │ 127.0.0.1:PORT       │   │         │                            │
│       └──────────────────────┼───┼─────────┘                            │
│         Bearer + HMAC        │   │  docker network: mediahaven_internal │
│                              │   │   (internal: true — no gateway)      │
│       x docker CLI via       │   │   ┌────────────┐                     │
│         wsl.exe -d <distro>  │   │   │   probe    │ no published ports │
└──────────────────────────────┘   │   └────────────┘                     │
                                   └──────────────────────────────────────┘
```

Rules:

1. **One published port.** `core` publishes exactly one port, to the VM's loopback.
   Nothing else publishes anything.
2. **No LAN exposure by design, and verified.** WSL2's NAT means a container port is not
   reachable from the physical LAN unless someone creates a `netsh interface portproxy`
   rule. The preflight script checks for such rules and reports them; the setup guide
   tells the user not to add them. This is a *verified assumption*, not a belief — P0-13
   must confirm reachability from a second machine is absent.
3. **`probe` is networkless.** It sits on an `internal: true` network with no gateway, so
   even a fully compromised parser cannot open an outbound connection.
4. **No Docker socket.** The socket is never bind-mounted into any container. No
   container can create, inspect or start another container.
5. **The frontend never speaks Docker.** Only the host supervisor invokes the Docker CLI,
   and it does so by running `docker` *inside* the WSL2 distro through `wsl.exe`. The
   Windows `docker.exe` shim is not required, and no TCP Docker endpoint (`2375`/`2376`)
   is ever enabled.
6. **Egress is explicit.** `core` reaches the internet only for an opted-in metadata
   provider. With no provider configured or opted in, the compose file defines the
   container with no route to the internet (see §5 of the compose README).

### 3.1 The one unresolved networking question

WSL2 forwards Windows `localhost` to listeners inside the VM. Whether that forwarding
reaches a socket bound to the **VM's loopback** (which is what a `-p 127.0.0.1:…`
publication creates) has varied across WSL2 releases and networking modes.

- **Preferred:** publish `127.0.0.1:PORT:8080` inside the VM.
- **Documented fallback:** publish `0.0.0.0:PORT:8080` inside the VM, which is still not
  LAN-reachable **in NAT mode**, and rely on the absence of `portproxy` rules.

**Mirrored mode is excluded.** An earlier draft of this document listed
`networkingMode=mirrored` (Windows 11 22H2+) as a fallback for host→container
reachability. That was wrong and has been removed. Microsoft lists "Connect to WSL
directly from your local area network (LAN)" as a *benefit* of mirrored mode: mirrored
networking deliberately makes the WSL2 instance reachable from the LAN, and published
container ports become LAN-reachable without any `portproxy` rule. Using it would
silently break rule 2 of §3.

Mirrored mode also changes what "loopback" means, and `127.0.0.1` inside mirrored
networking is shared with the host, so a loopback publication stops being a private
per-machine channel. MEDIA_HAVEN therefore **requires the default NAT networking mode**
and treats a mirrored `.wslconfig` as a preflight failure, reported to the user rather
than silently worked around.

This is not decided from documentation alone. **P0-13 measures NAT-mode reachability
from the host and from a second machine**, and the result is recorded in this file
before CP-0 is signed off.

---

## 4. Filesystem boundary

Five distinct classes. Conflating any two of them is the failure mode this design exists
to prevent.

| Class | Lives in | Container sees | Writable by |
|---|---|---|---|
| **1. Original user media** | user's own folders, wherever they chose | bind mount, `:ro` | nobody. Not `core`. Not `probe`. Not the host app. |
| **2. Application data** | named volume `mediahaven_data` | `/var/lib/mediahaven` | `core` only |
| **3. Temporary processing** | tmpfs inside `probe`/`core` | `/tmp`, `/var/lib/mediahaven/probe-tmp` | the owning container, lost on restart |
| **4. Imported / untrusted files** | named volume `mediahaven_staging` | `/var/lib/mediahaven/staging` | `probe` writes, `core` reads and promotes |
| **5. Config and secrets** | Windows Credential Manager + runtime env | environment only, never a file in an image | host supervisor |

Hard rules:

- Root filesystems are `read_only: true`. Every writable path is named in §2 and in the
  compose file. `docker inspect` is the evidence.
- The user's media is mounted **read-only** unless the user, on a specific screen,
  explicitly grants a write operation. That grant is per-root and revocable, and v1 ships
  with no feature that needs it.
- **Never** mounted: `C:\`, `C:\Users`, `%USERPROFILE%`, `C:\Windows`, `C:\ProgramData`,
  `\\.\pipe\docker_engine`, the WSL2 distro filesystem, or any removable drive root.
- Mounts are declared per library root, one bind mount per root, named `/media/root-<n>`.
  Adding a root restarts `core`; it never re-creates the container with a wider mount.
- Path containment is enforced **twice**: lexically before any filesystem call, and again
  after canonicalisation, with reparse points and junctions refused. The container's
  read-only mount is the third layer, not the first.

Consequence worth stating plainly: because media mounts are read-only, a path-traversal
bug in the backend cannot damage the user's library. It can only disclose files inside a
root the user already chose to share. That is the design working as intended.

---

## 5. Resource limits

Declared per container in the compose file, not left to chance:

| Limit | `core` | `probe` |
|---|---|---|
| `cpus` | 2.0 | 1.0 |
| `mem_limit` | 512 MB | 1 GB (parsers and thumbnailing are spiky) |
| `pids_limit` | 256 | 128 |
| `read_only` | true | true |
| `tmpfs` size | `/tmp` 64 MB | `/tmp` 256 MB, `probe-tmp` 512 MB |
| `shm_size` | 64 MB | 256 MB |
| log driver | `json-file`, `max-size: 10m`, `max-file: 3` | same |

These are **starting points**, not measured truth. They are validated against the P0-14
resource baseline and adjusted with evidence; `progress.md` records the measured figures.

**Storage limits are a known gap.** Docker on WSL2 does not reliably support per-volume
quotas (`overlay2` with `pquota` is not available on the default WSL2 setup, and
`--storage-opt size=` is filesystem-dependent). MEDIA_HAVEN therefore enforces storage
limits **in application policy**: a declared budget for the artwork cache, staging area
and logs, with admission refusal and eviction when the budget is hit. That is weaker than
a kernel-enforced quota and is recorded as residual risk R-09.

### 5.1 Reference-machine baseline (observed, not a promise)

Measured on the development machine, 2026-11-15, using the Docker Desktop WSL2 backend:

| Property | Observed value |
|---|---|
| Host | Windows 11 Pro build 26300, 16 GB RAM, Intel Core i7-13700H (14C/20T), Intel Iris Xe |
| Engine | Docker Engine 29.9.3, kernel `6.18.58.1-microsoft-standard-WSL2`, cgroup v2 |
| Storage driver | `overlayfs` (not `overlay2`) — no `pquota` support, confirming the quota gap above |
| VM memory cap | 8 073 883 648 bytes (8 GB) as reported by `docker info` |
| CPUs visible to containers | 20 |
| `SecurityOptions` | `name=seccomp,profile=builtin`, `name=cgroupns` — **no AppArmor** |
| Distros present | `docker-desktop` (Running), plus one unrelated user distro (Stopped) |
| Other workloads on the same daemon | 12 unrelated containers already running, consuming the same 8 GB VM budget |

That last row is the important one, and it is why ADR-0001 recommends a **dedicated
distro** rather than reusing a shared engine. On this machine MEDIA_HAVEN's workspace
would compete for the same VM memory ceiling as an unrelated Supabase stack, and a
`docker compose down` issued by an unrelated project could not be told apart from ours.
P0-10 and P0-14 measure the isolated case; the shared case is recorded as R-16.

These numbers describe one machine. They are the baseline to compare against, not a
published minimum specification.

---

## 6. Least privilege, in detail

| Control | Applied how | Verified by |
|---|---|---|
| Non-root user | `user: "10001:10001"` in compose; image `USER` set in the Dockerfile | `docker exec core id` → uid 10001 |
| No privilege escalation | `security_opt: [no-new-privileges:true]` | `docker inspect` |
| Dropped capabilities | `cap_drop: [ALL]` | `docker inspect` |
| Non-privileged | `privileged` absent (never `true`) | `docker inspect` |
| Read-only rootfs | `read_only: true` | `docker inspect` |
| Explicit writable paths | `volumes:` + `tmpfs:` only | diff rootfs vs declared list |
| Seccomp | Docker default profile retained; no `seccomp:unconfined` | `docker info` → `name=seccomp,profile=builtin` |
| AppArmor | **Not available under WSL2 — observed, not assumed.** Documented as an absent control; `probe`'s networkless design carries the weight instead | `docker info` → `SecurityOptions` |
| Pinned images | `@sha256:` digests, never `:latest`; digest supplied as a required variable so Compose fails closed | compose file review |
| No socket | compose has no `/var/run/docker.sock` anywhere | `grep` in CI, checklist |
| Health | `healthcheck` on `core`; supervisor waits for it | startup trace |

Seccomp and AppArmor are listed as *where supported*, and the second is now an
**observed absence**: `docker info` on the reference machine reports
`SecurityOptions: [name=seccomp,profile=builtin name=cgroupns]` with no AppArmor entry,
because the WSL2 kernel does not offer it. Do not claim AppArmor protection on Windows.
If AppArmor is unavailable, `probe`'s networkless design carries the weight, and that
substitution is stated in the threat model rather than glossed over.

---

## 7. Lifecycle

Owned by the **host supervisor** — a small Windows process inside the Tauri shell. The
user never types a Docker command.

```
launch
  └─ preflight          docker reachable? engine running? distro present? images present?
      ├─ no  → guided setup screen (consent required, nothing installed silently)
      └─ yes → compose up -d --wait
                 └─ poll core /healthz until 200 or timeout
                      ├─ ok      → hand the API base URL + token to the UI
                      └─ timeout → diagnostics screen with real log tail (secrets redacted)

exit
  └─ compose stop (not down)   → data preserved, containers stopped
     optional: "keep running for background import" toggle, off by default

crash / Windows restart
  └─ nothing auto-starts. Next launch detects leftover containers and resumes.
     restart: on-failure:5 exists for transient container death, not for host reboot.
```

Deliberate choices:

- **`stop`, never `down`** on exit, so a mistaken exit cannot remove a volume.
- **No autostart service.** A media app has no business running when nobody is watching.
  The cost of this choice is that first launch after a reboot pays a container-start
  latency, which the splash screen absorbs.
- **Image updates are user-initiated and reversible.** Tag the previous digest, pull the
  new one, run migrations inside `core`, keep the old digest recorded so a failed update
  can be rolled back. Never `latest`.
- **Recovery after unexpected termination** = re-run `up -d --wait`. Idempotent by design.

---

## 8. Residual risks after all of the above

| ID | Risk | Why it survives | Mitigation | Status |
|---|---|---|---|---|
| R-08 | WSL2/VM escape or Linux kernel exploit in the shared guest kernel | Containers share the VM kernel | Keep the WSL2 kernel updated; keep containers minimal; treat `probe` as untrusted | Accepted |
| R-09 | No kernel-enforced storage quota on volumes | Docker/WSL2 limitation; `overlayfs` has no `pquota` | Application-level budgets + eviction | Accepted, documented |
| R-10 | Same-user local process reads the API token | Windows has no per-process secret isolation from the same user | Loopback-only + token + no CORS + no cookies; same-user attackers are out of scope | Accepted |
| R-11 | Docker daemon is root inside the VM | Rootful engine is the practical default | The VM is the boundary; no socket exposure; containers are non-root | Accepted |
| R-12 | Malicious image or supply-chain compromise | Images come from a registry | Digest pinning, minimal images, user-visible update step, no `latest` | Open — signing decision deferred |
| R-13 | WSL2 NAT networking assumption wrong | Cannot verify without measuring | P0-13 measures host reachability and LAN non-reachability; mirrored mode is excluded (§3.1) | **Open — blocks CP-0** |
| R-14 | Audio/GPU behind an extra host↔container hop if playback ever moves into Docker | Not required in v1 | Playback is host-side by decision (ADR-0002) | Closed by design |
| R-15 | The application database in a named volume is readable and writable from Windows via `\\wsl$` | WSL2 exposes distro filesystems to the Windows host by design (§1.1) | Not a secret store; no secret is kept there; other containers still cannot reach it; original media protected by read-only mounts, not by volume obscurity | Accepted, documented |
| R-16 | MEDIA_HAVEN's workspace shares a VM memory ceiling and a daemon with unrelated workloads on the same machine | A shared engine or a shared WSL2 VM has one budget | Dedicated WSL2 distro and dedicated daemon (ADR-0001 Part 2); preflight detects a shared engine and reports it | Accepted, mitigated by design |
| R-17 | No AppArmor confinement under WSL2 | The WSL2 kernel does not provide it (§1.1) | Seccomp + cgroup namespaces remain; `probe` is networkless and non-root; this is a substitution, not parity | Accepted, documented |
| R-18 | WSL2 mirrored networking mode would expose published container ports to the LAN | Mirrored mode is designed to be LAN-reachable | Preflight fails on a mirrored `.wslconfig` and reports it; NAT mode required (§3.1) | Accepted, documented |

---

## 9. What this design does not claim

Written down so nobody later mistakes the diagram for a guarantee:

- It does not make the backend immune to its own bugs. It bounds the damage.
- It does not protect a user who runs the app as an administrator and installs a hostile
  Docker daemon. The daemon is trusted.
- It does not provide confidentiality against the same Windows user, including for
  data inside a named volume (§1.1, R-15).
- It does not provide AppArmor confinement on Windows (R-17).
- It does not remove the need for the lexical path policy. The mount is a backstop.
- It does not make `probe` safe. It makes `probe`'s compromise uninteresting.
