# Seeded Sources And Search Plan

> **For agentic workers:** REQUIRED SUB-SKILL: subagent-driven-development. Steps use checkboxes.

**Goal:** Ship the provider definitions inside the app, install them into the user's Prowlarr with one click, and let the user search those providers from a title's Details page and send a chosen result to the downloader.

**Plain-language summary:** After giving the app their Prowlarr address and key, the user clicks "Set up sources" once. The app adds Nyaa, YTS, The Pirate Bay and the other saved providers to their Prowlarr. Then, on a movie or episode page, "Find sources" shows a short list. Choosing one sends it to qBittorrent, which still checks for `.exe` files and scans the download before playback.

**Architecture:** Copy five definition files into the core crate and embed them with `include_str!`. Add a host-internal route in `core` (`/host/indexers_seed`) that reads the Prowlarr URL/key from environment and uses `reqwest` to add the providers through Prowlarr's own API. Add `/host/source_search` that queries each enabled Prowlarr indexer's Torznab feed, parses the XML with simple string scanning (no new crate), normalizes and ranks results, and returns path-free candidates. The renderer gets two host-only Tauri commands and a small shortlist panel on Details. Nothing is downloaded automatically.

**Tech Stack:** Rust (existing `reqwest`, `serde_json`), Tauri commands, React 19. No new dependency.

**Spec:** user instruction 2026-11-22: use the providers from the Miauflix repo itself, seed them in the codebase, plug and play after Prowlarr URL + TMDB key. `docs/operations/prowlarr-setup.md`. ADR-0018/0019 (download boundary, app-managed cleanup).

## Global Constraints

- Definitions are stored in our repo **with attribution** to `Prowlarr/Indexers` and `maury91/miauflix-indexers`. No other site list is added.
- Prowlarr URL/key come only from the DPAPI secret store; never in source, DTO, log or error.
- Search results are candidates only. No auto-download, no auto-play. Sending a result reuses existing `download_add` + `source_set` and the executable guard, ffprobe and Defender gates.
- No new renderer command (contract stays 25); host routes are not renderer commands.
- Cap 10 results per search and 2 seconds per indexer request. Failures degrade to an empty list.
- `PASS`/`FAIL`/`NOT RUN` honest. Tests use fake HTTP data; no live indexer call in tests.

---

## Task 1: Bundle the provider definitions

**Files:** create `crates/core/resources/indexers/{yts,nyaasi,thepiratebay,eztv-api,animetosho}.yml`, `crates/core/resources/indexers/ATTRIBUTION.md`, `crates/core/src/indexers.rs`; modify `crates/core/src/lib.rs`.

**Interfaces:**
- Produces: `pub const BUNDLED_INDEXERS: &[BundledIndexer]` in `crates/core/src/indexers.rs`, where `BundledIndexer { id: &'static str, name: &'static str, definition_file: &'static str, yaml: &'static str }`.

- [ ] Step 1: copy these exact files (already downloaded to temp during research) into `crates/core/resources/indexers/`:
  - `yts.yml` from `https://raw.githubusercontent.com/Prowlarr/Indexers/master/definitions/v11/yts.yml`
  - `nyaasi.yml` from `https://raw.githubusercontent.com/Prowlarr/Indexers/master/definitions/v11/nyaasi.yml`
  - `thepiratebay.yml` from `https://raw.githubusercontent.com/Prowlarr/Indexers/master/definitions/v11/thepiratebay.yml`
  - `eztv-api.yml` from `https://raw.githubusercontent.com/maury91/miauflix-indexers/main/definitions/eztv-api.yml`
  - `animetosho.yml` from `https://raw.githubusercontent.com/maury91/miauflix-indexers/main/definitions/animetosho.yml`

  Write `crates/core/resources/indexers/ATTRIBUTION.md` naming both upstream repositories and stating that the files are copied verbatim for operator convenience.

- [ ] Step 2: write a failing test in `crates/core/src/indexers.rs` asserting `BUNDLED_INDEXERS.len() == 5` and every `yaml` starts with `---` and contains an `id:` line.

- [ ] Step 3: implement the constants in `crates/core/src/indexers.rs` with paths relative to that file:
```rust
include_str!("../resources/indexers/yts.yml")
```
and the same shape for the other four. There is exactly one copy of each file, under `crates/core/resources/indexers/`.

- [ ] Step 4: run `cargo test -p mediahaven-core indexers` — expect PASS.

---

## Task 2: Seed the providers into Prowlarr

**Files:** modify `crates/core/src/acquisition.rs`, `apps/desktop/src-tauri/src/commands/renderer.rs`, `lib.rs`; create `apps/desktop/src/services/sources.ts` + test; modify `apps/desktop/src/pages/Settings/SettingsPage.tsx`.

**Interfaces:**
- Host route `POST /host/indexers_seed` → JSON `{ added: string[], skipped: string[] }`.
- Tauri command `indexers_seed() -> Result<Value, String>`.

- [ ] Step 1: failing test in `acquisition.rs`: with a fake HTTP client, `seed_indexers` returns added ids for the three Prowlarr-native definitions and skipped ids for the two custom ones.

- [ ] Step 2: implement `seed_indexers` in `acquisition.rs`:
  - read `MEDIAHAVEN_PROWLARR_URL` and `MEDIAHAVEN_PROWLARR_API_KEY` from env; if either blank, return `NotFound`-style empty result.
  - GET `/api/v1/indexer/schema?apikey=…` once and keep the JSON array. For each id in `["yts", "nyaasi", "thepiratebay"]`, find the object whose `definitionName` equals that id. POST `/api/v1/indexer` with this exact body shape (proved to return 200 in research for YTS, Nyaa and The Pirate Bay):
```json
{"name":"<schema name>","implementation":"<schema implementation>","configContract":"<schema configContract>","protocol":"<schema protocol>","appProfileId":1,"enable":true,"priority":1,"tags":[],"fields":"<schema fields array>","settings":{"definitionFile":"<definitionName>","baseUrl":"<first indexerUrls value>"}}
```
  The `fields` array must be the schema object's own `fields`, not empty; without it Prowlarr returns 400.
  - skip ids that already exist (compare `GET /api/v1/indexer` names).
  - return fixed, path-free strings only.

- [ ] Step 3: add the host route to `host_router()` and the Tauri command + registration.

- [ ] Step 4: add `sources.ts` with `seedSources(): Promise<{added: string[], skipped: string[]}>` calling the host command, with a dev shim.

- [ ] Step 5: Settings button "Set up sources" calling `seedSources()` and showing counts only. Add test that no URL/key appears in the rendered DOM.

- [ ] Step 6: gates: `cargo fmt --check`, clippy, `cargo test -p mediahaven-core`, frontend six gates.

---

## Task 3: Search and shortlist

**Files:** modify `crates/core/src/acquisition.rs`, `apps/desktop/src-tauri/src/commands/renderer.rs`, `lib.rs`; create `apps/desktop/src/components/cards/SourceResults.tsx` + CSS + test; modify `apps/desktop/src/pages/Details/MediaDetailsPage.tsx`, `apps/desktop/src/services/api.ts`, `roadmap.md`, `progress.md`.

**Interfaces:**
- Host route `POST /host/source_search` with `{ title, year?, season?, episode? }` → `{ results: [{ id, title, size, seeders, magnet }] }` (max 10).
- Tauri command `source_search(args: Value) -> Result<Value, String>`.

- [ ] Step 1: failing test with fake Torznab XML containing one matching and one wrong-year item; assert only the matching one is returned and magnet is present.

- [ ] Step 2: implement `source_search`:
  - GET `/api/v1/indexer?apikey=…` for enabled ids.
  - For each id, GET `/api/v1/indexer/{id}/newznab?apikey=…&t=search&q=<title>` (proved 200 during research), cap 2 s.
  - Split response on `<item>`, read `<title>`, `<magnet:link>` or `<link>`, `<torznab:attr name="seeders" value="…">`.
  - Accept only items whose normalized title starts with the requested title and whose year/season/episode match when supplied. Sort by seeders desc, take 10.
  - Errors are fixed strings; no URL or key in output.

- [ ] Step 3: add host route + Tauri command.

- [ ] Step 4: renderer: `sourceSearch(args)` in `api.ts`; `SourceResults` panel on Details with a "Find sources" button. Selecting a result calls the existing download command then `api.sourceSet(media.id, magnet)`; show fixed status text. No auto-selection.

- [ ] Step 5: docs: ADR-0029 for seeded sources + search; P6-11 entry; progress entry with live search NOT RUN until the user has indexers.

- [ ] Step 6: gates: Rust fmt/clippy/tests + frontend six gates + `cargo check -p mediahaven-desktop --all-targets`.

---

## Self-review

All three user requirements have tasks: definitions in codebase, one-click setup, search from Details. No placeholders. Names are consistent. No new dependency. Secrets stay in the store.
