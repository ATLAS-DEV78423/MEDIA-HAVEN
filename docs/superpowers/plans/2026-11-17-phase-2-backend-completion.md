# Phase 2 Backend Completion

> **For the implementing agent:** execute task by task. Tasks 1 → 2 → 3 are sequential (each depends on the previous). Tasks 4 and 5 are independent of each other and only depend on 1–3 being complete. Task 6 is last.

## Goal

Close the backend gaps in Phase 2 (Library) so that `roadmap.md` P2-02 and P2-03 acceptance criteria are met and the CP-2 backend evidence items can be produced. No frontend work in this plan.

Currently: scanning works, roots work, persistence works, three HTTP routes exist — but every scanned file is inserted as `MediaKind::Movie` with a random UUID, and re-scanning a root deletes and recreates every row, which cascades away watchlist/queue/progress state. Scan errors are counted but never reported. There are no tests for cascade behaviour, path escapes at the OS level, resume, duplicates, or index usage.

## Architecture

Single-writer SQLite behind one repository trait (`LibraryRepository` in `crates/app-core/src/ports/library.rs`). Scanner lives in `crates/media-library` and is pure (no DB, no HTTP). Routes in `crates/core/src/routes.rs` are thin: read-only port calls, then map domain errors to `AppErrorCode`. Identity of a media item is derived from its absolute path, not from a database-generated UUID, so rescans are a no-op for unchanged files.

## Tech Stack

Rust 2021, `sqlx` (SQLite, runtime-tokio), `axum`, `serde`, `sha2` + `hex`, `thiserror`, `uuid`. Tests: `#[tokio::test]` + temp directories.

## Spec

- `Development.md` §4.2–§5 (domain + ports), §7 (UI contract — not touched here), §8 (testing)
- `roadmap.md` Phase 2 (P2-01, P2-02, P2-03, CP-2 evidence list)
- `docs/superpowers/plans/02-phase-2-library.md` Tasks 3, 5, 6, 7
- `docs/decisions/README.md` ADR format

## Global Constraints

- Backend only. Do not touch `apps/desktop`.
- No new crates. No new migrations. No schema changes.
- No new dependencies in any `Cargo.toml`.
- Never delete user files. Only database rows are pruned.
- All new SQL uses bound parameters. No string interpolation of values.
- Timestamps are UTC RFC-3339 strings produced by `chrono::Utc::now().to_rfc3339()`.
- `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` must pass at the end of each task.
- Report NOT RUN separately from PASS. A build succeeding is not a test passing.

### Scope explicitly not included (do not add)

- **`MediaUpsert` metadata fields** (year, synopsis, runtime, genres, cast, director). The scanner produces filename-only metadata and P2-06's provider is blocked on O-04, so there is no producer and no consumer. The columns already exist in `0001_init.sql` and are preserved by the upsert; nothing populates them.
- **Artwork cache.** No artwork provider exists (blocked on O-04). A cache with no producer is dead code. Recorded as a documented limitation in Task 6.
- **`crates/metadata` crate / `MetadataProvider` trait.** No implementor, no caller, blocked on O-04.
- **`collections` table / repository / route.** Named in `roadmap.md` P2-03 and `Development.md`'s entity list but absent from every migration and every consumer. Recorded as a deviation ADR in Task 6, not implemented here.
- **Episode → series synthesis.** The scanner emits one row per file. `MediaKind::Series` rows are never produced. Documented limitation.
- **`build_search_query` shared helper** from `02-phase-2-library.md` Task 5. `escape_like` and `contains_match` already exist and work; a second implementation would be duplication. Task 5 below is tests only.

---

## Task 1: Scanner emits a stable id and a real `MediaKind`

**Files**

- Modify: `crates/media-library/src/scanner.rs`
- Modify: `crates/media-library/src/lib.rs`

**Interfaces**

- `pub fn stable_id(path: &str) -> String` — SHA-256 hex of the path bytes. Deterministic across runs for the same path.
- `pub fn classify_kind(extension: &str) -> Option<MediaKind>` — video and audio extensions map to `MediaKind::Movie`; artwork extensions return `None`.
- `ScannedFile` gains `pub id: String` and `pub kind: MediaKind`.

**Why audio maps to `Movie`:** `media_items.kind` has a `CHECK (kind IN ('movie','series','episode'))` constraint. There is no audio kind. Documented as a limitation in Task 6.

**Why artwork is skipped:** images are indexed as media today, which is wrong for a library view. Returning `None` makes the scanner skip the file entirely rather than invent a row. `ScannedFile.kind` stays non-`Option` because skipped files never produce a struct.

**Steps**

- [ ] In `crates/media-library/src/scanner.rs`, add `use app_core::domain::MediaKind;` next to the existing imports.
- [ ] Add `pub fn stable_id(path: &str) -> String` that runs `Sha256::digest(path.as_bytes())` and returns `hex::encode(...)`. Reuse the `sha2`/`hex` imports already used by `hash_file`.
- [ ] Add `pub fn classify_kind(extension: &str) -> Option<MediaKind>` matching the existing `is_media_extension` list: video (`mp4`, `mkv`, `avi`, `mov`, `webm`, `m4v`, `mpg`, `mpeg`, `ts`) and audio (`mp3`, `flac`, `wav`, `aac`, `ogg`, `m4a`, `opus`, `wma`) → `Some(MediaKind::Movie)`; image (`jpg`, `jpeg`, `png`, `webp`, `gif`, `bmp`) and anything else → `None`.
- [ ] Add `id: String` and `kind: MediaKind` to the `ScannedFile` struct.
- [ ] In `scan`, inside the file-push block, before constructing `ScannedFile`, compute `let ext_str = extension_of(path)` once and use it for both the allowlist gate and classification. If `classify_kind(&ext_str)` returns `None`, `continue` without pushing any error (a skipped artwork file is not a scan error).
- [ ] Build the lossy path string once (`let path_string = path.to_string_lossy().to_string();`) and use it for both `stable_id(&path_string)` and the `path` field so the id is derived from exactly the stored text.
- [ ] Keep the existing `title` fallback and `size_bytes` / `hash` behaviour unchanged.
- [ ] In `crates/media-library/src/lib.rs`, add `classify_kind` and `stable_id` to the existing `pub use scanner::{...}` list. Do not add a new `pub use` line.
- [ ] Add unit tests in the `scanner.rs` `#[cfg(test)]` module: `stable_id` is deterministic and differs for two different paths; `classify_kind("mkv") == Some(MediaKind::Movie)`; `classify_kind("mp3") == Some(MediaKind::Movie)`; `classify_kind("png") == None`; `classify_kind("txt") == None`.
- [ ] Run `cargo test -p media-library` and confirm every test passes.
- [ ] Run `cargo clippy -p media-library --all-targets -- -D warnings`.

---

## Task 2: `media_replace_for_root` upserts by id and prunes stale rows

**Files**

- Modify: `crates/persistence/src/sqlite/library_repo.rs` (`media_replace_for_root`, currently ~440–491)

**Interfaces**

- No signature change. `async fn media_replace_for_root(&self, root_id: &str, items: &[MediaUpsert]) -> Result<(), PersistenceError>` keeps its name and arguments — the post-condition is now "the rows for this root equal `items`" rather than "delete everything and insert".

**Behaviour**

1. For each `item` with a non-empty `id`: `INSERT ... ON CONFLICT(id) DO UPDATE`. Preserves `created_at`, `progress_*`, `availability_*`, and the metadata columns written by no one yet.
2. For each upserted `media_id`: `DELETE FROM media_files WHERE media_id = ?` then insert exactly one file row. Without this, rescanning appends a file row per scan.
3. After the loop: `SELECT id FROM media_items WHERE root_id = ?`, diff against the incoming id set in Rust, and delete only the stale ids in chunks of 500.
4. Empty `item.id` is no longer substituted with `Uuid::new_v4()` — it is a contract violation from the caller. Return `PersistenceError::Invalid(...)` (or the crate's existing invalid-input variant) naming the item title.

**Why chunked `IN (...)` and not `NOT IN (json_each(?))`:** avoids depending on SQLite JSON1 being compiled in, and avoids the classic `NOT IN` `NULL` trap. Stale sets are normally a handful of rows.

**Steps**

- [ ] Read the current `media_replace_for_root` end to end before editing.
- [ ] Replace the leading `DELETE FROM media_items WHERE root_id = ?` with nothing — deletion now happens only for stale ids after the upserts.
- [ ] Replace the insert statement with `INSERT INTO media_items (id, kind, title, root_id, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET kind = excluded.kind, title = excluded.title, root_id = excluded.root_id, updated_at = excluded.updated_at`.
- [ ] Keep `let now = chrono::Utc::now().to_rfc3339();` computed once outside the loop and bound for both `created_at` and `updated_at`.
- [ ] Remove the `if item.id.is_empty() { Uuid::new_v4() }` branch; bind `item.id` directly and return an error for the empty case.
- [ ] Before re-inserting the file row, add `sqlx::query("DELETE FROM media_files WHERE media_id = ?").bind(&id)` inside the same transaction.
- [ ] After the upsert loop, collect the incoming ids into a `HashSet<&str>`, run `SELECT id FROM media_items WHERE root_id = ?` on the same connection/transaction, and collect ids not present in the set.
- [ ] If the stale set is non-empty, delete in chunks of 500 using a dynamically built placeholder list (`"DELETE FROM media_items WHERE id IN (?,?,...)"`) with every id bound — never interpolate an id into the SQL string.
- [ ] Confirm the whole function still runs inside whatever transaction boundary it uses today; do not introduce a second transaction.
- [ ] Run `cargo clippy -p persistence --all-targets -- -D warnings` and `cargo test -p persistence`.

---

---

## Task 3: `library_scan` uses the stable id, reports duplicates, and reports errors honestly

**Files**

- Modify: `crates/core/src/routes.rs` (`library_scan`, currently ~336–404)

**Interfaces**

- Response body gains `duplicates`: `[{ "hash": String, "paths": [String] }]`, one entry per hash that appears on two or more scanned files, sorted by hash.
- `errors` changes from a number to `[String]`, each entry `"{path}: {message}"`.
- `scanned` and `cancelled` are unchanged.

**Why `errors` changes type:** P2-02's acceptance criterion is "interrupted scans recovered and reported honestly". A count tells the user nothing. Verified there is no consumer of `library_scan` in `apps/desktop/src`, so the additive/shape change is safe.

**Why duplicates are computed in the route and not the scanner:** the scanner already surfaces `hash`; grouping is three lines and keeps the scanner signature untouched.

**Steps**

- [ ] Add `use std::collections::BTreeMap;` to the imports at the top of `crates/core/src/routes.rs`.
- [ ] In the `.map(|f| MediaUpsert { ... })` closure, replace `id: String::new()` with `id: f.id` and `kind: MediaKind::Movie` with `kind: f.kind`. Use `app_core::domain::MediaKind` as the existing code does.
- [ ] After `let items: Vec<MediaUpsert> = ...;` collect duplicates: iterate `&items`, skip entries whose `hash` is `None`, group paths into a `BTreeMap<String, Vec<String>>` keyed by hash (clone the path string), then drop groups whose `paths.len() < 2` and collect into `Vec<serde_json::Value>` shaped `json!({"hash": h, "paths": p})`.
- [ ] Replace `"errors": outcome.errors.len()` with `"errors": outcome.errors.iter().map(|e| format!("{}: {}", e.path, e.message)).collect::<Vec<String>>()`. Build this before `outcome` is partially moved if the compiler complains — `outcome.files` is moved by `into_iter()`, but `errors` and `cancelled` remain usable on the partial move.
- [ ] Add `"duplicates": duplicates` to the `json!({...})` block.
- [ ] Do not change the `root_update` calls, the cancellation flag, or the error path. A failed scan must still return `AppErrorCode::Internal` with the persistence message.
- [ ] Do not populate `error_message` on the *successful* `root_update` beyond what exists today.
- [ ] `cargo clippy -p core --all-targets -- -D warnings`.
- [ ] `cargo build --workspace` and confirm the route compiles against the new `ScannedFile` fields.

---

## Task 4: Scanner acceptance tests (`crates/media-library/tests/roots.rs`)

**Files**

- Create: `crates/media-library/tests/roots.rs`

**Interfaces**

- Integration tests against `media_library::Scanner` only. No database, no HTTP, no network.
- Fixtures are created under `std::env::temp_dir().join(...)` with a unique per-test suffix and removed at the end of the test.

**Test list (one `#[test]` each)**

- [ ] `path_escape_is_refused` — create a media file outside the root, then a symlink/junction inside the root pointing at it (Windows: `std::os::windows::fs::symlink_file`; skip the assertion with an early `return` when symlink creation fails for lack of privilege, and print why). Assert the outside file never appears in `outcome.files`.
- [ ] `artwork_files_are_skipped` — a `cover.jpg` next to a `movie.mkv` produces exactly one `ScannedFile`, and it is the video.
- [ ] `hostile_filenames_do_not_break_the_scan` — files named with `..`, `%2e%2e`, unicode direction overrides, and a 200-character name are either indexed with a sanitised title or skipped; the scan never panics and never returns a path containing a `..` component.
- [ ] `vanishing_file_is_skipped_not_fatal` — delete a file from a concurrently-iterated directory is hard to make deterministic; instead assert the contract that already holds: a directory containing one unreadable file path (created then removed before `scan` runs on its parent) still yields the readable files and an `errors`/`files` split that sums to the visible entry count. Skip this test on non-Windows if the setup cannot be made reliable, and document the skip inline.
- [ ] `cancel_then_rescan_is_idempotent` — set the cancel flag after the first file, run `scan` twice, and assert the second (uncancelled) run yields the full file set with no duplicate `id` values.
- [ ] `duplicate_hashes_are_detected` — two byte-identical files with different names produce two `ScannedFile`s with equal `hash`, and `files.len() == 2`.
- [ ] `stable_ids_are_unique_per_path` — every `ScannedFile.id` in a multi-file fixture is distinct and 64 hex chars.
- [ ] `empty_root_is_not_an_error` — an empty directory yields `files.is_empty()`, `errors.is_empty()`, `cancelled == false`.

**Steps**

- [ ] Create the file with `use media_library::{MediaKind, Scanner};` and `use std::sync::{atomic::AtomicBool, Arc};`.
- [ ] Add a helper `fn temp_root(name: &str) -> std::path::PathBuf` that creates `temp_dir().join(format!("mh-{name}-{}", std::process::id()))`, removes it if it exists, creates it, and returns the path.
- [ ] Add a helper `fn write_file(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf` using `std::fs::create_dir_all` + `std::fs::write`.
- [ ] Add a helper `fn scan(dir: &Path) -> media_library::ScanOutcome` that builds a fresh `AtomicBool` and calls `Scanner::new().scan(dir, &cancel)`.
- [ ] Implement each test above. Use `std::fs::remove_dir_all` at the end, ignoring the error.
- [ ] Run `cargo test -p media-library --test roots -v` and record the pass/fail count per test. If a Windows symlink test cannot create the link, report it as SKIPPED with the reason, not PASS.
- [ ] Run `cargo clippy -p media-library --all-targets -- -D warnings`.

---

## Task 5: Persistence acceptance tests (cascade, prune, resume, search, indexes)

**Files**

- Modify: `crates/persistence/tests/repository.rs`

**Interfaces**

- Reuse the existing `Fixture` + `seed()` helpers already in that file. Do not add a second harness.
- Any new fixture data uses the existing `MediaUpsert` shape with **non-empty** `id` values (Task 2 rejects empty ids).

**Test list**

- [ ] `rescan_preserves_watchlist_and_queue` — seed a root + item, add a watchlist entry and a queue entry pointing at it, run `media_replace_for_root` again with the same id, assert both rows still exist and `media_items.created_at` is unchanged.
- [ ] `rescan_preserves_progress_and_availability` — write a progress row and an availability flag for an item, rescan, assert both survive with the same values.
- [ ] `prune_removes_only_stale_ids` — scan set `{a, b}`, then `{a, c}`; assert `b` is gone, `a` and `c` exist, and the row count for the root is 2.
- [ ] `rescan_does_not_accumulate_media_files_rows` — rescan the same single file three times; assert `SELECT COUNT(*) FROM media_files WHERE media_id = ?` is 1.
- [ ] `empty_id_is_rejected` — `media_replace_for_root` with `id: String::new()` returns an error and leaves the table unchanged.
- [ ] `search_treats_like_metacharacters_literally` — insert titles `100% Pure`, `a_b`, `a%b`, then search for `%` and for `_`; assert each returns only the literal match and that a `100` query does not match `100% Pure` by pattern expansion.
- [ ] `search_query_uses_escape_clause` — assert the built statement contains `ESCAPE` by inspecting `sqlx::query` construction indirectly: run a search whose term contains `%` and assert the result set proves the escaping (behavioural assertion, not string inspection).
- [ ] `timestamps_round_trip_as_utc` — write an item, read it back, parse both with `chrono::DateTime::parse_from_rfc3339`, assert the offset is `+00:00` and the instant matches within one second.
- [ ] `browse_query_plan_uses_the_root_index` — run `EXPLAIN QUERY PLAN SELECT id FROM media_items WHERE root_id = ?` on a connection with at least one row and assert the plan text mentions an index (`idx_media_root` or `sqlite_autoindex`), not `SCAN media_items`.
- [ ] `#[ignore] searching_10k_items_is_within_budget` — insert 10,000 rows using bound parameters inside one transaction, run the browse and search queries, and assert the search completes in under 200 ms. Mark `#[ignore]` so the default suite stays fast; document how to run it (`cargo test -p persistence -- --ignored`).

**Steps**

- [ ] Read the top of `crates/persistence/tests/repository.rs` to learn the exact `Fixture` constructor and `seed()` signature before writing anything.
- [ ] Add each test as a `#[tokio::test]` using the existing fixture pattern. Match the file's existing assertion style.
- [ ] For the index-plan tests, fetch the plan with `sqlx::query("EXPLAIN QUERY PLAN ...")` and join the `detail` column values into one string before asserting.
- [ ] For the 10k test, reuse the same `media_items` insert path as production (`media_replace_for_root`) rather than a hand-written INSERT, so the fixture is representative; time only the search call with `std::time::Instant`.
- [ ] Run `cargo test -p persistence` and record the result. Then run `cargo test -p persistence -- --ignored` separately and record the timing.
- [ ] Run `cargo clippy -p persistence --all-targets -- -D warnings`.

---

## Task 6: Record the deviations and update the roadmap/progress

**Files**

- Create: `docs/decisions/0010-library-entity-deviations.md`
- Create: `docs/decisions/0011-metadata-provider-selection.md`
- Modify: `docs/decisions/README.md` (index table)
- Modify: `roadmap.md` (P2-02 / P2-03 status and CP-2 evidence notes)
- Modify: `progress.md` (what shipped, what is still open)

**Content — ADR 0010 (proposed, not accepted)**

Context: the library entity set in `Development.md` names `collections`, but no migration, repository method, route, DTO, or UI consumer exists. Also, `media_items.kind` has no audio kind, the scanner emits no series rows, ids derive from paths, and there is no artwork cache.

Decision: record the current backend behaviour as-is rather than inventing schema. Specifically:
- `collections` is not implemented. It is deferred until a consumer exists.
- Audio extensions are stored as `MediaKind::Movie` because the schema `CHECK` allows only movie/series/episode.
- One row per file; no episode→series synthesis.
- `media_items.id` is the SHA-256 of the absolute path, so moving a library root changes ids and resets per-item user state. Users must be told in the UI when a root is moved.
- No artwork cache and no provider; blocked on O-04.

Consequences: the library view can render today without a metadata provider; per-item state is stable across rescans but not across root moves; `collections` remains a roadmap gap rather than silent scope loss.

**Content — ADR 0011 (proposed, not accepted)**

Context: P2-06 needs a metadata provider and O-04 is unresolved (provider choice + terms). The plan proposes options; it does not choose one.

Options to list: TMDB API, OMDb, TVmaze, local NFO/embedded tags only, and a plugin-style provider trait with no default. For each: auth/terms, rate limits, whether an offline/no-network mode is possible, and the licence implications of caching artwork.

Decision: none. Status `proposed`. The implementing agent may not mark it `accepted` (README forbids self-approval).

**Steps**

- [ ] Read `docs/decisions/README.md` end to end for the exact template headings and the status vocabulary.
- [ ] Write `0010` and `0011` following that template exactly (Context / Decision / Alternatives considered / Consequences / Evidence / Revisit trigger).
- [ ] In `README.md`, add index rows for `0010` and `0011` with status `proposed`.
- [ ] In `roadmap.md`, leave P2-01/02/03 at `[~]`; add a short note under each listing which acceptance criteria this plan closes and which remain (frontend, `collections`, provider). Do not mark any criterion done that this plan does not cover.
- [ ] In `roadmap.md`, add a CP-2 evidence note recording that the backend-side evidence items are now producible and naming the commands that produce them.
- [ ] In `progress.md`, append a dated entry: what changed (Tasks 1–5), what was tested and observed, what remains untested (real HTTP scan against a large real folder, Windows reparse-point refusal if skipped), and the next roadmap task.
- [ ] Do not claim PASS for anything not actually run. List NOT RUN explicitly.
- [ ] Run `cargo build --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo fmt --check`; record each result.

---

## Self-review

- [ ] Every task's file paths exist in the live tree (verified: `crates/media-library/src/scanner.rs`, `crates/core/src/routes.rs`, `crates/persistence/src/sqlite/library_repo.rs`, `crates/persistence/tests/repository.rs`).
- [ ] No step contains `TODO`, `...`, `similar to`, or an unnamed function.
- [ ] No new crate, dependency, migration, or schema change appears anywhere in Tasks 1–6.
- [ ] The type names in Task 3 match Task 1's outputs (`ScannedFile.id: String`, `ScannedFile.kind: MediaKind`).
- [ ] The persistence error variant named in Task 2 is the one that actually exists in `PersistenceError`; confirm before editing.
- [ ] Task 4 and Task 5 touch disjoint files and can be run in parallel.
- [ ] Task 6's ADR statuses are `proposed`, never `accepted`.
