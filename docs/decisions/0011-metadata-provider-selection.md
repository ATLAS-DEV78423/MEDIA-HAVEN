# ADR-0011: Metadata provider selection

- **Status:** **Accepted** (2026-11-18) — TMDB is the network provider, local NFO
  the offline fallback.
- **Date:** 2026-11-17 (accepted 2026-11-18)
- **Deciders:** human partner (acceptance 2026-11-18), contributing agent
- **Related:** P2-06, O-04, Development.md §4.2–§4.3, docs/superpowers/plans/2026-11-18-backend-completion.md

## Context

P2-06 requires a metadata provider (titles, synopsis, artwork) and O-04 was
unresolved: provider choice plus terms. The backend has no `MetadataProvider`
trait, no `crates/metadata` crate and no artwork cache. This ADR lists the
options considered and now records the accepted choice.

## Acceptance record

- **2026-11-18 — accepted by the human partner.** Asked to choose between
  "accept TMDB (with local NFO fallback)" and "local NFO only", the human
  partner answered "i have no problem with tmdb". That is the O-04 decision.
  It is recorded here by the agent at the human's direction; the agent did not
  choose it.
- Correction for the record: on 2026-11-17 this ADR's status line read
  "Accepted (provisional) — TMDB primary" while the Decision section said no
  provider was chosen. No human acceptance existed at that point, so the line
  was corrected to `Proposed` on 2026-11-18 and the acceptance was made
  explicitly the same day.

## Decision

**TMDB is the network metadata provider, with local NFO as the offline
fallback.** The backend reaches it only through a `MetadataProvider` port in
`crates/metadata`, so the provider can be replaced without changing library or
UI code.

Constraints that come with this acceptance:

- Network use is **opt-in and off by default**. With opt-in off, no request
  leaves the machine. With the network down, the library stays fully browsable
  from scanner output, local NFO data and the cache.
- Only search terms are sent (title and year, plus the API's own request
  metadata) — never a file path, folder name or library contents. The opt-in
  text names exactly what is sent.
- TMDB attribution is rendered wherever its data appears.
- The API key comes from the approved secret store (ADR-0007), never from the
  repository, logs or the renderer.
- Metadata and artwork are cached locally with a size cap and eviction;
  artwork is type-checked and sanitised before display, and placeholders hold
  the layout when nothing is available.

Local NFO reading never depended on this decision and is implemented first
(plan Task D2), so the offline path exists before any network call does.

## Alternatives considered

| Option | Auth/terms | Rate limits | Offline mode | Artwork cache licence | Verdict |
|---|---|---|---|---|---|
| TMDB API | Free API key, attribution required | ~40 req/10s | No | Cache allowed with attribution | **Accepted** |
| OMDb | Free tier + paid; attribution | 1000/day free | No | Restricted | Not chosen |
| TVmaze | No key for basic; CC BY-SA data | Generous | No | CC BY-SA | Not chosen |
| Local NFO / embedded tags only | None | N/A | Yes | User-owned | Accepted as the offline fallback, not as the only source |
| Plugin-style provider trait, no default | N/A | N/A | Depends on impl | Depends on impl | Accepted in shape: the port exists, TMDB is the first implementor |

## Consequences

Positive: the library gains canonical titles, synopsis and artwork when the user
opts in, while remaining useful offline; the port keeps the provider swappable;
the choice is now recorded rather than assumed.

Negative, and accepted as the cost: TMDB is an external dependency with terms,
rate limits and availability outside our control, so the app must degrade to
local data on any failure. Caching artwork costs disk and carries an
attribution obligation. Egress becomes a feature the user must consciously
enable, which adds an opt-in surface to maintain and test. OMDb and TVmaze are
not used, so their terms are not reviewed further.

## Evidence

- `crates/app-core/src/ports/`: no metadata provider trait defined yet.
- `crates/`: no `metadata` crate yet.
- `apps/desktop/src`: no artwork cache consumer yet.
- TMDB API terms and attribution requirements: to be re-read and cited in the
  implementing task (D3) before the first request is written, since terms can
  change.

## Revisit trigger

TMDB changes its terms or rate limits in a way this product cannot meet, the
provider becomes unavailable for a sustained period, a paid requirement is
introduced, or the user requests a different provider. In those cases write a
superseding ADR rather than editing this one.
