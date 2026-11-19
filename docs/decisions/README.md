# Architecture Decision Records

One file per decision. ADRs are **append-only**: to change a decision, add a new
ADR that supersedes the old one and mark the old one `Superseded by ADR-NNNN`.
Never rewrite an accepted ADR's decision.

## Index

| ADR | Title | Status | Evidence needed |
|---|---|---|---|
| [0001](0001-windows-container-runtime.md) | Mandatory Docker boundary and Windows runtime distribution | Mandatory boundary **Accepted**; distribution **Accepted (provisional)** (dedicated WSL2 + Docker Engine) | P0-10, P0-13, P0-14 |
| [0002](0002-playback-engine.md) | Playback engine and placement | **Accepted (provisional)** — mpv via JSON IPC, abstract interface | P0-11, P0-12, O-09 |
| [0003](0003-modular-monolith.md) | Modular monolith | Accepted; deployment boundary reconciled by ADR-0008 | — |
| [0004](0004-local-playback-first.md) | Local HDMI playback first | Not started; requires human acceptance | P0-01 |
| [0005](0005-host-container-boundary.md) | Host/container transport and authentication | Proposed; not verified | P0-13, P1-08 |
| [0006](0006-filesystem-and-volume-policy.md) | Filesystem and volume policy | **Accepted (provisional)** | P1-07, P6-03 |
| [0007](0007-secrets-management.md) | Secrets management | **Accepted (provisional)** — token storage mechanism amended by ADR-0014 | P1-03, P1-08 |
| [0008](0008-modular-monolith-deployment-boundary.md) | Modular monolith across Windows host and Docker backend | Proposed; human review required | P0-07, P1-08 |
| [0009](0009-frontend-first-contract-driven.md) | Frontend-first, contract-driven development | Accepted | — |
| [0010](0010-library-entity-deviations.md) | Library entity deviations and scanner conventions | Proposed | P2-02, P2-03 |
| [0011](0011-metadata-provider-selection.md) | Metadata provider selection | **Accepted** (2026-11-18) — TMDB primary, local NFO offline fallback, network opt-in | P2-06, O-04 |
| [0013](0013-backend-completion-sequencing.md) | Containerise the backend before the remaining features | **Accepted** (2026-11-18) | P1-08, P4-08 |
| [0014](0014-host-token-storage.md) | Host token storage uses DPAPI | **Accepted** (2026-11-18) — amends ADR-0007 for the API token | P1-08, O-07 |
| [0015](0015-playback-descriptor.md) | Playback source resolved host-side from the media id — no path or token in the renderer | **Proposed** — recommendation only, not accepted | P3-01, P4-06 |

**Two decisions are already binding and are not reopened by Phase 0:**

1. **Docker is mandatory for v1** (ADR-0001 Part 1). The backend and the media-inspection
   workload run in containers. There is no host-side backend fallback.
2. **The frontend never receives Docker daemon access** (ADR-0005 §3). Lifecycle
   operations go through the host supervisor only.

**Still open, and each needs human acceptance before code depends on it:** the Windows
runtime distribution (ADR-0001 Part 2), the playback engine and its placement (ADR-0002),
the host/container transport and auth scheme (ADR-0005), the volume policy (ADR-0006),
the secret-provisioning mechanism (ADR-0007) and the deployment-boundary rules
(ADR-0008). The metadata provider is no longer open: ADR-0011 was accepted by the
human partner on 2026-11-18 (TMDB primary, local NFO offline fallback).

## Status values

- **Proposed** — written, not approved. Code must not depend on it.
- **Accepted** — approved by the human partner. Binding.
- **Rejected** — considered and declined; record why, so it is not revisited.
- **Superseded** — replaced by a later ADR; keep for history.
- **Deprecated** — still true, but no longer recommended for new work.

## Rules

- Only the human partner may move an ADR to **Accepted**. An agent may write a
  Proposed ADR, present evidence, and recommend. It may not self-approve.
- An ADR without recorded evidence is a preference, not a decision. Cite
  measurements, test output or documentation pages.
- Every ADR must state what it costs: memory, disk, licence, maintenance, risk.
- If implementation reveals the ADR is wrong, stop and write a superseding ADR.
  Do not quietly deviate.

## Template

```markdown
# ADR-NNNN: Title

- **Status:** Proposed | Accepted | Rejected | Superseded by ADR-NNNN
- **Date:** YYYY-MM-DD
- **Deciders:** human partner (named), contributing agent
- **Related:** roadmap task IDs, other ADRs, Development.md sections

## Context
What forces this decision? What is measured vs assumed?

## Decision
The decision, stated in one paragraph, in the active voice.

## Alternatives considered
| Option | Pros | Cons | Verdict |

## Consequences
Positive, negative, and what becomes harder.

## Evidence
Measurements, test output, documentation links.

## Revisit trigger
The condition under which this must be reopened.
```
