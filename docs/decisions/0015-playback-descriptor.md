# ADR-0015: Playback source is an opaque token resolved host-side, never a path in the renderer

- **Status:** **Proposed — recommendation only; not accepted.** Code must not depend on it (see the ADR index rules). Only the human partner may accept it.
- **Date:** 2026-11-19
- **Deciders:** human partner (acceptance pending), contributing agent
- **Related:** ADR-0002 (playback on the Windows host via mpv JSON IPC; the containerised backend stays the authority on *what* to play), ADR-0005 §3 (named commands only, no path to the renderer, no generic passthrough) and §5 (failures must be legible), ADR-0009 / roadmap P4-06 (typed command and DTO contract), ADR-0013 (containerise before features), plan `2026-11-18-backend-completion.md` tasks B1/B2, roadmap P3-01 … P3-05. D-6 (autoplay off by default) is in force and is not reopened here.

## Context

Task A5 created 24 named commands. `playback_start` currently returns a `PlaybackSession` and the renderer still shows a simulated clock; there is no stream route and no playback engine yet (B2+). The containerised `core` is the authority on what to play — it owns the library, resume position and policy (ADR-0002).

Task A9 (`apps/desktop/src-tauri/src/mounts.rs`) produces a `RenderOutput { fragment, mounts }` with a host-path map (`HostPathMap`) that is deliberately kept host-side. Container paths are `/media/root-<n>` by position. That map is the only legitimate bridge from a container-path identity to a host file.

The problem this ADR resolves: ADR-0002 places playback on the Windows host, but the file's identity lives in the container. The host must learn which file to open without any filesystem path ever crossing into the WebView. ADR-0005 §3 forbids a path in a DTO and forbids a generic passthrough; P4-06 outlaws "filesystem paths that the frontend can act on"; R-24 requires that resolution never escape a declared root.

Measured vs assumed: the command list, the `RenderOutput`/`HostPathMap` shape and the "no path in a DTO" rule are observed in the current code and contracts. The token round-trip described below is a design proposal, not yet implemented or tested.

## Decision

### Recommendation (Proposed — pending human acceptance, not Accepted)

**`playback_start` returns the session plus an opaque `sourceToken`.** The host resolves `sourceToken` through its own `HostPathMap` from A9 to a host path, and hands that host path only to mpv. Neither the token nor the resolved path is returned to, or usable by, the renderer. Playback is driven by a host-side command `playback_open(mediaId) -> EngineHandle`; the renderer sees only named commands and typed, display-only DTOs. The backend remains the authority on what to play (ADR-0002); the host decides only how to open the already-authorised file.

This is a recommendation. It is **not** accepted and must not be treated as binding until the human partner moves this ADR to Accepted.

### Containment rule

- No filesystem path, host or container, crosses into the renderer. `sourceToken` is an opaque identifier with no path semantics the WebView can interpret or act on; it lives host-side alongside the mount map.
- Resolution happens **only** in the host, **only** through the A9 `HostPathMap`, and **only** to a file under a declared read-only media root. The host never accepts a raw path from the container or the renderer, and never widens a resolution beyond the declared roots (R-24).
- The capability surface stays named commands (ADR-0005 §3): `playback_open` is one operation, not an `http_request`-style passthrough. The player never talks to the backend API directly (ADR-0002); it receives a resolved host path and a start position from the host and nothing else.

### Failure mode when a file moves between container and host views

A media item recorded by the container has a container-path identity (`/media/root-<n>/…`); the host resolves that identity to a host path through the mount map. The container's bind-mounted view and the host filesystem can diverge — a file can be moved, renamed, deleted on the host, or a root re-mounted so that the recorded container path now points somewhere else.

In that case resolution **fails closed**: if the token does not resolve through the current map, or the resolved path is missing, is no longer under a declared root, or points to a different file than the one authorised, the host does **not** open a guessed path and does **not** pass any path back to the renderer. It surfaces a legible `source unavailable` state (ADR-0005 §5) with a re-scan/re-provision affordance, and playback does not start. There is no fallback that guesses a new path, and no silent retry loop. A token is single-use for one resolution attempt and is bound to the session that produced it, so a stale token cannot be replayed against a later, different file at the same path.

## Alternatives considered

| Option | Pros | Cons | Verdict |
|---|---|---|---|
| Opaque `sourceToken` resolved host-side through the A9 map (this recommendation) | No path reaches the renderer; reuses the mount map A9 already builds; backend stays the authority on what to play (ADR-0002); fails closed on a moved file | Adds a token round-trip and a host-side resolve step; token/session binding must be implemented and tested (B2) | **Recommended — not yet accepted** |
| Return a raw filesystem path in the `playback_start` DTO | Simplest to implement | Forbidden by ADR-0005 §3 and P4-06; leaks a host path into the WebView and hands the renderer an actionable path; any XSS becomes a file-open primitive | Rejected |
| Stream file bytes through the API to a renderer/host player | Avoids exposing any path at all | Unnecessary on one machine; reintroduces the encode/extra-decode latency and bandwidth ceiling ADR-0002 rejected; still needs a path somewhere to read the bytes | Rejected |
| Generic passthrough command (`http_request`/`api_request`) | Flexible | Violates ADR-0005 §3 (named commands only) and R-23; expands the renderer's capability surface | Rejected |

## Consequences

Positive: the renderer never holds a path or a host detail; the container→host boundary is crossed in exactly one host-side place that already owns the mount map; a moved file fails legibly instead of opening the wrong bytes.

Negative, and accepted: `playback_start` grows an opaque token field and the host must resolve it before mpv can open anything — one more step in the critical path to a playing file. The token must be bound to a session and short-lived so it cannot be replayed (part of B2's work, not decided away here).

Harder: the mount map and the library's container-path identity must stay consistent. Renaming or re-rooting a media root between scan and playback invalidates outstanding tokens; B2 must resolve against the *current* map and treat staleness as a legible failure, not silently re-map. This is the same consistency R-24 guards, now made explicit for playback.

## Revisit trigger

Reopen this ADR if: playback moves into the container or a stream route is genuinely required (which would supersede ADR-0002, not quietly change this ADR); a second machine or user is introduced so a single host-side mount map no longer applies; or the human partner requires an inspectable/revocable source handle that an opaque token cannot provide. Do not deviate from this shape in code without a superseding ADR.

## Verification

No tests for this decision (it is a documentation gate). B2 carries the executable evidence:

- a test that a valid `sourceToken` resolves through the A9 `HostPathMap` to a host path under a declared root, and that the resolved path is handed only to the player;
- a test that an unknown, stale or replayed token, and a token whose file has moved or been deleted, fail closed with a legible `source unavailable` state and no path in any renderer-facing DTO;
- a check that no path or token field appears in renderer-facing DTOs or the production bundle (consistent with A7/A8's bundle scans).
