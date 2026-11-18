# ADR-0013: Containerise the backend before the remaining features

- **Status:** Accepted (2026-11-18)
- **Date:** 2026-11-18
- **Deciders:** human partner (accepted 2026-11-18), contributing agent
- **Related:** ADR-0001, ADR-0005, ADR-0008, ADR-0014, plan `2026-11-18-backend-completion.md` (D-1)

## Context

The backend currently runs as a host process (`cargo run -p mediahaven-core`).
`infrastructure/containers/` holds a Compose file and Dockerfile that have never
been run, and `apps/desktop/src-tauri/` does not exist. Playback is simulated in
the UI, the queue has routes but no UI, and the renderer talks HTTP directly
with a development token — a shape ADR-0005 does not allow to ship.

Completing the remaining features on the host first would be faster to a
playing file, but three of the expensive pieces — token provisioning, the
host-to-container transport, and the container→host media path map — are exactly
what playback depends on. Building playback against a host path and then
changing where files live means building that mapping twice and re-testing
resume, completion and recovery afterwards.

## Decision

**Containerise first.** The boundary work (Tauri shell, token provisioning, host
supervisor and Compose lifecycle, named command surface, container build, path
map) lands before playback, queue and the remaining feature work. Feature
implementations then target their final process and trust boundaries.

## Acceptance record

- **2026-11-18 — accepted by the human partner.** Presented with three options
  (containerise first, host first, or leave the decision open), the human
  partner chose **"Containerise first"**. Recorded here by the agent at the
  human's direction.

## Alternatives considered

| Option | Pros | Cons | Verdict |
|---|---|---|---|
| Containerise first | Path map, token and transport built once; playback targets real boundaries; no second migration of features | Nothing plays until the shell, supervisor and container build exist | **Accepted** |
| Host first, container later | Reaches a playing file sooner | Path mapping built twice; token design changes at the move; every feature retested after the move | Not chosen |
| Leave the decision open | No commitment | Blocks the plan; invites work that assumes the host shape | Not chosen |

## Consequences

Positive: one migration, not two. The security boundary that v1 requires exists
before features depend on it, and the renderer never ships holding a token.

Negative: the first visible product progress is a window that renders the UI
inside Tauri and talks to a containerised `core` — not a film playing. The
dependency chain is longer before CP-3, and container startup time becomes part
of every development loop.

Harder: quick local experiments must go through Compose rather than a bare
`cargo run`. That is the intended cost, not something to engineer around with a
host-mode fallback, which ADR-0001 Part 1 forbids.

## Revisit trigger

The container build cannot be made to run on the reference machine within the
P0-10/P0-13 evidence budget, or a Windows/WSL2 limitation makes the
host-to-container transport unusable. In that case write a superseding ADR;
do not quietly keep building on the host.
