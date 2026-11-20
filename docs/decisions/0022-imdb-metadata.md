# ADR-0022: IMDb ratings, age ratings and listings (additive, opt-in)

- **Status:** **Proposed** — written, not approved. No live IMDb/OMDb request
  has been made; code must not depend on this until the human accepts it.
- **Date:** 2026-11-21
- **Deciders:** human partner (request 2026-11-21), contributing agent
- **Related:** ADR-0011 (TMDB primary, local NFO fallback, opt-in network),
  ADR-0018 (no bundled sources), ADR-0007 (secrets), AGENTS.md (no new
  dependency without ADR),
  `docs/superpowers/plans/2026-11-21-imdb-metadata.md`

## Context

On 2026-11-21 the human partner requested IMDb-sourced ratings, age ratings and
listings (title/year/genres) on movies and shows. This is an **addition** to the
metadata picture, not a replacement: ADR-0011 already makes TMDB the primary
network provider with local NFO as the offline fallback, and that decision is
not reopened here.

The official IMDb datasets (`title.basics`, `title.ratings`) supply ratings,
vote counts and listings but **no certification / age rating**. Age ratings
therefore need a second, separately opt-in seam with the shape of OMDb, using
an operator-supplied key. The workspace has no HTTP/TLS crate and no ADR
permits adding one, so transport is an injected seam and v1 accepts plain
`.tsv` files only — no gzip dependency.

## Decision

Add an `imdb` provider in `crates/metadata` that reads operator-supplied IMDb
`title.basics.tsv` and `title.ratings.tsv` files, merges them by `tconst`, and
sanitises every textual field. Expose a separate OMDb-shaped fetch seam that
fills `age_rating`; a missing key yields `Unavailable`, never a fabricated
value. All of it is **off by default**: until the operator opts in, no file is
read and no request leaves the machine. Results are attributed in the UI with
`IMDB_ATTRIBUTION` ("Ratings and age ratings: IMDb / OMDb") and are display-only
fields on the media DTOs, so TMDB/NFO data remains the canonical source and any
failure degrades to local data without breaking a view.

Constraints that travel with this decision:

- **No bundled content source, indexer or "find a rating" scraper.** The
  operator supplies the dataset files or the OMDb key; use must be lawful.
- **Opt-in text names exactly what is sent and to whom** (title/year or IMDb
  id; IMDb datasets / OMDb). Backend validates; the renderer receives no path
  or token.
- **Attribution** is rendered wherever IMDb data appears.
- **Cache** has a TTL, a byte cap and oldest-first eviction (Task 1).
- **No new dependency** — no gzip crate in v1; the HTTPS transport needs its
  own ADR before it is written.

## Alternatives considered

| Option | Pros | Cons | Verdict |
|---|---|---|---|
| Operator-supplied IMDb datasets (TSV) | Authoritative ratings/listings, fully offline, no key | Large files, no age rating, no gzip in v1 | **Accepted** for ratings/listings |
| OMDb API (operator key) | Certification + richer detail | External dependency, rate limits, key management, terms | **Accepted** as a separate opt-in seam for age ratings only |
| Bundle an IMDb dump / scraper | No operator setup | Bundles a content source, licence/tracker risk, violates ADR-0018 | Rejected |
| Replace TMDB with IMDb | Single network provider | Reopens ADR-0011, drops synopsis/artwork work already done | Rejected — additive only |
| Add a gzip/HTTP crate now | Smaller files, real transport | New dependency without ADR | Rejected — plain TSV + injected seam in v1 |

## Consequences

Positive: users who opt in get ratings, vote counts, age ratings and listings
alongside the existing TMDB/NFO metadata; all parsing and merging is testable
with no network and no new dependency.

Negative, and accepted as the cost: dataset files are large and must be
obtained by the operator; age ratings depend on a third-party key with its own
terms and rate limits; every display surface must render attribution and an
empty state instead of inventing a value; the opt-in, cache and attribution
add surface to maintain and test.

## Evidence

- Human request dated 2026-11-21 (recorded in the plan's Spec line).
- `crates/metadata/src/{model,nfo,tmdb}.rs`: existing sanitiser, opt-in gate,
  fail-closed fetch seam and cache patterns reused rather than reinvented.
- ADR-0011 §Decision: network opt-in, attribution, secret store, failure
  degrades to local data — carried over unchanged.
- ADR-0018: no bundled content source.
- Task 0 verification: offline parser/merge/opt-in tests in
  `crates/metadata/tests/imdb.rs`; no live IMDb/OMDb call made.

## Revisit trigger

The human requests IMDb replace TMDB, a transport/gzip dependency is needed
(write a new ADR first), IMDb or OMDb change terms/licence in a way this product
cannot meet, or the additional opt-in surface proves too costly to maintain.
In those cases write a superseding ADR rather than editing this one.
