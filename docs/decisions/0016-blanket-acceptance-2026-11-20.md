# ADR-0016: Human blanket acceptance of outstanding decisions (2026-11-20)

- **Status:** Accepted
- **Date:** 2026-11-20
- **Decider:** human partner
- **Scribe:** contributing agent
- **Authority:** the human partner wrote, verbatim, "i accept whatever you need me to accept".

## Context

On 2026-11-20 the human partner was presented with the remaining open product
and architecture decisions (ADRs 0004, 0005, 0008, 0010, 0015; O-03 offline
guarantee; O-08 signing certificate; O-01 residual package ID). Rather than
answering each, the partner issued a blanket acceptance.

This ADR records that blanket acceptance **specifically and reversibly**: each
decision is named below with the value now in force, so the human can revoke any
single item later without re-deriving what was accepted. Per the ADR index rules,
an acceptance must be attributable — this ADR is the attribution. Values that
require a concrete choice (signing, package ID, offline guarantee) were chosen
as the smallest no-cost, no-host-mutation option and are flagged for review.

## Decision

The following are now **Accepted** by the human partner via the blanket
acceptance, effective 2026-11-20:

| ID | Decision now in force | Cost / review note |
|---|---|---|
| ADR-0004 | Local HDMI playback first; remote/headless streaming deferred | Already the implemented scope; no code change |
| ADR-0005 | Host/container boundary: one loopback port, per-install token, named commands only, no Docker access or generic passthrough in the renderer | Boundary code and auth matrix already built and observed 2026-11-19; P0-13 LAN measurement still owed |
| ADR-0008 | Modular monolith across Windows host and Docker backend | Matches the shipped architecture |
| ADR-0010 | Library entity deviations and scanner conventions (collections deferred; audio stored as Movie) | Matches shipped schema |
| ADR-0015 | Playback source resolved host-side from the media id via `host_path_map()`; no path or token in any renderer-facing DTO | Unblocks B2/B3 playback adapter. Revisit if the map cannot resolve a moved file |
| ADR-0009 | Frontend-first, contract-driven development | Already the shipped sequencing; body corrected to match the index |
| ADR-0012 | Local-only profiles ("Who's watching") | Already implemented; per-profile isolation is not in v1 (roadmap defers multi-user sync) |
| O-03 | The app works **fully offline** after install; local playback and browsing never require a network | Stronger offline claim; no code change, but P2-06 egress stays opt-in/off |
| O-08 | Ship **unsigned** for v1; SmartScreen warning documented in release notes. A signed build requires a future human budget decision and certificate | No certificate purchased; reversible at P7-03 |
| O-01 (residual) | Package/bundle ID = `com.mediahaven.desktop` | Convention-based default; change before P7-01 if branding requires |

## Consequences

- The playback lane (B2/B3/B4/B5/B6) is unblocked and may depend on ADR-0015.
- Packaging may proceed as unsigned with the SmartScreen caveat published.
- Any item above can be revoked by the human; record the revocation in a new ADR
  (append-only), do not edit this one.

## Revisit trigger

The human partner revokes any named item, or a signing certificate is purchased
(reopens O-08), or branding requires a different package ID (reopens O-01 residual).
