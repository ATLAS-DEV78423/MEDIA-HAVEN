# Documentation index

Start with `Development.md` at the repository root. It is the contract; this
directory holds the supporting detail.

This index is the map. If a document is missing, it has not been written yet —
check `roadmap.md` for the task that creates it rather than inventing content.

## architecture/

| File | Contents | Status |
|---|---|---|
| `overview.md` | System shape, process model, module boundaries, data flow | Written |
| `docker-workspace.md` | The required Docker/Compose workspace: container inventory, network boundary, filesystem classes, resource limits, residual risks | Written; not validated on a running engine |
| `process-model.md` | Always-running vs optional processes, startup/shutdown | Written |

## decisions/

Architecture Decision Records. One file per decision, immutable once accepted
(supersede with a new ADR instead of editing history).

| File | Decision | Status |
|---|---|---|
| `0001-windows-container-runtime.md` | Docker is mandatory in v1; which Windows runtime distribution hosts it | **Mandatory boundary Accepted**; distribution **Proposed** (dedicated WSL2 distro) — needs P0-10 |
| `0002-playback-engine.md` | Which playback engine ships, and whether it runs on the host or in a container | **Proposed** — host placement recommended; needs P0-11, P0-12, O-09 |
| `0003-modular-monolith.md` | Modular monolith over microservices | Accepted (from `Development.md` §3.1); deployment boundary reconciled by ADR-0008 |
| `0004-local-playback-first.md` | MVP is a local HDMI appliance | **Not started** — P0-01 |
| `0005-host-container-boundary.md` | Host↔container transport, authentication and permissions | **Proposed** — needs P0-13, P1-08 |
| `0006-filesystem-and-volume-policy.md` | Filesystem classes, named volumes, read-only media rule | **Proposed** — needs P1-07, P6-03 |
| `0007-secrets-management.md` | Secret generation, storage and delivery | **Proposed** — needs P1-03, P1-08 |
| `0008-modular-monolith-deployment-boundary.md` | One codebase deployed across the Windows host and the Docker backend | **Proposed** — human review required |

## design/

| File | Contents |
|---|---|
| `design-system.md` | Token set, type scale, surfaces, focus, motion, contrast rules |
| `navigation-spec.md` | Focus model, shelf traversal, overlay stack, restoration |

## security/

| File | Contents |
|---|---|
| `threat-model.md` | Assets, actors, entry points, abuse cases, mitigations |
| `compose-security-checklist.md` | Per-release checklist for container configuration |

## testing/

| File | Contents |
|---|---|
| `test-matrix.md` | Layers, hardware matrix, media matrix, definition of done |

## setup/

| File | Contents |
|---|---|
| `first-run.md` | The first-run experience, what preflight checks and why |

## operations/

| File | Contents |
|---|---|
| `backup-recovery-uninstall.md` | Backup, recovery, safe uninstall, data locations |

## superpowers/

Planning artifacts produced with the brainstorming and writing-plans skills.

| File | Contents |
|---|---|
| `specs/2026-11-15-media-haven-design.md` | Design spec resolving Phase 0 framing |
| `plans/00-phase-0-discovery.md` | Bite-sized task plan for Phase 0 |
| `plans/01-phase-1-foundation.md` | Bite-sized task plan for Phase 1 |
| `plans/02-phase-2-library.md` | Bite-sized task plan for Phase 2 |

## Conventions

- Documents describe **actual** behaviour. If the code and the document
  disagree, that is a defect — fix one of them and record which.
- No secrets, tokens, personal paths or media filenames in any document.
- Every open decision in `Development.md` §18 must appear as an ADR marked
  Proposed, or as `[?]` in `roadmap.md`. It must never be silently resolved.
