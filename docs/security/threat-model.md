# Threat model

Derived from `Development.md` §8. Required deliverable per §20.5. Reviewed at
CP-0 and revisited at CP-5.

**Scope:** the local Windows desktop appliance, its local data, its **mandatory Docker
workspace** (`core` and `probe`), the Windows container runtime that hosts it, and the
installer.

**Out of scope (for v1):** remote/internet-facing access, multi-tenant use, cloud sync,
mobile clients.

## Assets

| Asset | Why it matters | Where it lives |
|---|---|---|
| User's source media | Irreplaceable; loss is unacceptable | User-chosen folders, mounted read-only into containers |
| Library database | Watch history, resume positions, queue | Named volume `core-data` inside the WSL2 distro |
| User configuration | Folders, limits, preferences | Named volume `core-data`; host config in `%LOCALAPPDATA%/<AppName>/` |
| Managed media | Downloads, staging, trash | User-chosen managed root; named volume `staging` |
| Logs and diagnostic bundles | Contain paths and filenames if unredacted | App data dir; user-shared |
| API token | Grants access to the backend's privileged operations | Windows Credential Manager — never config, logs, image or database |
| Provider credentials (if any) | Third-party access | Windows Credential Manager — never config, logs or source |
| Host integrity | WSL2/features/firewall state | Windows host |
| The Docker daemon and its images | Root inside the VM; full control of the containers | WSL2 distro |

## Actors

| Actor | Capability | Motivation |
|---|---|---|
| Malicious media file | Malformed container/codec/subtitle/image | Exploit the player, the parser, or a thumbnailer |
| Hostile filenames and metadata | Control characters, extreme length, RTLO, crafted strings | Break rendering, spoof UI, poison logs |
| Local network peer | Reaches any non-loopback listener | Reach a management API, scrape the library |
| Other household user on the same machine | Runs as the same or another local user | Read another user's library or watch history |
| Same-user local process | Runs as the user, on the same machine | Read the API token, or read/write the database via `\\wsl$` |
| Compromised container | Runs as a service inside the VM | Escape, reach the host, reach source media |
| Compromised container image | Supply chain | Malicious code at build or pull time |
| Compromised dependency/update | Supply chain | Arbitrary code as the user |
| The user's own mistake | Confirms a destructive action | Permanent data loss |
| Curious/shared-machine snooper | Reads files on disk | Watch history, filenames |

## Entry points

1. Media file parsing (demuxer, decoder, subtitle renderer, artwork decoder).
2. Filename and embedded-metadata ingestion.
3. Local HTTP/IPC control surfaces on any local service.
4. Container image supply chain and volume mounts.
5. The updater and installer.
6. Configuration and database file loading.
7. Diagnostic bundle export (data leaves the machine).
8. Any optional provider network integration.
9. **The container runtime itself** — a compromised `dockerd`, a hostile image, or a
   container that escapes into the VM.
10. **The WSL2 boundary itself** — distro filesystem exposure via `\\wsl$`, and mirrored
    networking mode, which would make a loopback publication LAN-reachable.

## Abuse cases and mitigations

| # | Abuse case | Mitigation | Where |
|---|---|---|---|
| A1 | Crafted media triggers a parser exploit | Keep the engine patched; prefer a mature engine; **parse untrusted media in `probe`, a networkless, credential-free container — never in the privileged/UI process**; the host-side player is the accepted exception (ADR-0002, R-19) | P3-01, P0-12, P1-07 |
| A2 | Filename with control chars/RTLO spoofs the UI | Sanitise all display text; no raw path rendering; escape at the boundary | P2-02 |
| A3 | `..` / absolute / UNC path reads outside the library root | Canonicalise, then assert containment; reject reparse points/junctions; test every vector | P5-04 |
| A4 | Cleanup deletes outside the managed root | Containment check before every destructive op; staged trash; audit log | P5-02 |
| A5 | Service binds `0.0.0.0` and exposes the LAN | Loopback-only binding in **WSL2 NAT mode**; mirrored mode excluded (R-18); socket inventory test; per-install random credentials | P5-05, P0-13 |
| A6 | Container mounts home dir or system drive | Explicit narrow mounts; read-only unless writing is required; static CI check | P1-07 |
| A7 | Docker control socket reaches an untrusted container | Never mount the socket; never privileged; static CI check | P1-07 |
| A8 | Compromised image | Digest pinning; non-root; dropped capabilities; read-only rootfs; resource limits | P1-07 |
| A9 | Diagnostic bundle leaks a token, path or filename | Deny-by-default redaction with an absence test; filenames only on opt-in | P5-06 |
| A10 | Credentials in logs, source or plaintext config | Credential Manager; redaction; secret scanning in CI | P1-03, P6-02, P7-03 |
| A11 | Silent host mutation (WSL2, firewall, admin) | Read-only detection; consent screen naming every change; before/after state diff | P1-06, P1-07 |
| A12 | False "watched" triggers deletion of unwatched media | Multi-signal completion + exclusions + grace + consent + preview | P3-04, P5-02 |
| A13 | Compromised dependency or update | Pinned deps, lockfile review, scanning, checksums/signatures, SBOM | P7-01, P7-03 |
| A14 | Malicious downloaded file executes | Never execute downloaded media; no auto-open; extension is not trust | P3-01, P6-02 |
| A15 | Corrupt config/DB bricks the app | Quarantine + defaults + pre-migration backup + recovery state | P1-04, P1-05 |
| A16 | Shared machine: another user reads watch history | Data in per-user `%LOCALAPPDATA%` and in a per-user WSL2 distro; file ACLs left at user default; document that per-user data is per-user | P1-05, P6-01 |
| A17 | Piracy tooling turns the product into an infringement front-end | No bundled sources; provider interface only; legal review gate | P6-02 |
| A18 | Retry storm exhausts CPU | Bounded retries with backoff; never restart indefinitely | P5-03 |
| A19 | Compromised `probe` escapes its container into the VM | `probe` has no credentials, no database, no outbound network (internal network only, no gateway), a throwaway filesystem and `cap_drop: ALL`; treat it as hostile by design | P1-07, P6-01 |
| A20 | Compromised `core` reaches the user's original media | Media mounts are `:ro` inside every container; `core` holds the only writable paths; a path-traversal bug cannot damage the library | P1-07, P5-04 |
| A21 | An unrelated project on the same machine stops or starves our containers | Dedicated WSL2 distro and dedicated daemon (ADR-0001 Part 2); preflight detects a shared engine and reports it; `stop`, never `down`, on exit | P1-07, P6-03, R-16 |
| A22 | A same-user process reads the API token from the environment or the database | Token lives in Credential Manager, not in the volume; token+HMAC; same-user attackers are out of scope (R-10) | P1-08, P1-03 |
| A23 | A same-user process tampers with the database through `\\wsl$` | Accepted: the volume is a container-containment boundary, not a confidentiality boundary (R-15). The database holds no secret; integrity is restored by the verified backup/restore path | P1-07, ADR-0006 |
| A24 | Mirrored WSL2 networking makes the published port LAN-reachable | Preflight fails on a mirrored `.wslconfig` and reports it; NAT mode required; `portproxy` rules inventoried | P0-13, R-18 |
| A25 | The Windows host is the real trust anchor and is compromised | Out of scope by definition; documented as an explicit non-goal rather than an unstated assumption | §"Explicit non-goals" |

## Trust boundaries

1. **Media/corrupt input → app.** Nothing from a file is trusted: not its name,
   not its metadata, not its extension, not its embedded images.
2. **UI → backend.** The UI is untrusted input. Every Tauri command validates
   independently of any frontend validation.
3. **App → host.** No host change without explicit consent and a stated list.
4. **App → network.** Loopback only by default; no telemetry; any external
   request is disclosed in the UI.
5. **Container → host/filesystem.** Narrow mounts, non-root, no control socket.
6. **Container → container.** `probe` has no path to the database; `core` is the only
   holder of writable application state.
7. **Windows → WSL2.** The WSL2 filesystem is readable and writable from Windows by
   design. This boundary is weaker than it looks and is stated as R-15 rather than
   assumed away.

## Explicit non-goals

- Defending against a local administrator or kernel-level attacker.
- Defending against a same-user process on the Windows host (R-10, R-15, R-23).
- Malware analysis of the user's media. The app must be *robust to* hostile
  files, not *responsible for* judging them.
- Anonymity or anti-forensics for the user.
- Defending the Windows host against a hostile Docker daemon. The daemon is trusted.

## Residual risks accepted for v1

| ID | Risk | Why accepted | Revisit when |
|---|---|---|---|
| R-08 | WSL2/VM escape or Linux kernel exploit in the shared guest kernel | Containers share the VM kernel; the VM is the boundary | If the runtime moves to a hypervisor-isolated backend |
| R-09 | No kernel-enforced storage quota on volumes | `overlayfs` on WSL2 has no `pquota`; quotas are filesystem-dependent | If a quota-capable storage driver becomes available |
| R-10 | Same-user local process reads the API token | Windows has no per-process secret isolation from the same user; no alternative secret channel exists without a helper service | If a second machine or a multi-user profile is added |
| R-11 | Docker daemon is root inside the VM | Rootful engine is the practical default; the VM is the boundary | If rootless mode becomes viable for our mount model |
| R-12 | Malicious image or supply-chain compromise | Images come from a registry | If image signing is adopted (P7-03) |
| R-13 | WSL2 NAT networking assumption wrong | Cannot verify without measuring | P0-13 result, before CP-0 |
| R-14 | Audio/GPU behind an extra host↔container hop if playback ever moves into Docker | Not required in v1; playback is host-side by decision | If ADR-0002 is revisited |
| R-15 | Named volumes are readable and writable from Windows via `\\wsl$` | Core WSL2 behaviour; the volume holds no secret, and no other container can reach it | If a confidential store is ever added to the volume |
| R-16 | Workspace shares a VM budget and daemon with unrelated workloads | A shared engine or shared WSL2 VM has one budget | Mitigated by design (dedicated distro); revisit if a user insists on reusing Docker Desktop |
| R-17 | No AppArmor confinement under WSL2 | The WSL2 kernel does not offer it; seccomp and cgroup namespaces remain | If a backend with mandatory access control becomes available |
| R-18 | WSL2 mirrored networking would expose published ports to the LAN | Mirrored mode is excluded by design; preflight enforces it | Never, unless the exposure is mitigated by an equivalent control |
| R-19 | Third-party decoder parses hostile media in-process on the host | Accepted exception (ADR-0002); no mature way to sandbox direct local playback without heavy cost | If remote/streaming work begins |
| R-20 | Watch history readable by the same OS user | Matches per-user app-data expectations | If multi-user profiles are added |
| R-21 | No HDR guarantee | Deferred with the playback ADR | P0-12 result |

## Review log

| Date | Reviewer | Outcome |
|---|---|---|
| 2026-11-15 | contributing agent | Drafted. **Not yet reviewed by a human** — CP-0 requires human review |
| 2026-11-15 | contributing agent | Extended for the mandatory Docker workspace: added runtime actors and entry points 9–10, abuse cases A19–A25, trust boundary 6–7, and residual risks R-08…R-21. **Still not human-reviewed.** |
