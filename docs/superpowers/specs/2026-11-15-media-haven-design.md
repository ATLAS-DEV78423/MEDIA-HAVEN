# MEDIA_HAVEN — design specification

- **Date:** 2026-11-15
- **Status:** Approved-basis (derived). Produced with the brainstorming skill.
- **Source of truth:** `Development.md`. This document does not add requirements;
  it consolidates the design and records what is decided versus open.
- **Supersedes:** nothing. Replaces the need for a separate greenfield design
  discussion, because the contract already answers the questions below.

## Purpose

Turn a spare Windows laptop into a TV-first media appliance: one application,
local files, no accounts, no AI runtime, no data loss.

## Classification and process note

This is an **architectural** task: a new product with no existing code. The
brainstorming skill's architectural path normally begins with clarifying
questions. Those questions are already answered by a 2,302-line approved
contract, so re-asking them would waste the reader's time and risk inventing
requirements. This spec instead:

1. Records the design the contract already fixes.
2. Isolates the genuinely open decisions and blocks on them explicitly.

The approval gate is preserved in the only place it still matters: the open
decisions must be accepted by the human partner before dependent work starts
(see `roadmap.md` CP-0 and the `O-*` register). No implementation code was
written during this exercise.

## Goal

A non-technical user installs MEDIA_HAVEN on a Windows 11 x64 laptop connected
to a TV by HDMI, points it at folders they own, and browses and plays their
library with a remote — fully offline, free, without losing a single file.

## Non-goals (v1)

Remote/headless streaming · mobile app · multi-user profiles · cloud sync ·
transcoding farm · Kubernetes · AI recommendations or agents · paid metadata
providers · arbitrary network shares · automatic deletion of source media ·
bundled trackers, indexers or catalogues.

## Architecture

**One product surface, four internal layers.**

```
React + TypeScript UI  →  Tauri IPC  →  Rust core (authority)  →  SQLite + adapters
```

- **UI** renders and collects intent. No policy, no SQL, no deletion.
- **Command boundary** validates every argument. The frontend is untrusted.
- **Rust backend** owns domain invariants, use cases and ports; it is deployed in the required Docker `core` container.
- **Adapters** implement ports: SQLite, filesystem, playback integration, and the required Windows runtime supervisor boundary.

The user-facing product remains one modular application, but deployment spans the Windows Tauri host and mandatory Docker/Compose backend boundary. `probe` is a proposed isolated media-inspection worker. Neither the Docker runtime distribution nor all component placements are verified; see ADR-0001/0002 and `docs/architecture/overview.md`.

### Why this shape

Each alternative fails a stated constraint:

- Splitting product responsibilities into independently managed network services adds memory, IPC and ops cost on a single-user laptop (`ADR-0003`/ADR-0008).
- Docker is mandatory for the backend/security boundary; this does not require containerising the Windows UI or playback (`Development.md` §2.3–§2.5).
- A media-server-plus-player stack duplicates the playback architecture and
  creates a second user-facing surface, which the product forbids (§2.4.1, §20.1).

## Components

| Component | Responsibility | Boundary |
|---|---|---|
| Shell | Window, lifecycle, updater | Tauri commands only |
| Library | Import, scan, index | Reads user roots read-only |
| Metadata | Cache, optional providers | Never uploads without opt-in |
| Playback | Engine adapter, resume, completion | Never deletes |
| Queue | Durable ordering and transitions | SQLite-backed |
| Storage policy | Limits, exclusions, cleanup preview | Only writer of managed media |
| Runtime supervisor | Required Docker/Compose workspace lifecycle | Never imported by `domain`; unavailable to the frontend |
| Diagnostics | Health, redacted bundle | Local only, no upload |

## Data flow for the dangerous path

The only path that can destroy data: **watched → cleanup**.

1. Completion is a **backend** decision from multiple signals: position near the
   end, watched-duration threshold, a normal end-of-file event where available,
   plus a grace period. A seek to the end, an app close, a crash or a queue
   removal never marks complete.
2. Cleanup requires a preview listing exact paths and counts.
3. Exclusions are absolute: source media, currently playing, unfinished,
   unverified, outside the managed root.
4. The user confirms explicitly.
5. An audit event is written; the item is staged to managed trash.
6. Permanent deletion only after a retention period, and it is **off by default**.

No code path skips steps 2–5. This is enforced by tests that assert the absence
of effects, not merely the absence of errors.

## Data model

SQLite, migrated, foreign keys on, UTC timestamps, forward-safe migrations with a
pre-migration backup.

Entities: `schema_migrations`, `media_items`, `media_files`, `collections`,
`playback_sessions`, `playback_state`, `queue_items`, `downloads`,
`storage_records`, `settings`, `metadata_cache`, `audit_events`.

Rejected: storing secrets in `settings`; storing the database in the install
directory or a container layer; storing paths as opaque strings without
canonicalisation.

## UI design

Dark, cinematic, artwork-led. Design tokens only. TV mode is the default, not a
skinternal option. Details in `docs/design/design-system.md`; focus model in
`docs/design/navigation-spec.md`.

The single most regression-prone area is **focus navigation**. Every phase
re-runs the keyboard-only walkthrough at its checkpoint.

## Security posture

Full threat model: `docs/security/threat-model.md`. The eight guarantees in
`SECURITY.md` are the ones a security report is measured against.

Summary: no source-media deletion; bounded filesystem access; loopback-only;
no silent host mutation; no telemetry; redacted diagnostics; no bundled content
sources; least privilege.

## Testing

Layers and matrices: `docs/testing/test-matrix.md`. Two rules dominate:

1. **`NOT RUN` is not `PASS`.** Every report states which.
2. Security tests assert **absence of effect** outside the boundary — that no
   read, write or delete occurred — not just that an error was returned.

## Open decisions (must be resolved before dependent work)

| ID | Decision | Blocks | Evidence required |
|---|---|---|---|
| O-playback | Playback engine | Phase 3, ADR-0002 | P0-03 codec/hardware matrix |
| O-runtime | Supported Windows distribution for required Docker boundary | P0/P6 lifecycle, ADR-0001 | P0-10/P0-13/P0-14 evidence and human review |
| O-boundary | Host/backend deployment model and trusted operations | ADR-0008, P6-02 | Human architecture review and contract/security verification |
| O-transport | Host/container reachability and authentication | ADR-0005, P0/P6 | Approved transport measurement and threat-model review |
| O-name | Product name, package ID | Installer identity | Human decision |
| O-metadata | Metadata provider + terms | P2-06 | Provider ADR |
| O-downloads | Download provider legal review | P6-02 | Human legal sign-off |
| O-win10 | Windows 10 support claim | Support matrix | Servicing review + P7-04 |
| O-offline | "Works offline indefinitely" guarantee | Metadata behaviour | Decision after Phase 2 |

An agent may propose; only the human partner accepts.

## Definition of done for v1

All `Development.md` §17 release gates pass, and a non-technical user completes
install → import → browse → play → uninstall without assistance, without losing
a file, and without an internet connection.

## Self-review

- **Placeholders:** none. Every section either states a decision or names an open
  decision with its blocking evidence.
- **Consistency:** architecture, components, data flow and testing agree with
  `Development.md` §2–§20 and with `roadmap.md` task IDs.
- **Scope:** one plan family (Phase 0 → Phase 7). Phases are separable into their
  own plans; Phase 3+ plans are written when their Phase 0 dependency resolves.
- **Ambiguity:** the only intentional non-answers are the `O-*` rows, which are
  explicitly labelled as open rather than resolved by assumption.
