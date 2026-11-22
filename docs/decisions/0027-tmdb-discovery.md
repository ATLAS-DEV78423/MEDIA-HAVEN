# ADR-0027: TMDB discovery catalogue (display-only)

- **Status:** Accepted (2026-11-22) — human: use TMDB for the catalogue too.
- **Date:** 2026-11-22
- **Related:** ADR-0026 (operator token), ADR-0005 (named commands), AGENTS.md (no content-source bundling)
- **Decision:** Movies/TV/Anime browse pages gain a Discover mode backed by TMDB's public discover endpoints, using the operator's token. Anime is approximated with `with_genres=16&with_original_language=ja`; TMDB has no dedicated anime type and coverage is incomplete. Results are display-only: never downloaded, played, or written into the local library. Nothing is bundled beyond the upstream provider client. Attribution and the existing artwork cache rules apply.
- **Revisit:** if the human wants a dedicated anime source (e.g. Jikan/AniList) or downloadable catalog entries, write a superseding ADR.

## Context

The human asked to browse a catalogue using TMDB as well, rather than bundling a
site list or indexer. The operator already supplies a TMDB token under ADR-0026.
A live probe on 2026-11-22 confirmed `/discover/movie`, `/discover/tv` and
`/discover/tv?with_genres=16&with_original_language=ja&sort_by=popularity.desc&page=1`
each return 20 results with that token.

## Cost and evidence

- Cost: one display-only renderer command, reuse of the existing 30-day
  size-capped cache, and up to 20 TMDB requests per discovery page for posters.
  No new runtime, dependency or bundled catalogue data.
- Evidence: controller's live TMDB probe (2026-11-22): the three endpoints above
  each returned 20 results. Anime coverage is explicitly incomplete because the
  query is a genre/language filter, not a dedicated anime catalogue.
