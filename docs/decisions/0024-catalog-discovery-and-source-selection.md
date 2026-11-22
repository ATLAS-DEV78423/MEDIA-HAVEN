# ADR-0024: Host-side search with operator confirmation (minimal-change variant)

- **Status:** **Proposed** — not approved. No code written, no network call made,
  no host or container setting changed. Code must not depend on this ADR until
  the human accepts it. If accepted, it **supersedes ADR-0021's "no discovery"
  clause** (per-item manual source remains the fallback) and **amends ADR-0018 §5**
  to allow an operator-configured search endpoint.
- **Date:** 2026-11-21
- **Deciders:** human partner (direction requested 2026-11-21), contributing agent
- **Related:** ADR-0005 (host/container boundary, named commands), ADR-0007/0014
  (secrets in Windows Credential Manager), ADR-0018 (qBittorrent sandbox, no
  bundled sources), ADR-0019 (app-managed cleanup), ADR-0021 (R-01 mechanism),
  ADR-0023 (TMDB artwork transport), AGENTS.md non-negotiables, `roadmap.md` O-10

## Context

R-01 is cleared code-side: ADR-0021 chose per-item, operator-supplied source
mapping. A file becomes playable only when tied to a `media_id` and passing the
probe + Defender gates. The remaining gap is live evidence, not code.

The human asked for a way to *find, locate and pick* a source using miauflix as
a reference, with the **fewest possible changes**. miauflix does this with a TMDB
catalog, an operator-configured Torznab endpoint and automatic source selection.
Its code cannot be adopted (PolyForm Noncommercial 1.0 vs the intended permissive
licence); only the pattern can.

An earlier draft of this ADR put search inside `core`, which would have required
a new egress path and container-network change. **That is unnecessary.** The
Windows host already has network access and the Credential Manager secret store
(ADR-0007/0014); `core` deliberately does not (`MEDIAHAVEN_EGRESS: "none"`). The
host already owns the named-command boundary and already forwards the chosen
source through `source_set` → `host/acquisition_source_set` → `item_sources`.
Placing search on the host therefore reuses the entire existing acquisition
pipeline and touches **no container, network, database schema or gate**.

## Decision

1. **Search runs on the Windows host only.** A new named command, e.g.
   `source_search(title, year)`, performs a Torznab query. The renderer never
   talks to an indexer and receives no credential — only a list of candidates.
2. **Operator-supplied endpoint.** One Torznab URL and API key, configured by the
   operator and stored through the existing ADR-0007/0014 secret path. Nothing is
   bundled: no indexer definitions, tracker lists, catalogue or search service.
   Lawful use only. An absent endpoint or key disables the feature completely.
3. **Minimal egress payload.** The query carries only title and year. No path,
   folder name, library content, token or host detail leaves the machine.
4. **Rank, then confirm.** The host ranks results deterministically (seeders,
   size, resolution, audio/subtitle match) and returns a bounded top-N list. The
   user picks a candidate in the existing Details screen; picking it calls the
   **existing** `source_set` command with the chosen magnet/`.torrent` name.
   There is no silent auto-download and no fabricated source.
5. **Nothing downstream changes.** `item_sources`, `source_for`, the qB sandbox,
   the rolling buffer, the fail-closed probe + Defender gates and cleanup all
   stay exactly as they are. A search result never marks a file ready on its own.
6. **Manual source entry remains.** The existing Source text field is the
   fallback and the override. The search UI is additive.
7. **TMDB catalog is a separate, optional follow-up.** A Discover surface can
   later be added via ADR-0023/P6-07; it is **not** required for source search,
   which starts from an item already in the library. This keeps the first change
   small.

## Alternatives considered

| Option | Changes | Verdict |
|---|---|---|
| A — host-side search + existing `source_set` (chosen) | 1 named command, 1 Details UI panel, secret-store fields, one HTTP/Torznab client dependency; no container/network/schema/gate change | **Proposed** — smallest change that finds and picks a source |
| B — search inside `core` with a new egress route | New network path, container boundary rework, host consent, wider trust surface | Deferred — unnecessary for this goal |
| C — bundle a search service or indexer definitions | New container, content-source bundling, legal exposure | Rejected — violates AGENTS.md non-negotiables |
| D — adopt miauflix code wholesale | Incompatible licence, second product surface, loses fail-closed gates | Rejected |
| E — keep manual source only (ADR-0021 as-is) | Zero change | Retained as fallback; does not solve discovery |

## Consequences

- One new dependency is needed for HTTPS/Torznab requests on the host (an HTTP
  client with rustls). ADR-0023 proposes the same client for TMDB artwork; if
  both ADRs are accepted, one dependency decision covers both. No native TLS.
- The host's trust surface grows by one outbound request path. It remains
  opt-in, operator-configured, loopback-to-renderer and credential-free in DTOs.
- Privacy text must state that title and year are sent when search is used.
- A human lawful-use/legal gate still precedes enablement, as with O-05.
- Ranking must be deterministic and covered by fake-response tests; no live
  indexer call in any test. Live search is **NOT RUN** until an operator supplies
  an endpoint and key.

## Evidence

None. Based on the current `compose.yaml` (host and qB have egress, `core` does
not), the `source_set` command chain, ADR-0005/0007/0014/0018/0021 and the
miauflix README/ToDo. No code, network call, host change or test was performed.
Do not report any part of this as implemented or verified.

## Revisit trigger

Reopen if the human declines the legal gate or the new dependency; if the
operator cannot supply a lawful Torznab endpoint; if result ranking cannot be
made deterministic and auditable; or if a future requirement moves search into
`core` (which would reopen the egress design of the earlier draft).
