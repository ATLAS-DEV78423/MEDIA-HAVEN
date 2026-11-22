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

For the authoritative ADR index with statuses and evidence owed, see
`decisions/README.md`.

## decisions/

Architecture Decision Records. One file per decision, immutable once accepted
(supersede with a new ADR instead of editing history).

| File | Decision | Status |
|---|---|---|
| `0001-windows-container-runtime.md` | Docker is mandatory in v1; which Windows runtime distribution hosts it | Mandatory boundary **Accepted**; distribution **Accepted (provisional)** — needs P0-10 |
| `0002-playback-engine.md` | Which playback engine ships, and whether it runs on the host or in a container | **Accepted (provisional)** — mpv via JSON IPC, host placement; needs P0-11, P0-12, O-09 |
| `0003-modular-monolith.md` | Modular monolith over microservices | Accepted; deployment boundary reconciled by ADR-0008 |
| `0004-local-playback-first.md` | MVP is a local HDMI appliance | **Accepted** (2026-11-20, ADR-0016) |
| `0005-host-container-boundary.md` | Host↔container transport, authentication and permissions | **Accepted** (2026-11-20, ADR-0016); P0-13 evidence owed |
| `0006-filesystem-and-volume-policy.md` | Filesystem classes, named volumes, read-only media rule | **Accepted (provisional)** — needs P1-07, P6-03 |
| `0007-secrets-management.md` | Secret generation, storage and delivery | **Accepted (provisional)** — token storage amended by ADR-0014 |
| `0008-modular-monolith-deployment-boundary.md` | One codebase deployed across the Windows host and the Docker backend | **Accepted** (2026-11-20, ADR-0016) |
| `0009-frontend-first-contract-driven.md` | Frontend-first, contract-driven development | Accepted |
| `0010-library-entity-deviations.md` | Library entity deviations and scanner conventions | **Accepted** (2026-11-20, ADR-0016) |
| `0011-metadata-provider-selection.md` | Metadata provider selection | **Accepted** (2026-11-18) — TMDB primary, local NFO fallback, network opt-in |
| `0012-profiles.md` | Profiles ("Who's watching") | **Accepted** (2026-11-20, ADR-0016) |
| `0013-backend-completion-sequencing.md` | Containerise the backend before the remaining features | Accepted (2026-11-18) |
| `0014-host-token-storage.md` | Host API token storage uses DPAPI | Accepted (2026-11-18) — amends ADR-0007 |
| `0015-playback-descriptor.md` | Playback source resolved host-side from the media id — no path or token in the renderer | **Accepted** (2026-11-20, ADR-0016) |
| `0016-blanket-acceptance-2026-11-20.md` | Human blanket acceptance of outstanding decisions | Accepted |
| `0017-acquisition-conversion.md` | Adopt torrent acquisition of authorized media; supersede local-playback-first | Accepted (2026-11-20) |
| `0018-qbittorrent-boundary.md` | qBittorrent sandbox and trust boundary | Accepted (2026-11-20) |
| `0019-app-managed-cleanup.md` | Cleanup of watched, app-managed media only | Accepted (2026-11-20) |
| `0021-r01-source-mapping.md` | R-01 per-item operator-supplied source mapping | **Accepted** (2026-11-21, reversible) |
| `0022-imdb-metadata.md` | IMDb ratings, age ratings and listings (additive, opt-in) | **Proposed** (2026-11-21) |
| `0023-tmdb-artwork-transport.md` | TMDB artwork transport and loopback image serving | **Proposed** (2026-11-21) — from an unexecuted plan; human acceptance required |
| `0024-catalog-discovery-and-source-selection.md` | Host-side Torznab search + operator confirmation (minimal change) | **Proposed** (2026-11-21) — would supersede ADR-0021's no-discovery clause; legal + dependency gate |

There is no ADR-0020 — the number was skipped, not lost.

## design/

| File | Contents |
|---|---|
| `design-system.md` | Token set, type scale, surfaces, focus, motion, contrast rules |
| `navigation-spec.md` | Focus model, shelf traversal, overlay stack, restoration |

Design implementation lives in `apps/desktop/src/styles/tokens.css` — the only
place a literal colour or spacing value may appear (enforced by `check:tokens`).

## security/

| File | Contents |
|---|---|
| `threat-model.md` | Assets, actors, entry points, abuse cases, mitigations |
| `compose-security-checklist.md` | Per-release checklist for container configuration |
| `boundary-evidence.md` | Observed host/container boundary evidence |
| `socket-inventory.md` | Socket and port inventory |

## testing/

| File | Contents |
|---|---|
| `test-matrix.md` | Layers, hardware matrix, media matrix, definition of done |
| `frontend-evidence.md` | Frontend gate results |
| `phase0-measurements.md` | Phase 0 measurement records |
| `acquisition-matrix.md` | Acquisition criteria, with live-wiring results marked NOT RUN |

## setup/

| File | Contents |
|---|---|
| `first-run.md` | The first-run experience, what preflight checks and why |

## operations/

| File | Contents |
|---|---|
| `backup-recovery-uninstall.md` | Backup, recovery, safe uninstall, data locations |
| `release.md` | Installer build, checksums, SmartScreen caveat, SBOM status |
| `privacy.md` | Opt-in metadata behaviour (IMDb, acquisition), what leaves the machine |

## superpowers/

Planning artifacts produced with the brainstorming and writing-plans skills. A
plan describes intent; check `roadmap.md` and `progress.md` for what was actually
executed. Every step in an unexecuted plan stays a proposal.

| File | Contents |
|---|---|
| `specs/2026-11-15-media-haven-design.md` | Design spec resolving Phase 0 framing |
| `plans/00-phase-0-discovery.md` | Bite-sized task plan for Phase 0 |
| `plans/01-phase-1-foundation.md` | Bite-sized task plan for Phase 1 |
| `plans/02-phase-2-library.md` | Bite-sized task plan for Phase 2 |
| `plans/2026-11-16-backend-foundation.md` | Rust backend foundation plan |
| `plans/2026-11-17-phase-2-backend-completion.md` | Backend completion plan |
| `plans/2026-11-18-backend-completion.md` | Backend completion continuation |
| `plans/2026-11-19-documentation-reconciliation.md` | Documentation reconciliation plan |
| `plans/2026-11-19-scan-recovery.md` | Scan/recovery plan |
| `plans/2026-11-19-remaining-development.md` | Remaining development plan |
| `plans/2026-11-20-playback-host-wiring.md` | Playback/host wiring plan (executed) |
| `plans/2026-11-20-torrent-acquisition-conversion.md` | Authorized-media torrent acquisition plan (executed) |
| `plans/2026-11-20-downloaded-media-security.md` | Downloaded-media fail-closed security gate plan (executed) |
| `plans/2026-11-21-release-blockers.md` | R-01/Defender/installer plan (executed) |
| `plans/2026-11-21-imdb-metadata.md` | Offline IMDb metadata plan (executed) |
| `plans/2026-11-21-keyboard-navigation.md` | Arrow/Enter/Escape navigation plan (executed) |
| `plans/2026-11-21-tmdb-artwork.md` | **Unexecuted** — opt-in TMDB artwork transport (ADR-0023 Proposed) |

## Conventions

- Documents describe **actual** behaviour. If the code and the document
  disagree, that is a defect — fix one of them and record which.
- No secrets, tokens, personal paths or media filenames in any document.
- Every open decision in `Development.md` §18 must appear as an ADR marked
  Proposed, or as `[?]` in `roadmap.md`. It must never be silently resolved.
