# First-run setup

The first-run experience must explain the required local backend runtime in plain language to someone unfamiliar with WSL, virtualisation or containers. It must not change the machine without explicit consent.

Implemented by `roadmap.md` P1-06 using `scripts/windows/preflight.ps1`.

## Principles

1. **Read before write.** Every check is read-only. No installs, no feature
   toggles, no administrator prompt.
2. **Explain the why.** Explain that the local backend requires a supported Docker-compatible runtime, why it is required, and what host changes might be needed. Do not promise that setup can be skipped while retaining backend functionality.
3. **No host-side fallback.** If the required runtime is unavailable, the app shows a recoverable prerequisite state. Never run backend policy or database functionality outside the approved boundary.
4. **Clean abort.** Cancelling at any step leaves no partial configuration.
5. **No dead ends.** A failed check produces a next step, not an error code.

## Headless operation — the user never touches Docker

The product must work **without the Docker Desktop graphical interface**. The GUI is
never required for normal operation: containers are orchestrated through Compose and the
Docker CLI, driven by the Windows supervisor. A user must never need to open a terminal,
type a `docker` command, or click through a container dashboard.

What this means for the design:

- The application starts, stops, restarts and monitors its own containers.
- Docker Desktop, if present, is a **detected prerequisite**, never a UI the user
  has to keep open. Its window may be closed at any time and the containers keep
  running; its background engine stays resident, which is exactly why its resource
  cost and its licence terms are measured rather than assumed (ADR-0001, P0-10).
- If an existing container runtime is detected, the app reports which one it found and
  asks before using it. It never silently reuses a shared engine, because an unrelated
  project's `docker compose down` must not be able to stop ours (ADR-0001 Part 2, R-16).
- WSL2's **mirrored** networking mode is detected and reported as blocking: it would
  make the backend's published port reachable from the household LAN (R-18).
- Containers start when needed and stop when the app exits. `stop`, never `down`, so a
  mistaken exit cannot remove a volume.

## Preflight checks

`scripts/windows/preflight.ps1` reports, without modifying anything:

| Check | Why | Failure behaviour |
|---|---|---|
| Windows edition, version, build | Compatibility and support claims | Warn if below the supported build; do not block local playback |
| Architecture (x64 / ARM64) | Installer and engine choice | Block only if genuinely unsupported; state it plainly |
| RAM total and available | Tier expectations — the required runtime has a real memory floor | Warn using evidence-backed thresholds; all supported profiles include required runtime overhead |
| Free disk on target roots | Storage and import viability | Block import if critically low; suggest another drive |
| Virtualisation capability (read-only) | Whether the required runtime could work | State prerequisite and recovery action; never enable it |
| Existing Docker/runtime detection | Detect a supported usable runtime without modification | Required prerequisite; explain supported next steps, never modify an existing install |
| WSL2 present, version and networking mode | The required runtime depends on WSL2; mirrored mode breaks loopback isolation | Report; mirrored mode is a blocking condition, never worked around |
| Shared-runtime detection | An existing engine shared with other projects is a resource and blast-radius risk | Report and ask; never reuse silently |
| Port availability | The published API port | Informational at this stage |
| WebView2 presence | Tauri runtime requirement | Actionable message with the official installer link |
| Existing app data | Upgrade vs first run | Offer recovery/backup rather than overwrite |

## Flow

1. **Welcome** — what the app does, what it will never do (no uploads, no
   deletion of your files, no account).
2. **Compatibility summary** — the preflight results in plain language, grouped
   as "ready", "recommended", "needs attention".
3. **Media folders** — pick one or more folders. Show how many files were found.
   Nothing is read until the user confirms.
4. **Managed storage location** — where downloads, staging and trash live, with
   free-space validation. May be a different drive.
5. **Required backend runtime** — clearly explain that the backend needs the selected Docker-compatible runtime, that it runs headlessly with no Docker interface, and that MEDIA_HAVEN never modifies the user's media. Show every prerequisite and host change, require explicit consent before any change, and offer a safe exit/recovery path. Declining or lacking the runtime leaves backend-dependent features unavailable; there is no local-library backend fallback.
6. **Optional services** — list only separately approved provider integrations; they do not change the required backend boundary.
7. **Review** — a single summary of exactly what will be created and where, including runtime/workspace changes.
8. **Confirm** — explicit action. Nothing is written or installed before this point.
9. **Done** — open the library only when the required backend is ready; otherwise show recovery guidance.

## What setup must never do

- Install software or a runtime.
- Enable WSL2, Hyper-V, virtualisation or any Windows optional feature.
- Request administrator rights.
- Change firewall, proxy, DNS or security settings.
- Scan the whole profile, Desktop, Documents or all drives.
- Write configuration before the final confirmation.
- Treat the required runtime as optional or silently start a host-side backend when the user declines a prerequisite.

## Post-install verification

After setup, the app writes a setup record used by diagnostics:

- Which checks passed, warned or failed.
- Which folders were added (counts, not full listings).
- Which optional features were enabled or declined.
- The storage root's free space at setup time.

This record contains no personal paths in a form that would leak into a
diagnostic bundle — it is redacted like any other log (P5-06).

## Recovery

If setup was interrupted, the next launch detects the incomplete state and offers to resume or restart from the beginning. It never half-applies configuration. If the runtime is absent or unhealthy, the UI remains available for diagnostics/recovery, but backend-dependent features stay unavailable.
