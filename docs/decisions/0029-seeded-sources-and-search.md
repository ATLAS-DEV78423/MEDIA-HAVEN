# ADR-0029: Seeded sources and search from Details

- **Status:** Accepted (2026-11-22) — human instruction recorded below.
- **Date:** 2026-11-22
- **Deciders:** human partner (2026-11-22 instruction), contributing agent (scribe)
- **Related:** ADR-0018 (qBittorrent boundary), ADR-0021 (R-01 source mapping),
  ADR-0024 (no executable downloads), ADR-0026 (operator TMDB token),
  ADR-0028 (startup wiring and Prowlarr inputs),
  `docs/superpowers/plans/2026-11-22-seeded-sources-and-search.md`

## Context

The human instructed (2026-11-22): "use the providers from the Miauflix repo
itself, seed them in the codebase, plug and play after Prowlarr URL + TMDB key."
The operator now supplies a Prowlarr address and key in Settings (ADR-0028), but
had to add every indexer by hand, and the only way to attach a source to a title
was to paste a magnet or a bare `.torrent` name on the Details page. The app
bundles no content sources, trackers or catalogues (ADR-0018), so any list we
ship must be upstream provider *definitions* with attribution, never a site
list or an acquisition instruction.

## Decision

Five provider definitions (YTS, Nyaa.si, The Pirate Bay from Prowlarr/Indexers;
eztv-api and animetosho from maury91/miauflix-indexers) are copied verbatim into
`crates/core/resources/indexers/` with an `ATTRIBUTION.md` naming both upstream
repositories, and embedded with `include_str!`. They are definitions only: no
tracker, indexer catalogue or unlawful-acquisition guidance is bundled.

One host-internal route, `POST /host/indexers_seed`, installs the three
Prowlarr-native definitions into the operator's *own* Prowlarr instance with one
click in Settings. It reads `MEDIAHAVEN_PROWLARR_URL` and
`MEDIAHAVEN_PROWLARR_API_KEY` from `core`'s environment only. The two custom
Miauflix definitions are reported as skipped because Prowlarr has no matching
`definitionName`. The address and key never enter a DTO, log, error string or
source file; the renderer sees only counts and fixed reason strings.

A second host-internal route, `POST /host/source_search`, queries each enabled
Prowlarr indexer's Torznab feed (`/api/v1/indexer/{id}/newznab`, 2 s timeout per
request) and parses the XML with simple string scanning — no new dependency. An
item is accepted only when its normalized title starts with the requested title
and, when supplied, its year and season/episode markers match. Candidates are
sorted by seeders descending and capped at ten. Each is `{ id, title, size,
seeders, magnet }`: no path, address, key or raw Prowlarr response crosses the
boundary. Every failure degrades to fewer candidates, never a leaked string.

Both routes are host-internal, not renderer commands: the renderer command
contract stays at **25**. The Tauri host forwards them through two named
commands (`indexers_seed`, `source_search`). On the Details page a "Find
sources" button opens a `SourceResults` panel. Results are candidates only:
choosing one calls the existing `download_add` command, then `source_set`, and
shows fixed status text. There is no auto-download, no auto-selection and no
playback path from search. The executable guard, ffprobe and Defender gates on
`download_add` remain the only way a chosen source reaches playback.

Search reuses the seeder's single `ProwlarrHttp` / `ProwlarrFetchFn` seam — there
is no second HTTP client and no live indexer call in any test.

## Alternatives considered

| Option | Pros | Cons | Verdict |
|---|---|---|---|
| Bundle provider definitions with attribution, seed via Prowlarr API | Plug-and-play with the operator's existing Prowlarr; no bundled site list | Needs an operator Prowlarr instance; two custom definitions skipped | **Accepted** |
| Bundle a tracker/indexer list in the app | No operator Prowlarr needed | Violates ADR-0018 (no bundled content sources); new attack surface | Rejected |
| Search indexers directly from the renderer | Fewer hops | Leaks address/key to the renderer; breaks the boundary | Rejected |
| Add a Torznab/XML parsing crate | Less hand parsing | New dependency for a small, bounded parser | Rejected |
| Auto-download the top result | One click less | Violates "candidates only"; bypasses operator intent | Rejected |
| Add a 26th renderer command for search | Uniform DTO surface | Breaks the frozen 25-command contract; host routes exist for this | Rejected |

## Consequences

Positive: after one setup click the operator's own Prowlarr holds the three
native providers, and any title's Details page can shortlist sources from them
without pasting magnets. No secret, path or bundled site list leaves the trust
boundary. Negative: live search quality depends on the operator's Prowlarr
indexers being healthy and configured; no candidate is guaranteed to be safe or
playable until the existing download gates run. Search results are capped at ten
and each indexer request is bounded at 2 s, so a slow or failing indexer only
shrinks the shortlist.

Preserved: loopback-only binding, fixed error strings, no telemetry, no AI
runtime dependency, no host feature/admin change, and no destructive operation.
No checkpoint is passed by this decision alone.

## Evidence

- Human instruction 2026-11-22 quoted above; recorded by the agent as scribe,
  not self-accepted. Research confirmed the `newznab` Torznab path returns 200 and
  `/api/v1/indexer` returns the enabled id list; `/api/v1/indexers/all` is 404.
- Core: `cargo test -p mediahaven-core` — RED (missing `SourceSearchArgs` /
  `source_search_with`), then GREEN (29 lib tests passed, 0 failed). Fake Torznab
  XML only; no live indexer call.
- Frontend: `SourceResults` test RED (missing module), then GREEN (3 tests); all
  six frontend gates green (19 files / 56 tests).
- Full command output and self-review:
  `.superpowers/sdd/2026-11-22-seeded-sources-and-search/task-3-report.md`.
- **Live Prowlarr seeding and live Torznab search are NOT RUN** — no operator
  instance was available; only fake HTTP fixtures were exercised.

## Revisit trigger

Reopen with a superseding ADR if the bundled definition set changes, a live
Prowlarr rejects the seed body, Torznab response shape changes, the renderer
command contract is unfrozen, or search is ever allowed to auto-download or
auto-play.
