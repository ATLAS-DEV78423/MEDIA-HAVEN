# Compose security checklist

Required deliverable per `Development.md` §20.5. Use for the required Docker/Compose backend workspace before implementation/release, and in CI as a static check where possible.

> **Status:** Docker/Compose is **mandatory** for the v1 backend boundary (ADR-0001 Part 1).
> `infrastructure/containers/compose.yaml` now implements the required `core`/`probe`
> workspace described in `docs/architecture/docker-workspace.md`. It parses and resolves
> cleanly, but it has **not** been run against a real engine, and the runtime
> distribution is still **Proposed** (ADR-0001 Part 2). P0-10/P0-13/P0-14 and P6-03 are
> the gates. This checklist is not proof of a deployed boundary.

## Identity and privilege

- [ ] Every service runs as a **non-root user** (`user:` set explicitly).
- [ ] `privileged: true` appears nowhere.
- [ ] `cap_drop: [ALL]` is set, with only the specific capabilities re-added if
      genuinely required — and the reason recorded inline.
- [ ] `no-new-privileges: true` is set.
- [ ] `read_only: true` root filesystem, with explicit tmpfs or volumes for the
      paths that must be writable.
- [ ] The Docker control socket is **never** mounted into any workload.
- [ ] No service runs with `pid: host`, `network_mode: host` or `ipc: host`.
- [ ] `probe` runs as a **different UID** from `core`, so compromising one does not
      grant the other's writable paths.
- [ ] `probe` receives **no credentials and no database handle**.

## Images

- [ ] Every image is pinned **by digest**, not only by tag.
- [ ] The digest is supplied as a **required variable** so Compose fails closed when it
      is missing — never a committed mutable tag.
- [ ] Base images are minimal; no build toolchains in the runtime image.
- [ ] Images come from official or first-party registries.
- [ ] The digest and version are recorded in the release notes.
- [ ] Image updates are a deliberate act with a recovery plan, never automatic.

## Network

- [ ] Services live on a private Compose network.
- [ ] Only the minimum endpoint is published, bound to `127.0.0.1`.
- [ ] Nothing binds `0.0.0.0` unless an ADR justifies it.
- [ ] No management dashboard or WebUI is reachable from the LAN.
- [ ] Any control API requires a per-install random credential.
- [ ] `probe` is attached **only** to an `internal: true` network with no gateway.
- [ ] **WSL2 is in the default NAT networking mode.** Mirrored mode makes the instance
      LAN-reachable and must be detected by preflight, not worked around (R-18).
- [ ] No `netsh interface portproxy` rule exists that could forward the published port.

## Storage

- [ ] Persistent state uses **named volumes** or explicitly managed host paths.
- [ ] No persistent data lives in the container writable layer.
- [ ] Source media is mounted **read-only**, and only the specific folders.
- [ ] The home directory, user profile and system drive are never mounted.
- [ ] The download service sees only its managed directory, read-write.
- [ ] Managed layout is `incoming/`, `ready/`, `quarantine/`, `trash/`.
- [ ] **No secret is stored in a volume.** WSL2 exposes distro filesystems to Windows via
      `\\wsl$`, so a volume is not a secret store (R-15).

## Resources and lifecycle

- [ ] CPU and memory limits are set per service.
- [ ] PID limits are set per service.
- [ ] tmpfs mounts are size-capped and carry `noexec,nosuid,nodev`.
- [ ] Health checks are defined and meaningful (not `true`).
- [ ] Restart policy is bounded — no unconditional infinite restart.
- [ ] Services stop when the app exits; no orphaned containers.
- [ ] Required `core`/`probe` workspace startup is bounded and owned by the Windows supervisor; optional providers alone are gated on separate approval.

## Verification commands

Run against the actual Compose project and record the real output in `progress.md`:

```bash
# validate the file resolves (supply throwaway digests; this parses only)
MEDIAHAVEN_CORE_IMAGE="example/core@sha256:$(printf '0%.0s' {1..64})" \
MEDIAHAVEN_PROBE_IMAGE="example/probe@sha256:$(printf '1%.0s' {1..64})" \
MEDIAHAVEN_API_PORT=18081 MEDIAHAVEN_API_TOKEN=placeholder \
  docker compose -f infrastructure/containers/compose.yaml config --quiet

# no privileged containers, no socket mounts, no host networking
grep -nE "privileged|/var/run/docker.sock|network_mode" infrastructure/containers/compose.yaml

# every image pinned by digest
grep -nE "^\s+image:" infrastructure/containers/compose.yaml | grep -v "@sha256:" || echo "OK: all pinned"

# the published port must be loopback-only
docker compose -f infrastructure/containers/compose.yaml config | grep -A2 "published" | grep "host_ip: 127.0.0.1"
```

A finding in the `grep` checks fails the release. There is no "known exception" list; if
an exception is truly needed it becomes an ADR.
