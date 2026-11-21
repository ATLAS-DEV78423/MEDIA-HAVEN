# ADR-0023: TMDB artwork transport and loopback image serving

- **Status:** **Accepted (2026-11-21)** — approved by the human partner, who
  explicitly accepted "the changes you asked for": this ADR, the `reqwest`
  dependency and the CSP widening.
- **Date:** 2026-11-21
- **Deciders:** human partner (2026-11-21 approval), contributing agent
- **Related:** ADR-0011 (TMDB primary, opt-in), ADR-0007 (secrets), ADR-0018
  (no bundled content source), AGENTS.md (no new dependency without an ADR),
  `docs/superpowers/plans/2026-11-21-tmdb-artwork.md`

## Context

ADR-0011 accepted TMDB for posters but deliberately left the HTTPS transport
unimplemented: no ADR permitted an HTTP client crate. The renderer also needs a
way to display cached images: its CSP currently lists only `asset:` and `data:`
in `img-src`, while `connect-src` already allows `http://127.0.0.1:8080`.

## Decision

- Add `reqwest` 0.13.5 (already in `Cargo.lock`; rustls TLS + blocking + query
  features, never native-tls) as a workspace dependency of `crates/metadata`
  only. Blocking calls are wrapped in `tokio::task::spawn_blocking` by `core` so
  no async worker blocks. `default-features = false` keeps native-tls/OpenSSL
  out of the build; the exact rustls feature in 0.13.5 is `rustls` (the older
  `rustls-tls` name no longer exists), and `query` is required for
  `RequestBuilder::query`.
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
  art appears.

## Alternatives considered

| Option | Pros | Cons | Verdict |
|---|---|---|---|
| `reqwest` 0.13.5, rustls + blocking, default features off | Locked version, no OpenSSL in the Linux container, blocking seam fits the existing synchronous provider | Adds an HTTP client and TLS transitive crates to the trust surface | **Accepted** |
| `reqwest` with native-tls default features | Fewer feature flags | Pulls OpenSSL/Schannel; contradicts the container boundary | Rejected |
| Bundle a poster scraper/indexer/catalogue | No TMDB key needed | Violates ADR-0018 and the no-bundled-source rule | Rejected |
| Leave the transport seam unconfigured forever | Zero new dependency | Real posters never populate; defeats ADR-0011 | Rejected |

## Consequences

Positive: real posters with zero runtime scraper, offline after first cache.
Negative: an HTTP client and a loopback image origin are now in the trust
surface; both are bounded by opt-in, loopback bind and opaque keys. Live calls
remain NOT RUN until an operator supplies a key and opts in.

## Evidence

- Human approval dated 2026-11-21 (see the Status line).
- `Cargo.lock`: `reqwest` 0.13.5 already locked; its sparse-index metadata names
  the TLS feature `rustls` (with `rustls-no-provider` as the provider-free
  variant), not the pre-0.13 `rustls-tls`; `query` is a separate feature gating
  `RequestBuilder::query`, and `blocking` gates the blocking client.
- `crates/metadata/src/tmdb.rs`: opt-in gate, fail-closed key check, 30-day text
  cache, sanitisation and the deliberate `unconfigured_fetch` seam already
  implemented and tested offline.
- Task 1 tests in `crates/metadata/tests/tmdb.rs` use fake bytes and pure
  helpers only; no live TMDB call is made.

## Revisit trigger

TMDB changes its API/CDN contract, a live call is observed failing with the
chosen TLS provider, or the human withdraws opt-in. Reopen with a superseding
ADR rather than quietly changing this one.
