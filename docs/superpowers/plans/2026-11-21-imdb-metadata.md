# IMDb Metadata Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add IMDb-sourced ratings, age/certification ratings, and listings (title/year/genres) to movies and shows, displayed in the UI, fetched only on explicit opt-in and offline-ready once cached.

**Architecture:** A new `imdb` provider module in `crates/metadata`, behind a `MetadataProvider`-style trait with an **injectable fetch seam** so all logic is testable with no network and no new dependency. Two data paths, both opt-in and off by default: (1) **official IMDb datasets** (title.basics/ratings TSV, downloadable by the operator — ratings/listings, no age ratings in the datasets); (2) **OMDb API** via operator-supplied key for age ratings (certification) and richer detail. Results cached in SQLite with TTL + size cap, sanitised, attributed ("Ratings & age ratings: IMDb / OMDb"), and merged into `MediaSummary`/`MediaDetail` as display-only fields. Network egress stays opt-in per ADR‑0011; local NFO/TMDB behaviour unchanged.

**Tech Stack:** Rust (serde_json, std), SQLite cache, React 19 display. NO new crate — the HTTPS transport is a seam; if a TLS/HTTP client is required later it needs an ADR. Official datasets may be provided as plain TSV files by the operator (no gzip dependency in v1).

**Spec:** user request 2026‑11‑21; ADR‑0011 (opt-in network, attribution, cache cap); ADR‑0018 (no bundled sources); AGENTS.md (no new dependency without ADR).

## Global Constraints

- OPT-IN ONLY, off by default. No request leaves the machine until the operator enables it. The UI opt-in text must name exactly what is sent (title/year or IMDb id) and to whom (IMDb datasets / OMDb).
- NO bundled content source, indexer, or "find a rating" scraper. Operator supplies dataset files or an OMDb key. Lawful use only.
- No new dependency without an ADR; no path/token in renderer DTOs; no commit unless instructed; Playwright/vite untouched.
- Attribution rendered wherever IMDb data appears. Cache size cap + TTL + eviction. Failures degrade to local data, never break a view.
- `PASS`/`FAIL`/`NOT RUN` describe observed runs. No live IMDb/OMDb call in tests — fake fetch only; real network call NOT RUN without opt-in/key.

---

## Task 0: ADR‑0022 + IMDb provider core (offline/testable)

**Files:** create `docs/decisions/0022-imdb-metadata.md`; create `crates/metadata/src/imdb.rs`; modify `crates/metadata/src/lib.rs`, `crates/metadata/src/model.rs`; create `crates/metadata/tests/imdb.rs`; report at `.superpowers/sdd/2026-11-21-imdb/task-0-report.md`.

**Interfaces:**
- Produces: `struct ImdbRecord { imdb_id, title, year, genres, rating: Option<f64>, votes: Option<i64>, age_rating: Option<String> }`; `trait ImdbFetch { fn basics(&self) -> Result<Vec<ImdbRecord>, ImdbError>; fn ratings(&self) -> ...; fn by_id(&self, imdb_id)->... }` (fake for tests); `ImdbProvider` with opt-in flag, cache, and `IMDB_ATTRIBUTION` constant.

- [ ] **Step 1 — ADR‑0022:** record the decision (IMDb datasets for ratings/listings; OMDb seam for age ratings; opt-in, attribution, no bundling) as **Proposed** with the human's 2026‑11‑21 request cited; note it does not replace ADR‑0011 (TMDB primary, local NFO fallback) — IMDb is additive and opt-in. Index it.
- [ ] **Step 2 — failing tests** with an in-memory fake: parse an IMDb `title.basics.tsv` fixture (tconst, titleType, primaryTitle, startYear, genres) + `title.ratings.tsv` fixture (averageRating, numVotes) into records; merge by id; sanitise hostile titles; skip `\\N` nulls. Test: valid rows parsed, hostile title sanitised, unknown id → None.
- [ ] **Step 3 — implement** `imdb.rs`: pure TSV parser (tab-split, header skip, `\\N` → None), merge, sanitisation (reuse the existing sanitiser pattern from `nfo.rs`), `ImdbFetch` trait + fake. Age ratings come from a separate OMDb-shaped record seam (`age_rating` field populated by the OMDb provider in Task 1).
- [ ] **Step 4 — verify:** fmt/clippy/tests 0; no network called.

## Task 1: OMDb age-rating provider + cache + opt-in

**Files:** `crates/metadata/src/omdb.rs` (new), `crates/metadata/src/lib.rs`, `crates/metadata/src/cache.rs` (extend), new migration `0008_imdb_cache.sql` under persistence, settings DTO/app-core, report.

- [ ] `OmdbFetch` trait + fake; `age_rating_for(imdb_id)` returns a certification string (e.g. "PG-13") with sanitisation; API key from env/secret store, never source/repo; missing key → `Unavailable`, never fabricated.
- [ ] Cache table: `imdb_cache(id, payload, fetched_at, source)` with 30-day TTL, byte cap + oldest-first eviction; test eviction and freshness with temp/seeded data.
- [ ] Settings: `imdb.enabled` (default false), `omdb.enabled` (default false), attribution string; opt-in text names what is sent. Backend validates; no egress when disabled (request counter test).
- [ ] Verify as above; no live key call.

## Task 2: UI display + attribution

**Files:** `apps/desktop/src/services/types.ts`, Details/Movies/Shows components, `docs/operations/*` (privacy note), report.

- [ ] Display-only fields on DTOs: `imdbRating`, `voteCount`, `ageRating` (all optional). Display in Details and on cards where space allows; render `IMDB_ATTRIBUTION` ("Ratings and age ratings: IMDb / OMDb") where data appears. Empty state when not enabled/cached — never invent a rating.
- [ ] Privacy doc updated: what is sent, to whom, opt-in default off. Vitest with a fake DTO: rating shown, no rating -> no invented value, attribution present.
- [ ] Verify full frontend gate; no path/token in DTOs.

## Sequencing

Task 0 → Task 1 → Task 2 (shared types/cache). All before/after the release-blocker plan — disjoint files, may run in parallel with that plan's Task 3 (packaging). No live network or checkpoint passed by agents.
