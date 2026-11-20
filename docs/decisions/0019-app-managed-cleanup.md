# ADR-0019: Cleanup of watched, app-managed media only

- **Status:** Accepted (2026-11-20)
- **Date:** 2026-11-20
- **Decider:** human partner
- **Scribe:** contributing agent
- **Authority:** the human partner's "i accept the ADRs" (2026-11-20), naming ADR-0019.
- **Related:** ADR-0017, `Development.md` §20.3, AGENTS.md non-negotiables,
  ADR-0005 (completion policy), conversion plan Task 6

## Context

The target specification requires automatic cleanup of watched episodes. The
project's binding rule is "never delete source media" and "automatic permanent
deletion is OFF by default". `Development.md` §20.3 permits automated retention
**only for app-managed downloads**. These are reconciled by scoping cleanup.

## Decision

1. Only files the app itself acquired (tracked in `download_items` / `file_mappings`
   with a torrent hash) are eligible for automated cleanup. **User-imported media
   under library roots is never eligible**, under any setting.
2. Eligibility requires: completion confirmed by the B5 policy (watched ≥90% +
   near-end + EOF event; seeking/close never complete), a replay grace period,
   not actively playing, and not a pending prefetch target.
3. Deletion is coordinated with qBittorrent: for multi-episode torrents, the
   task is removed with `deleteFiles=false` while other episodes are still
   needed, and only genuinely orphaned files are deleted.
4. Every cleanup writes an audit record; failure leaves state `failed` with a
   visible outcome — never falsely marks media as deleted.
5. A file-count regression test asserting zero deletions for non-app-managed
   media is a **release blocker** for this workstream.

## Consequences

- The "no auto-deletion" non-negotiable is narrowed to source media, not removed.
- A deletion capability now exists in the system and needs the traversal/boundary
  tests (F3) and failure drills (F6) extended to cover it.
- The frontend must present cleanup status honestly, including failures.

## Revisit trigger

A data-loss incident, or the human partner narrows/removes automated retention.
