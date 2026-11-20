# ADR-0010: Library entity deviations and scanner conventions

- **Status:** Accepted (2026-11-20) — via ADR-0016 blanket human acceptance
- **Date:** 2026-11-17
- **Deciders:** human partner, contributing agent
- **Related:** P2-02, P2-03, Development.md §4.2, §5, docs/superpowers/plans/2026-11-17-phase-2-backend-completion.md

## Context

`Development.md` names `collections` among the library entities, but the live
schema, repository, routes, DTOs and frontend contain no collection at all.
Separately, the scanner emits one row per file, ids derive from paths, and the
schema's `media_items.kind` CHECK allows only `movie`, `series`, `episode`,
so audio has nowhere to live. These gaps were discovered while completing the
Phase 2 backend. The question is whether to invent schema now or record the
backend as-is.

## Decision

Record the current backend behaviour as-is rather than inventing schema. Specifically:

- `collections` is not implemented. It is deferred until a consumer exists.
- Audio extensions are stored as `MediaKind::Movie` because the schema CHECK
  allows only movie/series/episode.
- One row per file; no episode→series synthesis.
- `media_items.id` is the SHA-256 of the absolute path, so moving a library
  root changes ids and resets per-item user state. Users must be told in the
  UI when a root is moved.
- No artwork cache and no provider; blocked on O-04.

## Alternatives considered

| Option | Pros | Cons | Verdict |
|---|---|---|---|
| Add a `collections` table now | Matches Development.md entity list | No consumer, no migration test, no route; invents scope | Rejected |
| Add an `audio` kind | Correct typing | Requires migration + CHECK change + UI; no consumer | Rejected |
| Synthesise series from episodes | Nicer library view | Heuristic grouping, no stable id source | Rejected |
| UUID ids | Survive root moves | Lose idempotent rescan stability; cascade-delete risk | Rejected |

## Consequences

Positive: the library view renders today without a metadata provider; per-item
state is stable across rescans but not across root moves; `collections` remains
a roadmap gap rather than silent scope loss.

Negative: audio is mislabelled as movie; series grouping is manual; moving a
root resets watchlist/queue/progress for that root's items.

## Evidence

- `crates/persistence/migrations/0001_init.sql` and `0002_library_roots_path_and_media_tracking.sql`: `media_items.kind` CHECK allows only movie/series/episode; no collections table.
- `crates/media-library/src/scanner.rs`: `classify_kind` maps video+audio to `MediaKind::Movie`, returns `None` for artwork extensions.
- `crates/core/src/routes.rs` `library_scan`: one `MediaUpsert` per scanned file, no episode→series synthesis.
- `crates/media-library/tests/roots.rs` and `crates/persistence/tests/repository.rs`: 8 + 15 passing tests (incl. ignored 10k) covering the above.

## Revisit trigger

A consumer for `collections` appears, or a metadata provider (ADR-0011) lands
and needs audio typing or series grouping, or a user reports data loss from a
moved root.