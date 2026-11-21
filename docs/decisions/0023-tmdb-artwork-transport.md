# ADR-0023: TMDB artwork transport and loopback image serving

- **Status:** **Proposed** — written from an unexecuted plan, not approved. No
  code has been implemented, no live TMDB call made, and code must not depend on
  this ADR until the human accepts it.
- **Date:** 2026-11-21
- **Deciders:** human partner (plan committed 2026-11-21), contributing agent
  (transcribed from the plan)
- **Related:** ADR-0011 (TMDB primary, opt-in, attribution, size-capped cache),
  ADR-0007 (secrets injected from the credential store, never in repo/log),
  ADR-0005 (renderer commands are named and path-free), `Development.md` §4.3
  and §7.4.12, `AGENTS.md` (no new dependency without an ADR),
  `docs/superpowers/plans/2026-11-21-tmdb-artwork.md`
- **Roadmap task:** P6-07 — planned, not started

## Context

ADR-0011 accepted TMDB for posters but deliberately left the HTTPS transport
unimplemented: no ADR permitted an HTTP client crate. The renderer also needs a
way to display cached images: its CSP currently lists only `asset:` and `data:`
in `img-src`, while `connect-src` already allows `http://127.0.0.1:8080`.

This ADR is transcribed verbatim in substance from Step 1 of the TMDB artwork
plan. The plan as a whole is **unexecuted** — every step is unchecked — so this
document records a proposed direction only. It does not authorise adding the
dependency, widening the CSP or making any network request.

## Decision

- Add `reqwest` 0.13.5 (already in `Cargo.lock`; rustls TLS + blocking features,
  never native-tls) as a workspace dependency of `crates/metadata` only. Blocking
  calls are wrapped in `tokio::task::spawn_blocking` by `core` so no async worker
  blocks.
- Serve cached artwork from a public `GET /artwork/:key` route on the existing
  loopback listener. `:key` is the SHA-256 hex of the sanitised TMDB poster
  path, so it is opaque and cannot traverse a path. Image bytes are public TMDB
  content; no token or host detail is exposed.
- Widen the Tauri CSP `img-src` by exactly one origin, `http://127.0.0.1:8080`,
  matching the origin already allowed in `connect-src`.
- Opt-in stays off unless `MEDIAHAVEN_TMDB_OPTIN=1` and a non-blank key from the
  ADR-0007 store is present. Only title, year and poster path leave the machine.
- Per image: 2 MiB cap, JPEG/PNG magic-byte check, SHA-256 key, 64 MiB total
  cache with oldest-first eviction. TMDB attribution is rendered wherever its
  art appears. The exact attribution string is:

  > This product uses the TMDB API but is not endorsed or certified by TMDB.

## Alternatives considered

| Option | Pros | Cons | Verdict |
|---|---|---|---|
| Keep bundled placeholder art only (status quo) | Zero network surface; fully offline | No real posters | Retained as the offline fallback; not replaced |
| `reqwest` with native-tls/OpenSSL | Familiar on Windows | Core ships in a Linux container; extra native build surface | Rejected — use rustls |
| Pass the poster CDN path directly in renderer DTOs | Simpler | Exposes a path and defeats the opaque-key boundary (ADR-0005) | Rejected — hashed key only |

## Consequences

Positive: real posters with zero runtime scraper, offline after first cache.

Negative: an HTTP client and a loopback image origin enter the trust surface;
both are bounded by opt-in, loopback bind and opaque keys. Live calls remain
**NOT RUN** until an operator supplies a key and opts in.

Additional constraint from the plan: no scraper, indexer, catalogue or
downloader is bundled; artwork is metadata only.

## Evidence

None yet — this is a proposal from an unexecuted plan. Verification planned in
the plan uses fake fetches only (no live TMDB call in any test): opaque hash key,
image type-check and byte cap, oldest-first cache eviction, loopback route
rejecting non-hash keys, `artFromDto` URL validation and offline fallback.
The final gate (fmt, clippy, Rust tests, frontend checks) is also planned, not
observed. Do not claim any of these as PASS.

## Revisit trigger

Reopen this ADR if human acceptance is withheld; if the opt-in gate, 2 MiB/64 MiB
caps, opaque key validation or attribution cannot be implemented as stated; if a
live TMDB request would be made without an operator-supplied key; or if an
existing dependency already provides the needed transport without `reqwest`.
