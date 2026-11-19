# ADR-0015: Playback source is resolved host-side from the media id — no path or token in the renderer

- **Status:** **Proposed — recommendation only; not accepted.** Code must not depend on it (see the ADR index rules). Only the human partner may accept it.
- **Date:** 2026-11-19
- **Deciders:** human partner (acceptance pending), contributing agent
- **Related:** ADR-0002 (playback on the Windows host via mpv JSON IPC; the containerised backend stays the authority on *what* to play), ADR-0005 §3 (named commands only, no path to the renderer, no generic passthrough) and §5 (failures must be legible), ADR-0009 / roadmap P4-06 (typed command and DTO contract), ADR-0013 (containerise before features), plan `2026-11-18-backend-completion.md` tasks B1/B2, roadmap P3-01 … P3-05. D-6 (autoplay off by default) is in force and is not reopened here.

## Context

Task A5 created 24 named commands. `playback_start` is a **renderer-facing** named command (`apps/desktop/src/services/api.ts`, `apps/desktop/src-tauri/src/commands/renderer.rs`); today it returns a `PlaybackSession` and the renderer still shows a simulated clock. There is no stream route and no playback engine yet (B2+). The containerised `core` is the authority on what to play — it owns the library, resume position and policy (ADR-0002).

Task A9 (`apps/desktop/src-tauri/src/mounts.rs`) produces a `RenderOutput { fragment, mounts }`. Its `RenderOutput::host_path_map()` returns a `BTreeMap<String, PathBuf>` keyed by container path (`/media/root-<n>` by position), deliberately kept host-side. That map is the only legitimate bridge from a container-path identity to a host file; there is no separate `HostPathMap` type.

The problem this ADR resolves: ADR-0002 places playback on the Windows host, but the file's identity lives in the container. The host must learn which file to open without any filesystem path or source handle ever crossing into the WebView. ADR-0005 §3 forbids a path in a DTO and forbids a generic passthrough; P4-06 outlaws "filesystem paths that the frontend can act on"; R-24 requires that resolution never escape a declared root.

Measured vs assumed: the command list, the `RenderOutput::host_path_map()` shape and the "no path in a DTO" rule are observed in the current code and contracts. The host-side resolution flow described below is a design proposal, not yet implemented or tested.

## Decision

### Recommendation (Proposed — pending human acceptance, not Accepted)

**`playback_start` stays a renderer-facing named command whose `PlaybackSession` DTO contains no filesystem path and no source token.** When the host handles `playback_start`, it uses the requested media id to find the item's container-path identity, resolves that to a host path through `RenderOutput::host_path_map()` from A9, and hands that host path only to mpv via the host-internal `playback_open(mediaId) -> EngineHandle`. The renderer sees named commands and typed, display-only DTOs; it never sees a path or a token. The backend remains the authority on what to play (ADR-0002); the host decides only how to open the already-authorised file.

**Why there is no opaque `sourceToken`:** the host already holds the media id and the mount map, so a token would add no containment. Returned in the `playback_start` DTO it would put a file-locating handle in the renderer (the defect this decision exists to prevent); kept host-side it would be redundant with the media id. We drop it. The renderer's only handle is the media id it already sends.

This is a recommendation. It is **not** accepted and must not be treated as binding until the human partner moves this ADR to Accepted.

### Containment rule

- No filesystem path, host or container, and no source token crosses into the renderer. `playback_start`'s DTO is display-only session state (ids, status, timings); the media id it carries is a library identifier, not a path. Resolution and the resolved path stay inside the host process.
- Resolution happens **only** in the host, **only** through A9's `RenderOutput::host_path_map()`, and **only** to a file under a declared read-only media root. The host never accepts a raw path from the container or the renderer, and never widens resolution beyond the declared roots (R-24).
- The capability surface stays named commands (ADR-0005 §3): `playback_open` is a **host-internal** operation, not a renderer-invocable command and not an `http_request`-style passthrough. The player never talks to the backend API directly (ADR-0002); it receives a resolved host path and a start position from the host and nothing else.

### Failure mode when a file moves between container and host views

A media item recorded by the container has a container-path identity (`/media/root-<n>/…`); at `playback_start` the host resolves that identity to a host path through `RenderOutput::host_path_map()`. The container's bind-mounted view and the host filesystem can diverge — a file can be moved, renamed or deleted on the host, or a root re-mounted so that the recorded container path no longer maps to the same bytes.

In that case resolution **fails closed**: if the map no longer contains the container path, or the resolved host path is missing, is no longer under a declared root, or is not the file the media id authorises, the host does **not** open a guessed path and does **not** pass any path back to the renderer. It surfaces a legible `source unavailable` state (ADR-0005 §5) with a re-scan/re-provision affordance, and playback does not start. There is no fallback that guesses a new path and no silent retry loop. Resolution is performed once, at command time, against the current map.

## Alternatives considered

| Option | Pros | Cons | Verdict |
|---|---|---|---|
| Host resolves the media id via `RenderOutput::host_path_map()` (this recommendation) | No path or token reaches the renderer; reuses the map A9 already builds; backend stays the authority on what to play (ADR-0002); fails closed on a moved file | Adds a host-side resolve step to the critical path to playback | **Recommended — not yet accepted** |
| Return an opaque `sourceToken` in the `playback_start` DTO | Looks opaque, so it appears not to be a path | `playback_start` is renderer-facing; this hands the renderer a field whose only purpose is locating a file, violating the no-path/no-handle rule (ADR-0005 §3, P4-06), plus token lifecycle and replay work for no containment gain. The host already owns the map | Rejected — resolves the ambiguity in the brief's proposed shape |
| Return a raw filesystem path in the `playback_start` DTO | Simplest to implement | Forbidden by ADR-0005 §3 and P4-06; leaks a host path into the WebView and hands the renderer an actionable path; any XSS becomes a file-open primitive | Rejected |
| Stream file bytes through the API to a renderer/host player | Avoids exposing any path at all | Unnecessary on one machine; reintroduces the encode/extra-decode latency and bandwidth ceiling ADR-0002 rejected; still needs a path somewhere to read the bytes | Rejected |
| Generic passthrough command (`http_request`/`api_request`) | Flexible | Violates ADR-0005 §3 (named commands only) and R-23; expands the renderer's capability surface | Rejected |

## Consequences

Positive: the renderer never holds a path, a source handle or a host detail; the container→host boundary is crossed in exactly one host-side place that already owns the mount map; a moved file fails legibly instead of opening the wrong bytes.

Negative, and accepted: the host must resolve the media id to a host path before mpv can open anything — one more step in the critical path to a playing file — and the `PlaybackSession` DTO is deliberately thinner than a token-carrying shape.

Harder: the mount map and the library's container-path identity must stay consistent. Renaming or re-rooting a media root between scan and playback invalidates the recorded container path; B2 must resolve against the *current* map and treat staleness as a legible failure, not silently re-map. This is the same consistency R-24 guards, now made explicit for playback.

## Revisit trigger

Reopen this ADR if: playback moves into the container or a stream route is genuinely required (which would supersede ADR-0002, not quietly change this ADR); a second machine or user is introduced so a single host-side mount map no longer applies; or the human partner requires an inspectable/revocable source handle that this media-id resolution cannot provide. Do not deviate from this shape in code without a superseding ADR.

## Verification

No tests for this decision (it is a documentation gate). B2 carries the executable evidence:

- a test that, when the host handles `playback_start`, the media id resolves through `RenderOutput::host_path_map()` to a host path under a declared root, and that the resolved path is handed only to the player;
- a test that a stale, moved or deleted source fails closed with a legible `source unavailable` state and no path in any renderer-facing DTO;
- a check that no path or source handle appears in renderer-facing DTOs or the production bundle (consistent with A7/A8's bundle scans).
