# ADR-0021: R-01 per-item operator-supplied source mapping

- **Status:** Accepted (2026-11-21) — reversible by the human partner
- **Date:** 2026-11-21
- **Decider:** human partner (delegation "solve whatever you can", 2026-11-21);
  mechanism chosen by the contributing agent as recommended option A
- **Scribe:** contributing agent
- **Authority:** the human's delegated scope for the 2026-11-21 release-blocker
  plan (Task 0). The human may reverse the mechanism without a new ADR being
  treated as a correction of an irreversible product commitment.
- **Related:** ADR-0018 (no bundled sources), ADR-0019 (app-managed cleanup),
  `docs/superpowers/plans/2026-11-21-release-blockers.md` Task 0,
  `media_haven_consolidated_specification.md` R-01

## Context

A downloaded file becomes playable only when it is tied to a `media_id` and
passes both the probe and Defender gates. The acquisition pipeline stores
downloads with `media_id = NULL` until a per-item source mapping is supplied
(R-01). Without that mapping the rolling prefetch buffer is a deliberate no-op
and the playback/detail gates cannot run.

ADR-0018 forbids bundling trackers, indexers, catalogues or any "find a
torrent" feature. Sources are lawful ones the operator already holds. The
human delegated "solve whatever you can" for the release blockers; the
recommended option A (per-item operator-supplied source) is adopted here.

## Decision

1. Each library item may have **one operator-supplied source**, set from its
   Details screen through the named `source_set` command. The source is a
   magnet URI or a bare `.torrent` file name. No source is discovered,
   suggested, bundled or invented.
2. A local `item_sources` table (`media_id`, `source`, `source_name`) is the
   store. `source_for_media` reads it; `source_for` in `core` now reads the
   store instead of returning `None`. Prefetch failure stays non-fatal.
3. Setting a source also sets `download_items.media_id` for every download row
   whose torrent hash matches that source, so the existing gate/cleanup paths
   can see the association.
4. The trust boundary validates every source (non-empty, bounded length,
   magnet or bare `.torrent` only, no paths) before it is stored. Renderer DTOs
   carry no filesystem path; the UI confirmation is path-free.
5. Fail closed: a download row with a media id but a NULL verification state
   is **not** playable. Library imports (no download row) remain allowed.
6. This ruling is **reversible by the human**. Replacing the mechanism (for
   example, a different mapping surface) requires a superseding ADR, not a
   silent deviation.

## Alternatives considered

| Option | Pros | Cons | Verdict |
|---|---|---|---|
| A — per-item operator-supplied source (chosen) | No discovery, no bundled sources; uses existing gates; small local store | Operator must attach the source per item | Accepted, reversible |
| B — bundled tracker/indexer lookup | Convenient | Violates ADR-0018 and the content-source-neutral product rule | Rejected |
| C — auto-invent a source for every missing episode | No UI work | Fabricates acquisition intent; unsafe and dishonest | Rejected |
| D — leave `source_for` as a `None` stub | Zero code | R-01 gate stays open; no downloaded file can play | Rejected |

## Consequences

- A small SQLite table and two port methods are added; no new dependency, no
  new container and no network access.
- The UI gains a Source text field on Details. It never displays a path or a
  credential; validation is repeated in Rust, which remains the authority.
- Live acquisition against a lawful source remains NOT RUN until the human
  supplies one; the mechanism is proven with fixtures and fakes only.

## Evidence

Task 0 tests: source set/read/unknown/invalid round-trip; source→download-row
`media_id` tie; fail-closed NULL-verification guard; prefetch wiring through
the store. Exact commands and exit codes are in
`.superpowers/sdd/2026-11-21-release-blockers/task-0-report.md`.

## Revisit trigger

A human decision to change how sources are attached, or any requirement to
bundle or discover sources (which reopens ADR-0018).
