# Catalog and source discovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Add an opt-in TMDB catalog and an operator-configured Torznab source search whose chosen magnet flows through the existing `source_set` → qBittorrent → probe/Defender → playback chain.

**Architecture:** All new HTTP runs on the Windows host as host-only named commands (no `COMMANDS`, no backend route, no container egress change) except one small core host route that creates a "wanted" `media_items` row so a catalog title has a `mediaId` for `source_set`. Secrets use DPAPI via `secret.rs`. Catalog picks create a wanted item, open the existing Details screen, then use the same shortlist as library titles.

**Tech Stack:** Rust 1.99, Tauri 2, `reqwest` 0.13 with rustls + blocking (transitive in Cargo.lock; declared as a direct dependency — accepted by the human override), React 19 + TypeScript.

**Spec:** `docs/superpowers/specs/2026-11-24-catalog-and-source-discovery-design.md` (ADR-0024 Accepted 2026-11-24).

## Global Constraints

- No bundled indexers, tracker lists, content-site catalogues or unlawful-acquisition instructions. Operator supplies endpoint/key; absent config disables the feature with zero requests.
- Host-only commands. `COMMANDS` stays 24. No new renderer DTO; renderer receives only display data (title, year, poster URL, shortlist fields). No path, token, credential or HWND in a renderer payload.
- `core` keeps `MEDIAHAVEN_EGRESS=none`; only the host and qB have egress.
- Opt-in TMDB: flag + key both required; only title/year/poster path leave the machine. Exact attribution string from `tmdb.rs::TMDB_ATTRIBUTION`.
- CSS uses tokens only: no `#hex`, `rgb(...)`, or `\d+px` literals outside `tokens.css` (`check:tokens` fails otherwise).
- No commits. No live indexer/TMDB call in any test (fake fetch only). Live calls are NOT RUN until an operator supplies credentials.
- Ponytail discipline: smallest change that fully works, no abstraction, finish all callers/tests.

## Fixed contracts (lanes depend on these verbatim)

- Host command `source_search(title: String, year: Option<i64>) -> Result<Value, CommandError>` → host-only; frontend wrapper `api.sourceSearch(title, year)` calls `"source_search"` with `{ title, year }`.
- Host command `catalog_browse(kind: String) -> Result<Value, CommandError>` → `{ kind: "movie" | "tv" }`; returns `{ items: CatalogItem[] }`.
- Host command `catalog_search(query: String) -> Result<Value, CommandError>` → `{ query }`; returns `{ items: CatalogItem[] }`.
- `CatalogItem` (display-only JSON): `{ id: String (TMDB numeric id as string), title: String, year: Option<i64>, kind: "movie"|"series", overview: String, posterPath: Option<String> }`.
- Core host route `POST /host/catalog_wanted_add` with body `{ title, year, kind }` returns `{ mediaId }`; host command `catalog_wanted_add(title, year, kind)` forwards to it. Generated `mediaId` is stable: `format!("catalog-{}-{}", kind, sha256(title|year) hex 32)`.
- Shortlist item (display-only): `{ title: String, sizeBytes: i64, seeders: i64, resolution: String, source: String, magnet: String }`. The renderer receives the magnet **only after search** and passes it verbatim to the existing `source_set`, exactly as the manual Source field does today.

---

### Task 1 — Lane A: host HTTP clients, secrets and commands

**Files:** Modify `apps/desktop/src-tauri/Cargo.toml`, `secret.rs`, `commands/renderer.rs`, `lib.rs`; Create `apps/desktop/src-tauri/src/discovery.rs` and its tests.

- [ ] **Step 1:** Add `reqwest = { version = "0.13", features = ["blocking", "rustls-tls"], default-features = false }` to the host crate's dependencies (record the version in the ADR index).
- [ ] **Step 2:** In `secret.rs`, add `pub fn store_string(service: &str, account: &str, value: &str) -> Result<(), SecretError>` and `pub fn load_string(service: &str, account: &str) -> Result<Option<String>, SecretError>` using the existing DPAPI `protect`/`unprotect`, blob path and `valid_identity` gate. Add round-trip and missing-blob tests with scratch identities.
- [ ] **Step 3:** In `discovery.rs`, implement a Torznab client with an injected fetch seam (like `TmdbProvider::with_fetch`): `torznab_search(endpoint, key, title, year, fetch) -> Result<Vec<ShortlistItem>, DiscoveryError>`. Parse RSS/XML with a bounded hand-rolled parser or `serde_json` only if the endpoint returns JSON; do not add another crate. Cap response at 1 MiB and results at 20. Deterministic ranking: seeders desc, then size desc, then resolution desc, then title asc. Sanitise every display field (drop control chars, cap length); reject path-like titles. Add fake-response tests (ranking order, hostile data, size cap, missing config ⇒ zero calls).
- [ ] **Step 4:** In `discovery.rs`, implement TMDB browse/search with an injected fetch seam: `tmdb_list(path, query, key, fetch) -> Vec<CatalogItem>` calling `https://api.themoviedb.org/3/{movie/popular|tv/popular|search/multi}` with only title/query + key. Cap 1 MiB and 40 items, sanitise fields, force `kind` to the closed set. Add fake-response tests.
- [ ] **Step 5:** In `renderer.rs`, add the three host-only commands from Fixed Contracts. Config from env: `MEDIAHAVEN_TORZNAB_URL`, `MEDIAHAVEN_TORZNAB_KEY` (loaded through `secret.rs` if present, else env), `MEDIAHAVEN_TMDB_OPTIN=1` + key via `secret.rs`/env. Absent config ⇒ empty list + a display-only `setup_required` flag, never an error leak. Register all three in `lib.rs::generate_handler!` only (not `COMMANDS`).
- [ ] **Step 6:** Gates: `cargo fmt --all -- --check`, `cargo clippy -p mediahaven-desktop --all-targets -- -D warnings`, `cargo test -p mediahaven-desktop --lib`, `cargo test -p mediahaven-contract command_lists_derive_from_one_source` (24 intact).

### Task 2 — Lane B: core "wanted" item route

**Files:** Modify `crates/app-core/src/ports/library.rs`, `crates/persistence/src/sqlite/library_repo.rs`, `crates/core/src/acquisition.rs` (or routes module) and tests. Do NOT touch host `renderer.rs`/`lib.rs` or any frontend file.

- [ ] **Step 1:** Add port method `fn wanted_add(&self, title: &str, year: Option<i64>, kind: &str, id: &str) -> Result<(), AppError>` with a stable id computed by the caller.
- [ ] **Step 2:** Implement in SQLite: INSERT into `media_items` with `availability_state='unavailable'`, empty genres, UTC `created_at`/`updated_at`, `ON CONFLICT(id) DO NOTHING`. No `media_files` row. Add a test that repeated calls are idempotent and that no file is created.
- [ ] **Step 3:** Add the signed host route `POST /host/catalog_wanted_add` accepting `{title, year, kind}`, computing the stable id `catalog-{kind}-{sha256(title|year)[..32]}`, calling the port, returning `{ mediaId }`. Validate kind and reject hostile title/path-like data. Add a route test using the existing fake-token harness.
- [ ] **Step 4:** Gates: fmt, clippy on the three crates, `cargo test -p app-core -p media-persistence -p mediahaven-core`.

### Task 3 — Lane C: frontend catalog + shortlist UI

**Files:** Modify `apps/desktop/src/services/api.ts`, `apps/desktop/src/pages/Details/MediaDetailsPage.tsx` (+ test), Create `apps/desktop/src/pages/Discover/` (page, CSS, test), Modify `src/App.tsx` and `src/components/nav/Nav.tsx`. Do NOT touch any Rust file.

- [ ] **Step 1:** Add api wrappers: `sourceSearch(title, year)`, `catalogBrowse(kind)`, `catalogSearch(query)`, `catalogWantedAdd(title, year, kind)` — each `call<...>` with the exact Fixed Contracts payloads. Add wrappers tests.
- [ ] **Step 2:** Details: add a "Find sources" panel. On click call `sourceSearch(title, year)`; render the ranked shortlist (size, seeders, resolution, source) with a Confirm button per row that calls the existing `api.sourceSet(mediaId, magnet)` and refreshes. Keep the manual Source field. Show `setup_required` as a clear state. Add a test asserting confirm passes the chosen magnet to `source_set` and that no path/token is rendered.
- [ ] **Step 3:** Discover page: route `/discover`, Nav tab "Discover". Tabs Movies/TV, a search field calling `catalogSearch`, shelves/grid calling `catalogBrowse`. Cards use existing `MediaCard`/`Shelf` or a small local card. Clicking a card calls `catalogWantedAdd` then navigates to `/details/:mediaId`. Render `TMDB_ATTRIBUTION` text where TMDB data appears. Use tokens-only CSS. Add tests for browse, search, and the click→wanted→navigate flow.
- [ ] **Step 4:** Gates: `npm run typecheck`, `npm run lint`, `npx vitest run`, `npm run check:tokens`, `npm run build`, `npm run check:bundle`.

---

## Final gate (controller)

Run all gates above on the finished tree plus `git diff --check`. Dispatch a whole-change reviewer. Record PASS/FAIL/NOT RUN in `progress.md`. No live indexer/TMDB call; no commit; no checkpoint passed.

## Self-review

Every spec section maps to a task: operator endpoint + no bundling (Global Constraints/Task 1), opt-in TMDB (Task 1/3), shortlist + confirm + existing source_set (Task 1/3), wanted item (Task 2), no renderer paths (all), tests without live calls (all). No placeholders: all command names, payloads and paths are fixed above. Type consistency: `CatalogItem` fields are identical in Task 1 output and Task 3 consumption; `mediaId` from Task 2 is passed unchanged to existing Details/`source_set`.
