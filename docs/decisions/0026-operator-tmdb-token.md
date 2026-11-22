# ADR-0026: Optional operator-supplied TMDB token

- **Status:** Accepted (2026-11-22) — human decision recorded below.
- **Date:** 2026-11-22
- **Deciders:** human partner (2026-11-22 decision), contributing agent
- **Related:** ADR-0007 (secrets), ADR-0014 (host token DPAPI), ADR-0018
  (no bundled content source), ADR-0023 (TMDB artwork transport), ADR-0025
  (keyless IMDb posters), `docs/superpowers/plans/2026-11-21-operator-tmdb-token.md`

## Context

ADR-0025 chose keyless IMDb posters because no TMDB key was available. The
human has since decided (2026-11-22) to supply their own TMDB read access token:
"we can just ask the user to add their tmdb access token". The same decision
mentioned Jackett/Prowlarr, but that is a separate, later effort and is not
part of this decision. Artwork should be open by default, not gated behind the
`MEDIAHAVEN_TMDB_OPTIN` switch ADR-0023 introduced.

## Decision

The operator may optionally paste a TMDB read access token in Settings. The
host stores it in the existing Windows DPAPI blob mechanism (ADR-0007/0014)
under an identity distinct from the install token, so the two can never be
confused. On container startup the supervisor injects the token into the
`docker compose up` environment only when one is configured; otherwise no TMDB
environment entry is added and the app behaves exactly as before. `core` uses
TMDB as the primary artwork source when a key is present, with no opt-in flag,
and falls back to the keyless IMDb posters of ADR-0025, then bundled art.

The token is operator-supplied, optional and reversible. A blank or missing
token means zero TMDB requests, working keyless IMDb artwork and a fully
offline app. The token is stored ONLY in the DPAPI blob: never in the repo, a
renderer DTO, a log, an error string, localStorage, or the renderer after save.
Settings shows only configured/not-configured state.

No scraper, indexer, catalogue, site list or Cardigann definition is added.
Jackett and Prowlarr are explicitly out of scope for this ADR. No new
dependency or runtime is introduced.

## Alternatives considered

| Option | Pros | Cons | Verdict |
|---|---|---|---|
| Optional operator token in DPAPI, TMDB primary + IMDb fallback | Real posters, no bundled source, offline without a key | Operator must supply and maintain a token | **Accepted** |
| Keep keyless IMDb only (ADR-0025) | No token needed | No TMDB posters; human changed decision | Superseded for posters |
| Bundle Jackett/Prowlarr or a site/indexer list | No TMDB key needed | Violates ADR-0018; large new attack surface | Out of scope, rejected here |
| Store token in localStorage / DTO / config file | Easy | Leaks the secret to renderer/repo; violates ADR-0007/0014 | Rejected |

## Consequences

Positive: posters improve when the operator supplies a token; the app remains
fully functional offline and without one. Negative: token lifecycle (expiry,
revocation) is the operator's responsibility. Clearing the token restores the
keyless behaviour immediately on the next restart.

Preserved: loopback-only serving, 2 MiB/64 MiB hashed cache, 12-item batch cap,
64-hex route validation, relative DTO URL, `onError` fallback, and exact TMDB
and IMDb attribution lines.

## Evidence

- Human decision (2026-11-22): "we can just ask the user to add their tmdb
  access token, and we can use the jackett prowlarr thing".
- `apps/desktop/src-tauri/src/secret.rs`: existing DPAPI store, fixed identities
  and fail-closed read semantics.
- Task 1 tests cover secret round-trip/isolation, identity non-collision,
  corrupt-blob error, kind allowlist and env injection; no live TMDB call.

## Revisit trigger

The human withdraws the token, TMDB changes its API contract, or a later
Jackett/Prowlarr plan changes the provider model. Reopen with a superseding ADR
rather than quietly changing this one.
