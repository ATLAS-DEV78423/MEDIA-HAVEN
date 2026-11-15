# infrastructure/containers

Container configuration for the **required** v1 Docker/Compose backend boundary.

> **Status:** Docker/Compose is **mandatory** for v1 (ADR-0001 Part 1 — the backend and
> the media-inspection workload run in containers; there is no host-side backend
> fallback). The supported Windows runtime distribution is still **Proposed**
> (ADR-0001 Part 2: Docker Engine in a dedicated WSL2 distro).
>
> `compose.yaml` now implements the required `core`/`probe` workspace from
> `docs/architecture/docker-workspace.md`. It **resolves cleanly** (`docker compose
> config --quiet` exits 0) and its static security posture has been checked, but it has
> **not** been run against a real engine. P0-10/P0-13/P0-14 and P6-03 are the gates.
> Do not present this file as a deployed boundary.

## Files

| File | Purpose |
|---|---|
| `compose.yaml` | The required v1 workspace: `core` (backend) and `probe` (isolated media inspection) |
| `README.md` | This file. |

## The workspace

Two containers. Each has exactly one responsibility. No container exists because it
was technically possible.

| Service | Responsibility | User | Network | Writable state |
|---|---|---|---|---|
| `core` | HTTP API, product policy, SQLite, job queue, storage policy | `10001:10001` | `edge` (published loopback port) + `internal` | `core-data`, `staging` volumes; `/tmp` tmpfs |
| `probe` | `ffprobe` metadata, thumbnails, trickplay — the only component that opens untrusted media bytes | `10002:10002` | `internal` only (no gateway) | `staging` volume; `/tmp` and `probe-tmp` tmpfs |

`probe` has no credentials, no database handle and no route to the internet or the LAN.
If it is fully compromised, the blast radius is a container that cannot reach anything.

## Hard rules

1. The user never sees a container dashboard. Ever.
2. No persistent data in a container's writable layer.
3. No Docker control socket in any workload container.
4. No privileged containers.
5. Images pinned by digest; the digest is a **required variable** so Compose fails
   closed rather than pulling `:latest`. Updates are deliberate.
6. Original media is mounted read-only and only through individually selected,
   validated roots; mounts are rendered per root by the Windows supervisor, so adding a
   root never widens an existing mount.
7. WSL2 must be in **NAT networking mode**. Mirrored mode makes the instance
   LAN-reachable and is excluded by design (R-18).
8. Required-workspace lifecycle and persistent-state behavior must be bounded and
   tested before release.
9. Any API publication must be loopback-only, with actual Windows/WSL/LAN reachability
   tested before claiming isolation.

## Why the stack is minimal

`Development.md` §2.4.1 mandates an intentionally small Compose design. Every
service must justify its memory, disk and attack surface. The default answer to
"should this be a container?" is no.

The Tauri UI and Windows playback are **not** containers — the UI needs a native
window and compositor, and playback needs the Windows GPU, audio endpoint and vsync
(ADR-0002). Additional providers such as downloads are optional and require separate
legal/security approval. Do not add services without a measured need and review.

## Running it locally

The supervisor supplies the required variables. For a parse-only check:

```bash
MEDIAHAVEN_CORE_IMAGE="example/core@sha256:$(printf '0%.0s' {1..64})" \
MEDIAHAVEN_PROBE_IMAGE="example/probe@sha256:$(printf '1%.0s' {1..64})" \
MEDIAHAVEN_API_PORT=18081 MEDIAHAVEN_API_TOKEN=placeholder \
  docker compose -f infrastructure/containers/compose.yaml config --quiet
```

`docker compose up` is **not** a supported entry point: the Windows supervisor owns the
lifecycle, the token provisioning and the per-root mount rendering. Full checklist:
`docs/security/compose-security-checklist.md`.
