# Process model

Derived from `Development.md` §2.5 and §9.2. The governing principle: keep the
number of always-running processes as low as the product allows.

## MVP processes

## Required processes and services

| Component | Required | Runs when |
|---|---|---|
| Windows Tauri host + React UI + supervisor | Yes | App is open; diagnostics/recovery UI remains available on failure |
| Docker Engine-compatible runtime distribution | Yes for backend operation | Supervisor detects, starts or reports it; distribution **Proposed** in ADR-0001 Part 2 |
| `core` backend/API container | Yes | Backend-dependent application operation |
| `probe` isolated media-inspection container | Yes (required workspace member) | Approved inspection tasks; network-isolated |
| Windows playback component | Yes | A media item is playing; host-side by decision (ADR-0002) |

Docker/Compose is the mandatory v1 backend boundary, not an optional feature. Do not run a bare-host backend fallback. Optional providers/helpers require separate approval and do not redefine the required core workspace. Runtime start/stop behavior, persistence and shutdown ownership remain unverified until P0/P6 evidence exists.

**Headless by design.** No user-facing step requires the Docker Desktop GUI, a
container dashboard or a terminal. The supervisor drives Compose through the Docker
CLI inside the WSL2 distro. Docker Desktop, if present, is a detected prerequisite
whose background engine stays resident even when its window is closed — which is
precisely why its resource cost is measured (P0-10) rather than assumed away.

**On demand.** The workspace is started when the app needs the backend and stopped
when it exits — `stop`, never `down`, so a mistaken exit cannot remove a volume.
There is no autostart service: a media app has no business running when nobody is
watching.

## Startup sequence

1. Read and validate config. On failure: quarantine, load defaults, warn, continue.
2. Open SQLite; run pending migrations after taking a backup; on failure enter
   recovery state.
3. Reconcile durable state through the required backend when its workspace is ready; retain recovery state if it is not.
4. Detect the supported runtime **read-only**. Absent or unhealthy → show the UI in recovery mode; backend-dependent features remain unavailable and never fall back to a host process.
5. Start the required workspace only through the approved supervisor lifecycle. Optional providers start only after separate explicit user action.

## Shutdown sequence

1. Persist playback position and queue state transactionally.
2. Stop services this session started, in reverse order, with a bounded timeout.
3. Close the database cleanly.
4. Flush logs.
5. Exit. No orphaned child processes — verified in P0-02 and P3-01.

## Failure behaviour

| Failure | Response |
|---|---|
| Config corrupt | Quarantine, defaults, warn, continue |
| Migration fails | Roll back, backup retained, recovery state, no data loss |
| Player crashes | Surface error, offer retry, do not mark complete |
| Optional provider fails | Bounded retries with backoff, then report; never loop |
| Required runtime/workspace unavailable | Keep diagnostics/recovery UI available; backend-dependent features unavailable; no host backend fallback |
| Disk full | Refuse the write, keep prior state intact, warn |
| Power loss | Next start recovers to last consistent state |

## Resource discipline

- No polling loops; use events or backoff.
- Pause background scans during active playback if measurement shows contention.
- Bound artwork cache size, metadata concurrency and download concurrency.
- Direct play preferred; transcoding is not a default path.
- Container CPU/memory limits are mandatory for the required workspace and must be verified against measurements.
- Idle CPU and RAM are release-gated measurements (`Development.md` §17).
