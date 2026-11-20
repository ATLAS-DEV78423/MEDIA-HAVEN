# ADR-0012: Profiles ("Who's watching")

- **Status:** Accepted (2026-11-20) — via ADR-0016 blanket human acceptance; local-only profile picker already implemented
- **Date:** 2026-11-18
- **Deciders:** human partner, contributing agent
- **Related:** Development.md §7 (UI), Stitch `profile_selection_who_s_watching`

## Context

MEDIA_HAVEN has no concept of multiple users. The "Who's watching" screen is a new product feature: a local-only profile picker shown before the main UI. Profiles store a display name, avatar colour, and (later) per-profile watch history/queue. No network, no telemetry, no authentication — profiles are a convenience layer on a single machine.

## Decision

Profiles are a first-class domain type `Profile` (id, name, avatar_color, created_at) persisted as a JSON array inside the existing settings blob (no new migration). They are read/written through the existing `settings_get`/`settings_set` commands; a default "You" profile is seeded when the list is empty. The frontend ProfileSelectionPage calls these commands. No new HTTP routes or trait methods are added in v1.

## Alternatives considered

| Option | Pros | Cons | Verdict |
|---|---|---|---|
| Dedicated profiles table + routes | Clean, queryable | New migration, new routes, more code | Rejected for v1 |
| Profiles in settings blob (chosen) | Reuses existing contract, no migration | Less queryable, single-writer | Accepted for v1 |
| Frontend-only (localStorage) | Fastest | Violates "Rust owns policy" | Rejected |

## Consequences

Positive: minimal code, no migration, reuses frozen command contract. Negative: profiles are not independently queryable; concurrent multi-client writes could clobber. Revisit when multi-device sync or per-profile data isolation is required.

## Evidence

Existing `settings_get`/`settings_set` round-trip verified by build; default seed added in `default_settings`.

## Revisit trigger

Per-profile watch history, queue isolation, or multi-device sync.