# Design: Opt-in catalog and operator source discovery

- **Status:** **Approved** — the human partner explicitly overrode the project
  documents on 2026-11-24 ("implement the features ... even if it's against the
  .md file") and approved "build both" (catalog + magnet flow). ADR-0024 is
  Accepted 2026-11-24 with attribution; ADR-0023's dependency is accepted by the
  same instruction because the catalog cannot work without it.
- **Date:** 2026-11-24
- **Related:** ADR-0024 (Accepted), ADR-0023 (TMDB artwork transport, dependency
  accepted here), ADR-0005 (named commands), ADR-0007/0014 (secrets), ADR-0018
  (qB sandbox), ADR-0021 (manual source fallback), `AGENTS.md` (as amended
  2026-11-24).

## Context

MEDIA_HAVEN already has: a single-window Tauri UI with local library search and
an offline recommendation shelf; a local watchlist; an operator-supplied Source
field and `source_set` chain into a qBittorrent sandbox; and fail-closed probe +
Defender gates before any download becomes playable.

The human wants the miauwatch-style experience in this app: browse a catalogue,
pick a title, search for a source, pull its magnet link, download and play — all
inside the one window. Earlier this was refused under the project's "no content
source bundling" rule. The human has now explicitly overridden that rule.

## Decision

Two independent sub-projects, both opt-in and credential-free at the renderer:

**D — Operator source discovery.** A host-only named command
`source_search(title, year)` queries one operator-configured Torznab endpoint
(Jackett/Prowlarr style) using the existing secret store. It returns a bounded,
deterministically ranked shortlist of candidates (title, size, seeders,
resolution, source name — no file paths, no magnet in a renderer DTO until
chosen).
The user picks one on the existing Details screen; picking calls the **existing**
`source_set` with the chosen magnet/`.torrent` name. Everything downstream —
`item_sources`, qB sandbox, rolling buffer, probe + Defender gates, cleanup —
stays exactly as it is. A search result never marks a file ready by itself.
Manual source entry remains the fallback.

**E — Opt-in TMDB catalog.** Enable the existing `TmdbProvider` transport seam
and add catalogue browse/search so a title outside the local library can be
discovered and then handed to D. Off by default: requires both the opt-in flag
and an operator API key from the secret store. Only a title, year and poster path
leave the machine; no library contents, paths or tokens. Attribution is rendered
where TMDB data appears. Cached art works offline afterwards.

## Boundary that still holds

Even under the override, the following are not shipped:

- No tracker lists, indexer definitions, content-site catalogues or instructions
  for unlawful acquisition.
- The endpoint and API key come from the operator; absent credentials disable
  both features completely.
- No silent auto-download. Every source is confirmed by a human.
- No renderer-facing path, token, credential or host detail. No backend route or
  container egress change (`core` stays `MEDIAHAVEN_EGRESS=none`); both HTTP
  clients run on the Windows host.

## Data flow

```text
E: TMDB browse/search (host, opt-in) -> display-only catalogue cards
   -> user picks title -> Details
D: source_search(title, year) (host, operator Torznab) -> ranked shortlist
   -> user confirms -> source_set(existing) -> qB -> probe/Defender -> playback
```

## Error handling

- Missing endpoint/key/opt-in: features disabled, no request, no file read; the
  UI shows a clear setup state, not an error or empty result.
- Transport failure: degrade to an explicit "search unavailable" state; never
  fabricate candidates or mark media ready.
- Malformed/hostile indexer or TMDB data: sanitise before display; cap response
  size and shortlist length; fail closed on path-like or control-character data.
- All network I/O runs off the UI thread through the host; timeouts are bounded.

## Testing

- Pure unit tests for ranking, response parsing, sanitisation, key construction
  and opt-in gating using fake responses — **no live indexer or TMDB call in any
  test**.
- Frontend tests for the shortlist/confirm path and the catalog browse/search
  states, using mocked commands.
- Existing 24-command parity test stays green (both commands are host-only, like
  `player_control` and `player_attach_surface`).
- Live calls are NOT RUN until the operator supplies an endpoint/key; recorded
  honestly as NOT RUN, never PASS.

## Risks / revisit triggers

- A dependency (HTTP client with rustls) is newly accepted by the human override;
  record the exact crate and version in the plan and SBOM.
- Ranking must be deterministic and auditable; reopens if it cannot be.
- Reopen if the operator cannot supply a lawful endpoint, or if the human
  withdraws the 2026-11-24 override.
