# Architecture overview

Derived from `Development.md` §2–§3. If this document drifts from
`Development.md`, the latter wins and this file is the defect.

## Shape

One product. One window. One navigation model. One library.

```
┌────────────────────────────── Windows host ─────────────────────────────┐
│ Tauri 2 shell: React UI + named commands + runtime supervisor + playback │
│  UI has no Docker access; supervisor owns approved runtime lifecycle     │
│  Playback is host-side by decision (ADR-0002) — the GPU, audio endpoint   │
│  and vsync exist only here                                              │
└───────────────────────────────────┬─────────────────────────────────────┘
                                    │ one host/container API boundary
                                    │ (loopback, bearer + HMAC, ADR-0005)
                                    ▼
┌────────────────── Required Docker/Compose workspace ────────────────────┐
│ `core`: Rust backend, product policy, SQLite and orchestration           │
│ `probe`: isolated, networkless media inspection                         │
└───────────────────────────────────┬─────────────────────────────────────┘
                                    │ narrowly selected, read-only mounts
                                    ▼
                   user-selected original media folders
```

Docker/Compose is a **mandatory** v1 boundary (ADR-0001 Part 1). The Windows runtime
distribution is proposed but not yet accepted (ADR-0001 Part 2). See
`docker-workspace.md` and the P0 evidence gates.

## Layering

| Layer | Location | Owns | Must not own |
|---|---|---|---|
| UI | `apps/desktop/src` | Rendering, focus, user intent | Policy, SQL, filesystem deletion |
| Command boundary | `apps/desktop/src-tauri/src/commands` | Input validation, DTO mapping | Domain logic |
| Application | `crates/app-core/application` (deployed in `core`) | Use cases, orchestration | UI details, container-runtime APIs |
| Domain | `crates/app-core/domain` | Entities, invariants, rules | Any I/O |
| Ports | `crates/app-core/ports` | Interfaces for I/O | Implementations |
| Infrastructure | `crates/persistence`, `media-library`, `playback`, `metadata`, `downloads`, `runtime-adapter`, `platform-windows` | Adapters | Product rules |

Dependency direction is inward only. Domain depends on nothing. If a container
runtime or a player SDK appears in `domain`, the architecture has broken.

## One authority rule

The Rust backend in the required `core` container decides. The frontend may display and collect intent; it may not
enforce a security rule, compute completion, or decide what is eligible for
deletion. Anything the frontend can be tricked into displaying, the backend
must already have validated.

## Data flow — opening the library

1. UI asks for shelves via a typed command.
2. Command validates arguments, delegates to an application use case.
3. Use case queries the repository port; SQLite adapter runs indexed queries.
4. Application assembles DTOs; artwork references point at the local cache.
5. UI renders, lazily loading artwork.

The UI never receives a raw filesystem path it can act on, and never composes a
path to read. If a path must cross the boundary for display, it is labelled as
display-only.

## Data flow — destructive operation

1. UI requests a **preview**.
2. Application enumerates candidates through the storage policy port, applying
   exclusions (source media, playing, unfinished, unverified, outside root).
3. UI shows exact paths and counts.
4. User confirms explicitly.
5. Application writes an audit event, stages to managed trash, verifies.
6. Permanent deletion, if ever enabled, only after retention and safety checks.

There is no code path from "watched" to "deleted" that skips steps 2–5.

## Storage split

| Data | Proposed location | Backed up | Deletable by app |
|---|---|---|---|
| User source media | User-chosen folders, narrowly mounted read-only | User's responsibility | **Never** |
| Database and backend state | Docker-managed persistent storage (ADR-0006 proposal) | Yes, via verified export/restore | Only explicit app-data uninstall flow |
| Host config, logs, diagnostics, exports | `%LOCALAPPDATA%/<AppName>/` (proposal) | As documented | Only explicit app-data uninstall flow |
| Managed media (if approved) | User-chosen managed root or dedicated volume, not yet verified | Optional | Yes, via policy only |

Exact volume ownership, backup and recovery behavior are proposals; validate them before implementation. The container writable layer is never the only copy of anything.

## Why a modular monolith

`Development.md` §3.1 mandates it. Splitting into services would add process
boundaries, IPC failure modes and deployment surface before there is any
measured need. Modules are separated by crate boundaries so a future extraction
is possible without a rewrite.

## Open architectural decisions and evidence gates

- Windows Docker runtime distribution and lifecycle — `docs/decisions/0001-windows-container-runtime.md`; the **mandatory boundary is accepted**, the distribution (dedicated WSL2 distro) is **Proposed**, pending P0-10/P0-13/P0-14 evidence.
- Host/container transport and authentication — `docs/decisions/0005-host-container-boundary.md` (Proposed; reachability unverified, P0-13).
- Filesystem and named-volume policy — `docs/decisions/0006-filesystem-and-volume-policy.md` (Proposed; implementation/restore unverified, P1-07/P6-03).
- Runtime secret provisioning — `docs/decisions/0007-secrets-management.md` (Proposed, P1-03/P1-08).
- Deployment boundary between host and container — `docs/decisions/0008-modular-monolith-deployment-boundary.md` (Proposed; human review required).
- Windows playback engine — `docs/decisions/0002-playback-engine.md` (Proposed; **placement** decided as host-side, engine and licence still open pending P0-11/P0-12/O-09).

These documents describe proposals and requirements, not observed behavior. Do not treat an untested property as verified.
