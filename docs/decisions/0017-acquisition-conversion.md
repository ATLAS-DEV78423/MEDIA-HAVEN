# ADR-0017: Adopt the torrent-acquisition conversion; supersede local-playback-first

- **Status:** Accepted (2026-11-20)
- **Date:** 2026-11-20
- **Decider:** human partner
- **Scribe:** contributing agent
- **Authority:** the human partner wrote, verbatim, "i accept the ADRs" after reading
  `docs/superpowers/plans/2026-11-20-torrent-acquisition-conversion.md`, which named
  ADR-0017, ADR-0018 and ADR-0019 as the three acceptance gates.
- **Related:** `media_haven_consolidated_specification.md`, ADR-0004, O-05, ADR-0016,
  `Development.md` §1.5/§18/§20, `docs/superpowers/plans/2026-11-20-torrent-acquisition-conversion.md`

## Context

The target specification describes an app that acquires authorized media via
qBittorrent in a Docker sandbox, maintains a rolling two-episode buffer, and
coordinates cleanup of watched, app-managed media. The existing MEDIA_HAVEN is a
local-first player for user-owned files. The target reverses earlier decisions.

## Decision

1. `media_haven_consolidated_specification.md` is adopted as the target architecture.
2. **ADR-0004** (local HDMI playback first; remote/acquisition deferred) is
   **superseded** by this ADR. Local playback remains; acquisition is added.
3. **O-05** (whole download workstream deferred out of v1) is **reopened and
   reversed**: acquisition is now in scope.
4. Local-first browsing and playback of already-acquired media still work fully
   offline (O-03, accepted). Only acquisition needs a network.
5. The already-built frontend, Rust `core`, SQLite, named-command boundary,
   Docker hardening and mpv playback are **reused**, not replaced.

## Consequences

- New subsystems (acquisition orchestrator, qBittorrent sandbox, prefetch/cleanup
  scheduler) are added per the conversion plan.
- The project's prior "no acquisition / downloads deferred" contract text in
  `Development.md` and `roadmap.md` must be updated to reference this ADR.
- The nine open design gates G-1…G-9 in the conversion plan still need human
  choices before gated tasks; this ADR does not decide them.

## Revisit trigger

The human partner reverts this conversion, or a lawful-use review changes the
scope of permitted acquisition.
