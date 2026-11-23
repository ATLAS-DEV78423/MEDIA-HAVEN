# Progress

## Last updated

2026-11-22 (startup wiring, Prowlarr inputs, bundled providers, one-click
seeding and Details search complete; live Prowlarr seeding and live Torznab
search **OBSERVED** against this machine's Prowlarr; packaged-app cold start,
live download and browser screenshot still NOT RUN). Merged the two GitHub
documentation commits from `master` into this branch on 2026-11-22. The full
pending list is in `whats-left.md`. Earlier 2026-11-22 and 2026-11-21 entries
are retained as history and are explicitly superseded where they disagree with
the newest entry.

## 2026-11-22 — Seeded sources and search from Details (plan Task 3; live search NOT RUN)

Implemented per `docs/superpowers/plans/2026-11-22-seeded-sources-and-search.md`
on worktree `ea5b46` (branch `tmdb-artwork-and-exe-guard`, base `c7b0476`, which
already carried plan Tasks 1–2). No new dependency, no push/merge/install.
Full detail: `.superpowers/sdd/2026-11-22-seeded-sources-and-search/task-3-report.md`.

- **Core search.** `POST /host/source_search` reuses the seeder's single
  `ProwlarrHttp` / `ProwlarrFetchFn` seam — no second client. It reads the
  address/key only from `core`'s env, GETs `/api/v1/indexer` for enabled ids,
  GETs `/api/v1/indexer/{id}/newznab` (2 s timeout, form-encoded `q`), splits the
  XML on `<item>`, and accepts only normalized-title prefixes with matching
  year/season/episode when supplied. Results sort by seeders desc, cap at ten,
  and carry only `{id, title, size, seeders, magnet}`. Blank config, bad response
  and per-indexer failure degrade to fewer candidates with fixed strings; no
  address, key, path or raw response text is returned.
- **Host/renderer.** Tauri command `source_search(args)` forwards to the host
  route through the existing signed `post_internal`; contract stays at 25.
  `api.sourceSearch(query)` and a `SourceResults` panel on Details add a "Find
  sources" button. Results are candidates only: choosing one calls the existing
  `download_add` then `api.sourceSet(mediaId, magnet)` and shows fixed status
  text. No auto-download, auto-selection or playback from the panel; the
  executable guard and probe gates are untouched.
- **Docs.** ADR-0029 recorded **Accepted (2026-11-22, human instruction)** by
  the agent as scribe and indexed in `docs/decisions/README.md`; P6-11 added to
  `roadmap.md`; no checkpoint passed.

### Verified commands (observed)

`cargo fmt --all -- --check` first reported diffs in `acquisition.rs`; `cargo fmt
--all` was run, then the check exited 0. One transient MSVC `LNK1104` file-lock
hit the first full `cargo test -p mediahaven-core` immediately after clippy; a
straight retry exited 0 with no code change (pre-existing toolchain quirk).

| Check | Result |
|-------|--------|
| `cargo test -p mediahaven-core --lib acquisition::tests::source_search` (before implementation) | **RED**, exit 1 — `cannot find struct SourceSearchArgs`, `cannot find function source_search_with` |
| same targeted test after implementation | **GREEN**, exit 0 — 1 passed |
| `npm test -- --run src/components/cards/SourceResults.test.tsx` (before implementation) | **RED**, exit 1 — `Failed to resolve import ./SourceResults` |
| same targeted test after implementation | **GREEN**, exit 0 — 3 passed |
| `cargo test -p mediahaven-core` | **PASS**, exit 0 — lib 29 passed / 0 failed; auth 11; 0 doc |
| `cargo fmt --all -- --check` | **PASS**, exit 0 |
| `cargo clippy -p mediahaven-core -p mediahaven-desktop --all-targets -- -D warnings` | **PASS**, exit 0 |
| `cargo check -p mediahaven-desktop --all-targets` | **PASS**, exit 0 |
| `npm run typecheck` / `npm run lint` / `npm test -- --run` | **PASS**, exit 0 × 3 — 19 files / 56 tests passed |
| `npm run check:tokens` | **PASS**, exit 0 — 90 UI files scanned, 0 violations |
| `npm run build` | **PASS**, exit 0 — 104 modules transformed |
| `npm run check:bundle` | **PASS**, exit 0 — production bundle clean |
| `git diff --check` | **PASS**, exit 0 |

### Honest verification note

**No live Prowlarr seeding and no live Torznab search were run.** No operator
indexer instance or credentials were supplied; fake Torznab XML and fake HTTP
closures only. No real secret was stored, read or transmitted. S/E detection
uses simple string scanning of normalized release titles and may miss unusual
formats; all candidates remain unverified until the existing download gates
run. No checkpoint passed; no ADR was self-accepted (ADR-0029 records a human
instruction).

### Single next action

With an operator Prowlarr instance configured, open Settings → "Set up sources",
confirm the three native indexers appear, then open any title's Details page,
press "Find sources", and record whether relevant candidates appear and whether
choosing one passes the executable guard and probe before download. Record the
observed result here.

## 2026-11-22 — App startup on mount and Prowlarr inputs (plan Task 2; live runtime NOT RUN)

Implemented per `docs/superpowers/plans/2026-11-22-app-startup-and-inputs.md`
on worktree `ea5b46` (branch `tmdb-artwork-and-exe-guard`, base `8225102`, which
already carried plan Task 1). No new dependency, no push/merge/install;
Playwright config and `vite.config.ts` untouched. Full detail:
`.superpowers/sdd/2026-11-22-app-startup-and-inputs/task-2-report.md`.

- **Controller fixes.** `runtime_start` no longer calls `wait_ready()`; it
  stages the compiled-in compose file at an absolute temp path
  (`std::env::temp_dir().join("mediahaven-compose.yaml")`) and returns right
  after `ensure_running()` with one fixed string via `startup_status`. No path,
  distro, env value or engine detail is in the output. New tests assert the
  staged path is absolute and the file exists.
- **Secrets and environment.** One `account_for(kind)` allowlist maps `tmdb`,
  `prowlarr_url` and `prowlarr_key` to distinct DPAPI accounts; all three named
  commands use it. `build_wsl_env` forwards the two Prowlarr inputs only when
  present, with `WSLENV` entries, leaving the no-input case unchanged. Engine
  redaction now masks all four provider assignments. `core.environment`
  declares both variables with `:-` defaults; an assertion test guards it.
- **Settings.** Prowlarr address (text) and API key (password) fields mirror the
  TMDB field: Save/Clear, `Configured` / `Not configured`, fixed notices, drafts
  cleared on success, saved values never rendered. `providerSecrets.ts` uses a
  three-name union and a per-kind session-only dev shim. Production bundle scan
  found no dev shim keys or storage writes.
- **Docs.** ADR-0028 recorded **Accepted (2026-11-22, human instruction)** by
  the agent as scribe and indexed in `docs/decisions/README.md`; P6-10 added to
  `roadmap.md`; no checkpoint passed.

### Verified commands (observed)

Rust used `CARGO_TARGET_DIR=target/verify`. One transient `LNK1104` linker
file-lock occurred when `cargo test` ran immediately after clippy/check; a
straight retry exited 0 with no code change (pre-existing MSVC toolchain quirk).

| Check | Result |
|-------|--------|
| `cargo test -p mediahaven-desktop --lib` (before implementation) | **RED**, exit 1 — 20 compile errors on the new test seams (`account_for`, `build_wsl_env` arity, `PROWLARR_*_ENV`, `stage_compose`) |
| same command after implementation | **GREEN**, exit 0 — `52 passed; 0 failed` (was 46) |
| `cargo fmt --all -- --check` (after `cargo fmt --all`) | **PASS**, exit 0 |
| `cargo clippy -p mediahaven-desktop --all-targets -- -D warnings` | **PASS**, exit 0 |
| `cargo check -p mediahaven-desktop --all-targets` | **PASS**, exit 0 |
| `cargo test -p mediahaven-contract` (extra parity check) | **PASS**, exit 0 — 8 passed, 0 failed; contract stays 25 commands |
| targeted Vitest (providerSecrets + SettingsPage, before implementation) | **RED**, exit 1 — 4 failed / 4 passed |
| same targeted Vitest after implementation | **GREEN**, exit 0 — 8 passed |
| `npm run typecheck` / `npm run lint` / `npm test -- --run` | **PASS**, exit 0 × 3 — 17 files / 50 tests passed |
| `npm run check:tokens` | **PASS**, exit 0 — 85 UI files scanned, 0 violations |
| `npm run build` | **PASS**, exit 0 — 101 modules transformed |
| `npm run check:bundle` | **PASS**, exit 0 — production bundle clean; no dev shim key or storage write |
| `git diff --check` | **PASS**, exit 0 |

### Honest verification note

**No live WSL/Docker start, backend readiness check or Prowlarr connection was
run.** No engine consent and no operator Prowlarr instance were supplied; no
real secret was stored or read in any test. The compose staging test writes and
removes the compiled-in file under the temp directory; it does not invoke WSL.
Readiness-after-start through the UI's normal API calls is therefore **NOT RUN**
even though the non-blocking command and its fixed strings are unit-tested. No
checkpoint passed; no ADR was self-accepted (ADR-0028 records a human
instruction).

### Single next action

With explicit engine consent and an operator Prowlarr instance, open the app,
observe `runtime_start` return a fixed status, confirm the UI reaches the API
without the command blocking, supply all three values in Settings, restart and
confirm the forwarded variables are used; record the observed result here.

## 2026-11-22 — TMDB discovery catalogue (Tasks 1–3; live catalogue with token NOT RUN)

Implemented per `docs/superpowers/plans/2026-11-22-tmdb-discovery-catalogue.md`
on worktree `ea5b46` (branch `tmdb-artwork-and-exe-guard`, Task-3 base
`11621c6`). No new dependency, no push/merge/install; Playwright config and
`vite.config.ts` untouched. Task 3 frontend detail:
`.superpowers/sdd/2026-11-22-tmdb-discovery-catalogue/task-3-report.md`.

- **Tasks 1–2 (already landed; unchanged here).** ADR-0027, the display-only
  `DiscoverProvider`, `discover_catalog` across contract/route/Tauri and the
  generated command union, and poster caching. Their Rust gates are recorded
  in the Task-1/Task-2 reports and were **not re-run** by Task 3.
- **Task 3 (frontend).** `types.ts` gained `CatalogKind`/`CatalogEntry`/
  `CatalogScreen` (camelCase, matching Rust); `api.discover(kind, page = 1)`
  calls the named command. New display-only `CatalogShelf` renders posters with
  an `apiBase` prefix exactly once, a guarded `onError` that hides a failed
  image, title/year/rating and the exact TMDB attribution line; `available:
  false` shows a quiet "Add a TMDB token in Settings" message with a Settings
  link and no cards. Movies/Shows/Anime gained a per-visit My library /
  Discover toggle; `BrowseView` itself is unchanged and library mode remains
  the default. Anime Discover states it uses TMDB's Japanese-animation filter
  and may miss anime; its local Anime tag filter is unchanged.

### Verified commands (observed)

| Check | Result |
|-------|--------|
| `npm test -- --run src/components/catalog/CatalogShelf.test.tsx` (before implementation) | **RED**, exit 1 — `Failed to resolve import "./CatalogShelf"`, `Test Files 1 failed`, `Tests no tests` |
| same targeted test after implementation | **GREEN**, exit 0 — `Test Files 1 passed`, `Tests 4 passed` |
| `npm run typecheck` | **PASS**, exit 0 |
| `npm run lint` | **PASS**, exit 0 (after moving the loading reset out of the effect into the mode-change handler) |
| `npm test -- --run` | **PASS**, exit 0 — `Test Files 16 passed`, `Tests 45 passed` |
| `npm run check:tokens` | **PASS**, exit 0 — `83 UI files scanned, 0 violations` |
| `npm run build` | **PASS**, exit 0 — `100 modules transformed` |
| `npm run check:bundle` | **PASS**, exit 0 — production bundle clean |

### Honest verification note

**A live catalogue run with a real token is NOT RUN.** No operator token was
supplied, no live TMDB request was made and no live poster was observed in a
WebView. Every catalogue test uses a fake `CatalogScreen`; no real secret was
read. Anime Discover is an approximation (TMDB Animation genre + Japanese
original language) and may miss anime. Rust gates for Tasks 1–2 were not
re-run. No checkpoint passed.

### Single next action

An operator supplies a token in Settings, restarts the app so `core` receives
`MEDIAHAVEN_TMDB_API_KEY`, then opens Discover on Movies and observes one live
TMDB poster plus the attribution line; record the result here.

## 2026-11-22 — Operator-supplied TMDB token (Tasks 1–3; live token run NOT RUN)

Implemented per `docs/superpowers/plans/2026-11-21-operator-tmdb-token.md` on
the isolated worktree `ea5b46` (branch `tmdb-artwork-and-exe-guard`, Task-1
base `d85d332`, Task-3 base `d503b66`). No new dependency, no push/merge/install;
Playwright config and `vite.config.ts` untouched. Full detail:
`.superpowers/sdd/2026-11-21-operator-tmdb-token/task-{1,2,3}-report.md`.

- **Tasks 1–2 (already landed before this entry; unchanged here).** Generic
  DPAPI secret helpers, named commands `provider_secret_set/has/clear` with a
  `kind == "tmdb"` allowlist, supervisor injection of
  `MEDIAHAVEN_TMDB_API_KEY`, and `core` TMDB → IMDb → bundled fallback. Their
  Rust gates are recorded PASS in `task-1-report.md` and `task-2-report.md`;
  they were **not re-run** by Task 3.
- **Task 3 (frontend).** New `src/services/providerSecrets.ts` reuses the repo's
  actual Tauri accessor (`call()` → `window.__TAURI_INTERNALS__.invoke`), not a
  made-up `window.__TAURI__` shape. Browser dev has a session-only shim; the
  production build tree-shakes every `sessionStorage` write out (verified in
  `dist/assets`). Settings gained a password field (`aria-label="TMDB read
  access token"`) with Save token / Clear token, a `Configured` / `Not
  configured` status, a restart notice, and text naming what is sent
  (title/year/poster path) and the IMDb fallback. `TmdbAttribution.tsx` keeps
  its exported names, adds `IMDB_POSTER_ATTRIBUTION` and renders both exact
  lines in one caption. No stored value is rendered, stored in the renderer,
  or echoed in an error.

### Verified commands (observed)

| Check | Result |
|-------|--------|
| `npm test -- --run src/pages/Settings/SettingsPage.test.tsx` (before implementation) | **RED**, exit 1 — `Test Files 1 failed`, `Tests 1 failed / 1 passed`; `TestingLibraryElementError: Unable to find a label with the text of: TMDB read access token` |
| targeted tests after implementation (Settings + attribution + providerSecrets) | **GREEN**, exit 0 — `Test Files 3 passed`, `Tests 6 passed` |
| `npm run typecheck` | **PASS**, exit 0 |
| `npm run lint` | **PASS**, exit 0 |
| `npm test -- --run` | **PASS**, exit 0 — `Test Files 15 passed`, `Tests 41 passed` |
| `npm run check:tokens` | **PASS**, exit 0 — 80 UI files scanned, 0 violations |
| `npm run build` | **PASS**, exit 0 — 98 modules transformed |
| `npm run check:bundle` | **PASS**, exit 0 — production bundle clean |
| production `dist/assets/*.js` string scan | **PASS** — no `mediahaven.dev.tmdb-token`, `sessionStorage.setItem` or `sessionStorage.removeItem`; `provider_secret_set` and both attribution lines present |

### Honest verification note

**A live token run is NOT RUN.** No operator token was supplied, no live TMDB
request was made, and no live TMDB poster was observed in a WebView. Every test
uses a mocked Tauri bridge or a session-only dev shim; no real secret was
stored in DPAPI. Rust gates for Tasks 1–2 were not re-run in this entry. No
checkpoint passed.

### Single next action

An operator supplies a token in Settings, restarts the app so `core` receives
`MEDIAHAVEN_TMDB_API_KEY`, and one live TMDB poster is observed; record the
observed result here. Until then P6-08 stays `[~]`.

## 2026-11-21 — Keyless IMDb posters (Tasks 1–2; full-library live seed NOT RUN)

Implemented per `docs/superpowers/plans/2026-11-21-imdb-keyless-posters.md` on
the isolated worktree `ea5b46` (branch `tmdb-artwork-and-exe-guard`, base
`efd5e73`). No key, no Python runtime, no HTML scraping, no new dependency, no
push/merge/install. Full detail: `.superpowers/sdd/2026-11-21-imdb-keyless-posters/task-{1,2}-report.md`.

- **Task 1 (ADR-0025 + keyless provider).** ADR-0025 is **Accepted
  (2026-11-21, human-approved)** and indexes in `docs/decisions/README.md`.
  `ImdbArtworkProvider` returns an image URL from IMDb's keyless suggestion
  JSON only after the `https://m.media-amazon.com/images/M/` allowlist passes;
  `fetch_image_bytes` re-checks the allowlist, the 2 MiB cap and real
  JPEG/PNG headers. Fake-fetch tests, no network.
- **Task 2 (core wiring + attribution).** `AppState` now holds
  `imdb_artwork: Arc<ImdbArtworkProvider>` with the `with_imdb_artwork(...)` test
  seam; `TMDB_OPTIN_ENV`, `tmdb`, `with_tmdb` and `tmdb_provider_from_env` are
  removed from `core` (the TMDB crate code stays, unwired). `fill_poster` uses
  `poster_url` — never the unvalidated `imdb_search_fetch` — and the existing
  `artwork_key`/`ArtworkCache`. The 12-item cap, pre-join 64-lowercase-hex
  validation, relative `/artwork/<hash>` DTO URL, host-free DTO assertion and
  frontend `onError` fallback are unchanged. Attribution is exactly
  "Posters: IMDb" (component name and import sites kept). The stale
  `crates/metadata` crate doc was corrected. No checkpoint passed.

### Verified commands (observed)

All Rust commands used `CARGO_TARGET_DIR=target/verify`.

| Check | Result |
|-------|--------|
| `cargo test -p mediahaven-core state::tests::imdb_artwork_provider_is_available_without_a_key` | **RED** exit 101 (`error[E0609]: no field imdb_artwork`), then **GREEN** exit 0 (`1 passed; 0 failed`) |
| `cargo fmt --all -- --check` (after `cargo fmt --all`) | **PASS**, exit 0 |
| `cargo clippy -p mediahaven-core -p media-metadata --all-targets -- -D warnings` | **PASS**, exit 0 |
| `cargo test -p mediahaven-core -p media-metadata` | **PASS**, exit 0 — metadata 49 passed, core 25 passed, 0 failed; route/failure tests `imdb_artwork_adds_only_a_relative_hashed_url`, `imdb_artwork_failure_adds_no_artwork`, `enrichment_is_bounded_but_returns_every_item`, `artwork_route_rejects_non_hash_keys` all ok |
| `cargo test -p mediahaven-contract` (extra parity check) | **PASS**, exit 0 — 8 passed, 0 failed; `command_lists_derive_from_one_source ... ok` |
| `npm run typecheck` / `npm run lint` / `npm test -- --run` / `npm run check:tokens` / `npm run build` / `npm run check:bundle` (from `apps/desktop`) | **PASS**, exit 0 × 6 — Vitest 13 files / 36 tests; tokens 77 files / 0 violations; build 97 modules; bundle clean |

### Honest verification note

**A full-library live IMDb seed is NOT RUN.** Every Rust test used a fake fetch
returning an allowlisted URL plus a pre-seeded JPEG byte file; no live
suggestion request, no live image download and no real library title were
exercised. No poster was observed rendering in a real WebView, and no such
claim is made. ADR-0025 is **Accepted (2026-11-21, human-approved)**. No
checkpoint passed.

### Single next action

Run one real lookup against a real library title and confirm the cached poster
renders in the WebView; record the observed result here. Until then P6-07 stays
`[~]`.

## 2026-11-21 — TMDB artwork (Tasks 1–3, opt-in; live call NOT RUN)

Implemented per `docs/superpowers/plans/2026-11-21-tmdb-artwork.md` on the
isolated worktree at `5aed36e`. Opt-in TMDB posters, off by default: fake-fetch
Rust tests and loopback-only frontend tests; no scraper, no new npm dependency,
no path/key in renderer DTOs, Playwright config and `vite.config.ts` untouched.
Full detail and evidence: `.superpowers/sdd/2026-11-21-tmdb-artwork/task-{1,2,3}-report.md`.

- **Task 1 (ADR-0023 + transport + hashed cache).** ADR-0023 is
  **Accepted (2026-11-21, human-approved)**. `cargo test -p media-metadata --test tmdb`
  RED then GREEN (`test result: ok. 12 passed; 0 failed`); `cargo fmt --all` exit 0;
  `cargo clippy -p media-metadata --all-targets -- -D warnings` exit 0;
  `cargo test -p media-metadata` exit 0. Fakes only.
- **Task 2 (core wiring).** `cargo test -p mediahaven-core
  state::tests::default_state_has_a_disabled_tmdb_provider` RED then
  `routes::tests::artwork_route_rejects_non_hash_keys` RED, both GREEN after;
  `cargo fmt --all -- --check` exit 0; `cargo clippy -p mediahaven-core
  -p media-metadata --all-targets -- -D warnings` exit 0; `cargo test
  -p mediahaven-core` (`13 passed; 0 failed` unit + `11 passed; 0 failed` route
  tests); `cargo test -p mediahaven-contract` (`4 passed; 0 failed`, 24-command
  route parity intact). Public `GET /artwork/:key` validates a 64-lowercase-hex
  key before any path join; DTOs carry only `/artwork/<64-hex>`.
- **Task 3 (frontend).** `artFromDto` RED (`4 failed`, `artFromDto is not a
  function`) then GREEN (`4 passed`) via `npm test -- --run
  src/services/adapt.artwork.test.ts`. The strict regex
  `^\/artwork\/[0-9a-f]{64}$` accepts only Task 2's hashed relative URLs and
  falls back to bundled art for `javascript:`, absolute third-party URLs,
  uppercase/short/wrong-shape paths, empty or undefined artwork. Attribution
  renders unconditionally in the Hero and gated on raw DTO artwork in Details,
  reusing the token-only `.attribution` style from `imdbRating.module.css`.
  CSP `img-src` widened by exactly one origin (`http://127.0.0.1:8080`, matching
  `connect-src`); Settings privacy text states what is sent. Full frontend gates
  (typecheck / lint / test / check:tokens / build / check:bundle) and the final
  Rust gates for this task are recorded verbatim in `task-3-report.md`.

### Honest verification note

**Live TMDB search and live image download are NOT RUN** — no API key was
supplied and `MEDIAHAVEN_TMDB_OPTIN` was not set. Every verification used fakes,
pure helpers and loopback only. No poster was observed in a real WebView with a
real key; no such claim is made. ADR-0023 is **Accepted (2026-11-21,
human-approved)**. No checkpoint passed.

### Single next action

An operator supplies a TMDB key from the ADR-0007 secret store and sets
`MEDIAHAVEN_TMDB_OPTIN=1`, then a single live call is run and its result recorded
here (search + one cached poster). Until then this task stays `[~]`.

## 2026-11-20 — Final review fix wave (consolidated findings 1–10)

Reviewed whole-branch findings and fixed every listed item. No checkpoint
passed; no commit/push/install/host/container change; Playwright and Vite config
untouched; no new dependency; all Rust verification reused `CARGO_TARGET_DIR=target/verify`.

- **F1 — persistence gap closed.** The host `/host/acquisition_add` route now
  persists each acquired item through the existing `LibraryService::download_add`
  port; `download_add` also INSERTs the `file_mappings` row (hash, host-internal
  container path, nullable media_id) in the same transaction. Cleanup can now
  see `app_managed=true`. Renderer DTOs remain path-free (`DownloadItem` carries
  no path; a persistence test asserts no `filePath`/`/downloads/` in its JSON).
- **F2 — no file deletion in `core`.** Removed `std::fs::remove_file` from the
  cleanup worker. `core` has only `:ro /downloads`; deletion is delegated to qB
  via `TorrentApi::remove(hash, true)` (rw bind). `exists` is a read-only
  post-condition probe. KeepFiles keeps every file (qB task-only removal).
- **F3 — no false success / AC-10.** The `needed` set now excludes the target
  media_id (its state flips to Watched only after success). `reconcile` requires
  a positive verified deletion count for `Removed`; a zero-file result maps to
  `failed` ("no files deleted; kept for still-needed episodes"). Added two
  integration tests through the production reconciliation path (real SQLite,
  fake qB, no injected predicate): a genuinely app-managed, grace-elapsed,
  confirmed-complete entry is really deleted and reported `removed`; a
  zero-removal (still-needed sibling) case is reported `failed` and deletes
  nothing. Both PASS.
- **F4 — honest matrix.** `docs/testing/acquisition-matrix.md` criteria 8–10
  downgraded to **PASS (pure policy) / NOT RUN (live wiring)**.
- **F5 — no tokio blocking.** Prefetch enqueue runs in `spawn_blocking`
  (10 s qB timeouts no longer block the worker; std/tokio only).
- **F6 — live-session gate release.** `playback_start` now calls the backend
  `playback_stop` if host mpv/source resolution fails after the backend marked
  the media as the live session, so cleanup is not blocked until restart.

### Verification (observed in this wave)

| Check | Result |
|-------|--------|
| `cargo fmt --all -- --check` | **PASS**, exit 0 |
| `cargo clippy -p media-acquisition -p media-persistence -p mediahaven-core -p mediahaven-contract -p mediahaven-desktop --all-targets -- -D warnings` | **PASS**, exit 0 |
| `cargo test` (same five packages) | **PASS**, exit 0 — 122 passed, 0 failed, 1 ignored; 24-command route parity `command_lists_derive_from_one_source ... ok` |
| Frontend `typecheck` / `lint` / `test` / `check:tokens` / `build` / `check:bundle` | **PASS**, exit 0 × 6; Vitest 6 files / 18 tests; tokens 66 files / 0 violations; build 93 modules |
| `git diff --check` | **PASS**, exit 0 (one pre-existing CRLF warning on untouched `Cargo.toml`) |

Live qB deletion, real-media appearance/playback and confirmed completion
remain **NOT RUN** (no lawful source; mpv engine absent). Full evidence and the
full text of changed untracked sources:
`.superpowers/sdd/2026-11-20-torrent-acquisition-conversion/final-fix-report.md`.

### Gate and next action

No checkpoint passed. CP-5/CP-6 gain evidence but stay OPEN pending human
review. Next safe action: supply a lawful source and a human-reviewed
per-episode mapping, then run the live acquire → play → prefetch → cleanup
chain and obtain human CP sign-off.

## 2026-11-20 — Torrent-acquisition conversion complete (Tasks 1–7)

ADRs 0017 (conversion / supersede local-playback-first), 0018 (qBittorrent
boundary) and 0019 (app-managed cleanup only) were **Accepted by the human
partner on 2026-11-20** and recorded by the controller as scribe; no ADR was
self-accepted. Implemented against the target spec
`media_haven_consolidated_specification.md` per
`docs/superpowers/plans/2026-11-20-torrent-acquisition-conversion.md`.

### What shipped, by task

- **Task 1 — acquisition records.** Migration `0004_acquisition.sql`
  (`torrent_tasks`, `download_items`, `file_mappings`, `prefetch_entries`,
  `cleanup_entries`); path-free camelCase domain types; `LibraryRepository`
  port methods and transactional SQLite implementation; multi-episode mapping
  test. Fixed the legacy `DownloadTask.state` wire contract (snake_case restored)
  so the renderer union did not drift.
- **Task 2 — qB sandbox + download store.** `qbittorrent` service on an egress
  `acquire` bridge, no published port, `cap_drop ALL`, `no-new-privileges`, UID
  10003; host-bound `/downloads:rw` into qB and `/downloads:ro` into `core`,
  `/downloads` added to the host path map for mpv. Pre-chowning
  `Dockerfile.qbittorrent` cleared the engine's exit-134 liveness blocker.
- **Task 3 — loopback WebUI client.** Std-only `QbClient` (`TcpStream` +
  `serde_json`, no new dependency) with mandatory login (PBKDF2 config generated
  from the injected token), cookie auth, form encoding, non-2xx → `QbError::Api`;
  fake-server tests; single shared host download store; cross-container WebUI
  reachability and read-only bind proven live.
- **Task 4 — acquisition flow + UI.** Source validation at the trust boundary
  (magnet / bare `.torrent` only, no paths), `download_exists` duplicate
  suppression, named `download_add/status/remove` commands routed as signed
  `/host/` core calls, real Downloads rows + Add/Remove dialog and Settings
  acquisition-root control (`AcquisitionRow` path-free). Live signed core→qB
  add/status/remove returned HTTP 200.
- **Task 5 — rolling two-episode buffer.** Pure per-series ordering (next two in
  season, stop at season/final boundary), skip-existing, 90%-of-20-GiB pause
  (`bufferNotice`, message string only) shared between enqueue decision and UI;
  prefetch triggered from `playback_start` after policy checks and never fails
  the playback session.
- **Task 6 — completion → coordinated cleanup (release blocker CLEARED).**
  B5-confirmed completion (≥90% watched + near EOF, seeking/close never confirm),
  24 h replay grace, not-playing and not-prefetch exclusions; cleanup confined to
  app-managed `file_mappings` inside `/downloads` with a zero-deletion file-count
  proof; NULL-`media_id` siblings fail closed as still-needed; multi-episode
  `remove(hash, false)` when a sibling is needed; reconciliation to `removed` or
  `failed` with a visible outcome; live playback session registry and 1 h
  scheduler wired through signed `/host/` routes.

### Verification (observed; this documentation task did not re-run the suites)

| Check | Result |
|-------|--------|
| `cargo fmt --all -- --check` | **PASS**, exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | **PASS**, exit 0 |
| `cargo test --workspace` | **PASS**, exit 0 — **84 passed, 0 failed, 1 ignored** (`searching_10k_items_is_within_budget`); 24-command parity `command_lists_derive_from_one_source ... ok` |
| `cargo test -p mediahaven-desktop` | **PASS**, exit 0 — 36 passed, 0 failed |
| Task 6 cleanup suite | **PASS** — 14/14 incl. zero-deletion file-count release blocker |
| Frontend `typecheck` / `lint` / `test` / `check:tokens` / `build` / `check:bundle` | **PASS** — exit 0 × 6; Vitest 6 files / 18 tests; tokens 66 files, 0 violations |
| `docker compose config --quiet` (all vars / no vars) | **PASS** — exit 0 / exit 1 (fail-closed) |
| Live qB boundary + shared `/downloads` store + WebUI auth (login 204 / authed 200 / unauth 403) | **PASS** (Task 2/3 evidence) |
| `git diff --check` | **PASS**, exit 0 |
| Specification acceptance matrix | `docs/testing/acquisition-matrix.md` — 12 criteria mapped (PASS, PASS-mechanism/NOT-RUN-live, and NOT RUN stated honestly) |

Two transient `LNK1104` linker file-locks occurred in the default target dir
during earlier task runs; a straight retry exited 0 with no code change (not a
test failure). All cargo verification reused the single `target/verify` dir.

### Deferrals / NOT RUN (honest gates)

- **Live authorized-torrent acquisition, real-media appearance/playback and live
  cleanup are NOT RUN** — no lawful source supplied and ADR-0018 forbids bundling
  sources. Real mpv playback also NOT RUN (engine absent, `#[ignore]` test).
  Acceptance criteria 1, 3, 5, 8, 10 and 12 are reported accordingly in the
  acquisition matrix; no PASS was invented.
- **Per-episode source mapping** (`source_for` stub → `None`) is a §15 gate; live
  prefetch is a deliberate no-op, not a failure.
- **Used-byte accounting** still returns 0, so `bufferNotice` is `null` in
  production until real retained-byte accounting lands (non-null path unit-tested).
- **Frontend cleanup-status rendering**, **scheduler persistence across `core`
  restart**, **`.torrent` byte upload / bounded hash-index poll**, and
  **second-machine LAN non-reachability** (human evidence gate) remain open.
- No commit, push, install, host-feature or container-runtime configuration
  change was made for the conversion; Playwright config and `vite.config.ts`
  were untouched.

### Gates

- **No checkpoint passed.** CP-5/CP-6 gain evidence (data-loss exclusions, qB
  boundary) but remain OPEN pending human review; CP-0…CP-4 and CP-7 unchanged.
- Next safe action: supply a lawful source and a human-reviewed per-episode
  mapping, then run the live acquisition → play → prefetch → cleanup chain; and
  obtain human CP review. Do not mark any checkpoint passed by agent action.

---

## 2026-11-20 — Playback host wiring (ADR-0015 executable evidence)

ADR-0015 is **Accepted** (2026-11-20, ADR-0016); code now depends on it. The
renderer is driven by real mpv engine state when the Tauri host is present;
the `SIMULATION_RATE = 4` clock remains **only** as a browser/dev fallback.

### What changed

- `GET /playback_source/:media_id` (signed, authenticated) returns the stored
  container-path identity via a new `LibraryRepository::media_source_path` port,
  service passthrough and SQLite lookup. It is a GET so the 24-command
  POST-only route-parity surface is unchanged; no `COMMANDS` entry was added.
- New host module `apps/desktop/src-tauri/src/player.rs`: `resolve_for_media`
  (A9 map + `media_playback::resolve_source`, fails closed), a managed
  `PlayerState` owning one `MpvPlayer`, a 500 ms `player://state` poller emitting
  `{timePos, pause, volume, mediaId}` only, and named `player_control` controls.
- `playback_start` now resolves host-side and opens mpv; resolution failure
  returns a fixed `source unavailable` `CommandError`. `playback_stop` stops the
  player (drop kills the child). `player_control` was registered in
  `generate_handler!`.
- New renderer bridge `src/services/player.ts`; `PlayerPage.tsx` follows engine
  `timePos`/`pause`, routes seek/play/pause/volume through `player_control`, and
  keeps checkpoint saves. New headless test mocks the event stream.
- `crates/playback/src/mpv.rs` gained an additive `set_volume` inherent method
  (+ one fake-Ipc test) so the named volume control is real. The `Player` port
  and the 12 existing playback tests were otherwise untouched.
- `apps/desktop/src-tauri/Cargo.toml` gained the existing workspace crate as a
  path dependency (`media-playback`); no external dependency was added.

### Verification

Observed with a fresh `CARGO_TARGET_DIR=target/wiring-test` to work around the
known default-target `LNK1104` file lock.

| Check | Result | Notes |
|---|---|---|
| `cargo fmt --all` then `cargo fmt --all -- --check` | **PASS** exit 0 | |
| `cargo clippy --workspace --all-targets -- -D warnings` | **PASS** exit 0 | |
| `cargo test --workspace` | **PASS** exit 0 | 136 passed, 2 ignored (10k search budget + live mpv smoke) |
| 24-command route parity | **PASS** | `command_lists_derive_from_one_source ... ok`; unchanged contract |
| `npm run typecheck` | **PASS** exit 0 | |
| `npm run lint` | **PASS** exit 0 | |
| `npx vitest run` | **PASS** exit 0 | 13/13 (was 11; +2 PlayerPage engine-state tests) |
| `npm run check:tokens` | **PASS** exit 0 | 63 UI files, 0 violations |
| `npm run build` | **PASS** exit 0 | 92 modules |
| `npm run check:bundle` | **PASS** exit 0 | no dev token / signed client in production bundle |

### NOT RUN / remaining risk

- Live mpv playback, codec matrix, hardware decode and the `player://state`
  emission through a real WebView are **NOT RUN**: mpv is absent and installing
  it needs human consent; the live test stays `#[ignore]`. No real file was
  opened by a real engine.
- The signed `GET /playback_source/:media_id` was not exercised against a live
  `core` container (engine consent required).
- No checkpoint was passed. **CP-3 remains OPEN.**
- Evidence detail: `docs/security/boundary-evidence.md`; implementation report:
  `.superpowers/sdd/2026-11-20-playback-wiring/report.md`.

## Current state

The repository holds the approved engineering contract, the checkpoint-driven
roadmap, the documentation baseline, a working React frontend, a working Rust
backend, a Tauri host and a CI workflow. No checkpoint has passed: CP-0 evidence
is still unmeasured and every later checkpoint is open.

Built and verified through HEAD `342addf` (latest code commit `9b706fb`): React
TV UI wired to the host through named Tauri commands, with full arrow/Enter/Escape
navigation; a Rust workspace of seven crates including `app-core`, `core`,
`media-library`, `metadata`, `persistence`, `contract` and `mediahaven-desktop`;
SQLite migrations, scanner, watchlist, durable queue, settings/profiles; DPAPI
per-install token, WSL supervisor and HMAC-signed command forwarder; CI with
contract-drift and dependency-graph assertions. Later waves added authorized-media
torrent acquisition (ADR-0017/0018/0019), fail-closed probe + Defender gates,
R-01 per-item source mapping (ADR-0021), offline IMDb metadata (ADR-0022 Proposed)
and an unsigned per-user NSIS installer.

Written but NOT RUN on a live engine: the `core`/`probe` images, the rendered
Compose override, mount scope, token forwarding into `core`, boundary evidence
and all container-based checkpoints. Also NOT RUN: live acquisition against a
lawful source, real-media playback/cleanup, live IMDb/OMDb/TMDB requests, clean-VM
install and every checkpoint sign-off. These need operator data, engine consent
or human review.

Superseded 2026-11-20/21: playback is no longer only simulated — the Tauri host
wires `crates/playback`/mpv and the renderer follows engine state; simulation is
a dev-only fallback. The earlier claim that "TMDB provider, artwork cache,
diagnostics bundle, safe mode, traversal CI suite, storage policy and installer
are not built" is **wrong**: the opt-in TMDB provider, diagnostics, storage
policy and installer now exist; only the TMDB *artwork transport/cache* is
unbuilt (P6-07, plan unexecuted). (The original 2026-11-19 text said there was
no playback crate or mpv adapter; that was true at that date.)

Superseded: earlier text in this file claimed "the backend does not yet exist",
"No Rust workspace, no database" and "src-tauri is still empty". Those claims
were true only for the 2026-11-15 baseline and were corrected on 2026-11-18 and
again on 2026-11-19.

## Active task

- **Task:** Documentation reconciliation (2026-11-21) — no product feature is
  active. ADR-0022 (IMDb) and ADR-0023 (TMDB artwork transport) await human
  acceptance; engine and live-source gates await operator consent.
- **Branch/HEAD:** `master`, HEAD `342addf`, fully pushed to `origin/master`.
- **Six newest commits** (`git log -6 --oneline`, verbatim):
  - `342addf` docs: add TMDB artwork implementation plan
  - `9b706fb` feat(a11y): full arrow/Enter/Escape keyboard navigation
  - `7ad7d47` chore: ignore live dev server logs
  - `806ffd9` chore: ignore live demo data directory
  - `4082920` feat: R-01 source mapping, offline IMDb ratings, Windows installer
  - `0db2cc1` feat(security): fail-closed probe + Defender gates for downloaded media

## Completed this session

### Frontend (`apps/desktop`)

- Scaffolded Vite + React 19 + TypeScript + React Router 7 + Vitest + Testing
  Library + ESLint 9 (flat config). 278 packages installed; no Rust or Tauri
  shell.
- Design tokens in `src/styles/tokens.css` — the only place a literal may live.
  Plus `global.css` and `motion.css`. `npm run check:tokens` enforces it.
- A frozen typed command contract in `src/types/media.ts` (DTOs, command
  surface, errors, helpers) — the single source of truth.
- A transport seam (`src/services/transport.ts`) that picks the mock in
  development and the Tauri host in production; the mock is excluded from the
  production bundle (verified by the build).
- A named command layer (`src/services/commands.ts`) — no generic passthrough
  (ADR-0005).
- A development mock returning a deterministic fixture library
  (`src/services/mock/`), with abstract, stable cover art and every state the
  UI must handle.
- `FocusManager` with the shelf navigation model, overlay stack and focus
  restoration — the highest-regression area in the app.
- Primitives: `Artwork`, `MediaCard`, `EpisodeCard`, `CollectionCard`,
  `ProgressBar`, `StatusPill`, `SkeletonCard`, `MediaShelf`, `HeroFeature`.
- `MediaDetails` overlay (seasons, episodes, play/queue/watchlist) and the
  `Player` surface with controls, scrubber, subtitle/audio selects, idle fade
  and up-next.
- Pages: `Home`, `Movies`, `Shows`, `Library`, `Watchlist`, `Downloads`,
  `Search`, `Settings`, `Setup`. Every page has loading, empty, error and
  offline states.
- A focus-navigation test suite (8 tests) covering horizontal movement, edge
  clamping, vertical movement with column preservation, unequal-shelf clamping,
  and overlay focus restoration.

### Verified this session

| Check | Result | Notes |
|---|---|---|
| TypeScript (`tsc --noEmit`) | **PASS** | Strict mode, including `noUncheckedIndexedAccess` |
| ESLint | **PASS** | `--max-warnings 0`, flat config |
| Vitest | **PASS** | 8/8 |
| Token discipline (`check-tokens.mjs`) | **PASS** | 48 UI files, 0 violations |
| Vite build | **PASS** | 80 modules, 318 KB JS / 36 KB CSS (gzipped 98 KB / 6.5 KB) |
| Dev server | **PASS** | `http://localhost:5173/` — Home renders with nav rail, hero and shelves, no console errors |
| Mock excluded from production bundle | **PASS** | `createMockTransport` is not present in `dist/` |

### Not verified

- Contrast ratios (no automated check wired yet — P4-01).
- Dropped-frame numbers (no profiling run — P4-05).
- A keyboard-only walkthrough of every screen (P4-03).
- Any data-dependent acceptance criterion (P4-08 — needs the backend).

## Current implementation facts

### Repository state
- Frontend: `apps/desktop` — Vite 7 + React 19 + TypeScript 5.9 + React Router 7,
  Vitest 3 + Testing Library, ESLint 9 flat config. 278 packages in
  `node_modules` (gitignored).
- `apps/desktop/src-tauri/` hosts the Tauri 2 shell (`mediahaven-desktop`) with
  the DPAPI per-install token, WSL supervisor and 24 named commands.
- Seven-crate Rust workspace, SQLite persistence and container build files are
  present. (Superseded 2026-11-19: the earlier claims that src-tauri was empty
  and that there was no Rust workspace, database or container images described
  only the 2026-11-15 baseline.)

### Decisions in force
- Desktop shell: Tauri 2 (scaffolded 2026-11-18). Backend: Rust. Frontend: React + TypeScript (Vite).
- Persistence: SQLite with migrations. Local-first, no mandatory accounts.
- **Docker/Compose is mandatory for v1.** The backend and the media-inspection
  workload run in containers. There is no host-side backend fallback.
- The Windows runtime distribution is **Accepted (provisional)** 2026-11-18:
  Docker Engine in a dedicated WSL2 distro (ADR-0001 Part 2). P0 evidence is still owed.
- **Playback runs on the Windows host** — an intentional, documented exception
  (ADR-0002). The backend stays the authority on what to play.
- **Frontend-first delivery** (ADR-0009): the interface is built against a frozen
  contract with a development-only mock. The mock implements no policy.
- One product surface. Internal containers are an implementation detail.
- MVP is direct HDMI playback on one Windows laptop. Remote streaming deferred.
- No AI runtime dependency after setup.
- Automatic permanent deletion is OFF by default.

### Reference machine (observed, read-only, 2026-11-15)
- Windows 11 Pro build 26300, 16 GB RAM.
- Intel Core i7-13700H (14C/20T), Intel Iris Xe (Xe-LP).
- WSL2 present; default distribution `OmniRushBox` (stopped); `docker-desktop`
  distro running.
- Docker Engine 29.9.3, kernel `6.18.58.1-microsoft-standard-WSL2`, cgroup v2,
  storage driver `overlayfs`.
- `docker info` reports `SecurityOptions: [name=seccomp,profile=builtin
  name=cgroupns]` — AppArmor absent, seccomp and cgroup namespaces present.
- WSL2 VM memory ceiling observed at 8 073 883 648 bytes (8 GB), 20 CPUs visible.
- 12 unrelated containers already running on the shared engine.

## Known working commands

- Frontend dev server: `cd apps/desktop && npm run dev` (port 5173)
- Frontend: `npm run typecheck`, `npm run lint`, `npm test`, `npm run check:tokens`,
  `npm run check:bundle`, `npm run build`
- Rust: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`
- Dependency graph (read-only): `bash tests/dependency_graph.sh`
- Compose parse (no daemon): `docker compose config --quiet` — exits 1 with four
  "required variable" errors when the token/digest env is absent (fail-closed)
- Preflight (read-only): `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/windows/preflight.ps1`
- Dev backend: `MEDIAHAVEN_DEV=1 MEDIAHAVEN_SEED=1 MEDIAHAVEN_DATA_DIR="$PWD/.mediahaven-dev" cargo run -p mediahaven-core`
- NOT available yet: any live `docker build` / `docker compose up` / installer command.

## Verification

Live results observed at HEAD `7d48507` on 2026-11-19 (exit code 0 for every
command):

| Check | Result | Notes |
|---|---|---|
| `git status --short` | **PASS** | Empty (clean tree) at the start of the run |
| `cargo fmt --all -- --check` | **PASS** | exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | **PASS** | exit 0 |
| `cargo test --workspace` | **PASS** | 107 passed, 1 ignored (pre-existing `searching_10k_items_is_within_budget`) |
| Frontend typecheck (`npm run typecheck`) | **PASS** | exit 0 |
| Frontend lint (`npm run lint`) | **PASS** | exit 0 |
| Vitest (`npx vitest run`) | **PASS** | 11/11 across `App.test.tsx` (2), `transport.test.ts` (4), `QueuePage.test.tsx` (5) |
| Token check (`npm run check:tokens`) | **PASS** | 61 files, 0 violations |
| Build (`npm run build`) | **PASS** | 91 modules, exit 0 |
| Engine-backed evidence (Docker/WSL2) | **NOT RUN** | Needs explicit engine consent |
| Hardware playback matrix | **NOT RUN** | No media engine / hardware run |
| TV-distance review | **NOT RUN** | Not performed |
| GitHub Actions green run | **NOT RUN** | CI workflow added; no green run observed |
| Installer lifecycle | **NOT RUN** | Installer not built |
| CP-0 evidence items | **NOT RUN** | 0 of the CP-0 criteria satisfied; no checkpoint passed |

## Blockers and failures

- **No blocker to the local build/test gates.** All local gates are green at HEAD
  `7d48507` (see Verification).
- **Engine consent is required** for any Docker/WSL2 build, `docker compose up`,
  digest recording, `docker inspect`, probe smoke or live token forwarding. None
  of these has been run.
- **ADR-0015 (playback source token, B1) is Proposed** and must be accepted by the
  human partner before playback B2/B3 work starts.
- **O-08 / package ID** and the remaining ADR acceptances are open human
  decisions. An agent may not accept an ADR or pass a checkpoint.
- **No checkpoint has passed.** CP-0 evidence is still unmeasured; CP-1..CP-7 are
  open.

## Known risks / regressions

- None in the frontend's verified checks.
- **Unreviewed-architecture risk:** ADRs 0002, 0005, 0006, 0007, 0008 and 0009 are
  all **Proposed**. An agent may write them and recommend; only the human partner
  may accept them.
- **Tooling risk:** `.agents/skills` and `.claude/` are gitignored, so a fresh
  clone will not have the skills or the design hooks. `README.md` documents the
  reinstall command.
- **Mock drift risk:** if the mock grows policy, filesystem access or a database,
  the mandatory boundary (ADR-0001) is breached. The lint and typecheck do not
  catch this; review does. ADR-0009 records the revisit trigger.
- **Inline styles:** `WatchlistPage` and `DownloadsPage` use a few inline styles
  that reference tokens (no literals). They are honest but inconsistent with the
  CSS-module convention; a follow-up should move them into modules.

## Next actions

1. With explicit engine consent: A6 engine build (Docker/WSL2) → A8 boundary
   evidence. A9 (mount override) already landed at `c23b06f`; its mount-scope
   proof and live token forwarding remain NOT RUN. This is the critical path to
   the container checkpoints.
2. After the human partner accepts ADR-0015: playback B2/B3 (engine + stream
   route). The B1 code path is proposed, not accepted.
3. Resolve O-08 / package ID and accept the remaining Proposed ADRs — human
   decisions only.
4. Do not mark any checkpoint (CP-0..CP-7) passed; each needs observed evidence
   and, where required, human sign-off.

## Decisions made

- **Docker is mandatory for v1** — reaffirmed by the product owner and now
  consistent across every document. Recorded in ADR-0001 Part 1.
- **Frontend-first delivery** — the product owner chose to build the full frontend
  before the backend. Recorded in ADR-0009. The frontend is built against a frozen
  contract with a development-only mock; the mock implements no policy.
- **Runtime distribution: dedicated WSL2 Docker Engine recommended** (ADR-0001
  Part 2). Status **Proposed**; the human partner accepts.
- **Playback on the Windows host** (ADR-0002). Status **Proposed**; placement
  recommended, engine and licence still open.
- **WSL2 mirrored networking excluded** from the publication path (R-18). Basis:
  Microsoft's own documentation lists LAN reachability as a mirrored-mode benefit.
- **Phase 6 restructured** to three tasks, with the workspace brought forward to
  P1-07/P1-08. Basis: the dependency inversion described above.
- **Watchlist added** as a first-class feature (Development.md §4.5.1, P2-07).
- **Git:** repository initialised, remote configured. Commit and push authorised by
  the human partner in this session; three commits on `master`, all pushed and
  verified in sync with `origin/master`.
- `memorix` remains **not adopted** — unchanged; it would need a decision record.
- No other product or architecture decision has been made by an agent.

## Open questions

1. **Runtime distribution:** dedicated WSL2 distro (recommended) or Docker Desktop?
   O-06 / O-07. Needs P0-10 evidence before acceptance.
2. **libmpv licence (O-09):** LGPL or GPL? This constrains the project's own
   licence and blocks CP-0.
3. **Project name and branding** (O-01) — needed before the installer and package
   IDs are fixed.
4. **Is ADR-0009 accepted as written?** It records the frontend-first sequencing
   and the contract+mock approach.
5. **Are ADRs 0002, 0005, 0006, 0007 and 0008 accepted as written?**

## Agent handover

- **HEAD:** `7d48507`.
- **Tree:** clean (`git status --short` empty) at the start of the verified run.
- **Local gates:** all green — `cargo fmt --all -- --check`, `cargo clippy
  --workspace --all-targets -- -D warnings`, `cargo test --workspace` (107 passed,
  1 ignored), `npm run typecheck`, `npm run lint`, `npx vitest run` (11/11), `npm
  run check:tokens` (61 files, 0 violations), `npm run build` (91 modules).
- **NOT RUN:** Docker/WSL2 engine evidence, live token forwarding, hardware
  playback matrix, TV-distance review, GitHub Actions green run, installer
  lifecycle, CP-0 evidence. No checkpoint passed; no ADR self-accepted.
- **Do not repeat:** `pids_limit` next to `deploy.resources.limits` (Compose v2
  rejects it); claiming WSL2 has no GPU access (it has compute access); claiming a
  named volume is unreachable from Windows (it is reachable via the WSL share);
  treating mirrored networking mode as a harmless fallback (it exposes the
  published port to the LAN).
- **Next safe action:** obtain engine consent, then A6 engine build → A8 boundary
  evidence (A9 mount override landed at `c23b06f`; live forwarding and mount-scope
  proof still NOT RUN), or playback B2/B3 after ADR-0015 acceptance.
- **Do not:** commit, push, accept an ADR, pass a checkpoint, or start the engine
  without the human partner's explicit consent.

---

## Backend foundation (2026-11-17)

The Rust backend foundation has been implemented per
`docs/superpowers/plans/2026-11-16-backend-foundation.md` (Tasks 1–6). The
`core` binary now serves the frontend's frozen command contract from SQLite
over an authenticated HTTP API, so the UI's data-dependent acceptance criteria
can finally be exercised.

### What shipped (commits `6a809f0`, `6322beb`, plus uncommitted Task 4–6 work)

**Task 1 — Workspace scaffold**
- `Cargo.toml` workspace root with members `app-core`, `media-persistence`
  (dir `crates/persistence`), `mediahaven-core` (dir `crates/core`).
- `rust-toolchain.toml` pins channel `1.99.0` with rustfmt + clippy.
- Crate manifests with workspace-inherited versions and dependency declarations.

**Task 2 — app-core domain and ports**
- `crates/app-core/src/domain/media.rs`: `MediaKind`, `MediaAvailability`,
  `MediaSummary`, `HomeScreen`, `MediaShelf`, `ShelfId`, `LibraryScreen`,
  `BrowseQuery`, `SearchQuery`, `SearchResults`, `AppErrorCode`, `AppError`
  with serde derives producing the exact camelCase JSON the frontend expects.
- `crates/app-core/src/ports/library.rs`: `LibraryRepository` trait.
- `crates/app-core/src/application/library.rs`: `LibraryService<R>` delegating
  to the repository.
- `deny_unknown_fields` added to `BrowseQuery`, `SearchQuery` and (later)
  `SettingsPatch`, `PlaybackStartRequest`, `PlaybackSettingsPatch`,
  `StorageSettingsPatch` per ADR-0005 §4.

**Task 3 — persistence (SQLite + migrations)**
- Migration `crates/persistence/migrations/0001_init.sql` with `media_items`,
  `media_files`, `library_roots`, `watchlist_items`, `queue_items` tables,
  foreign keys, indexes, UTC timestamps. `cast` column renamed to
  `cast_members` (SQL keyword collision); `watchlist_items.media_id` UNIQUE
  for idempotent add.
- `SqliteLibraryRepository` implementing `LibraryRepository`.
- 7/7 tests pass; clippy clean.

**Task 4 — core HTTP server**
- `crates/core/src/{state,auth,routes,server,lib,main}.rs`.
- `/healthz` (unauth) → `{"status":"ok"}`; `/readyz` (auth) → `{"status":"ready"}`.
- 20 named command routes mirroring the frontend contract.
- Auth middleware per ADR-0005 §2: Bearer token + HMAC-SHA256 signature over
  `timestamp\nMETHOD\npath\nhex(sha256(body))`, ±30 s window, constant-time
  compare. Dev token `dev-only-not-for-production` gated behind
  `MEDIAHAVEN_DEV=1`.
- Body limit 1 MiB; CORS loopback-only.
- `crates/core/tests/auth.rs`: 11 auth-matrix tests, all passing.

**Task 5 — Seed + frontend HTTP transport**
- `crates/core/src/seed.rs`: fixture data inserted when `MEDIAHAVEN_SEED=1`;
  `run()` calls `seed_if_requested`. Smoke verified Home returns
  `Continue Watching` + `Recently Added Movies` shelves.
- `apps/desktop/src/services/httpTransport.ts`: WebCrypto HMAC transport.
- `apps/desktop/src/services/transport.ts`: HTTP transport selected when
  `VITE_BACKEND_URL` is set; mock remains the default.
- `apps/desktop/src/vite-env.d.ts`: typed `VITE_BACKEND_URL` / `VITE_BACKEND_TOKEN`.
- Frontend: typecheck + build (82 modules) + `check:tokens` (0 violations) pass.

**Task 6 — Dockerfile for core**
- `infrastructure/containers/Dockerfile.core`: multi-stage (rust:1.99 builder →
  debian:bookworm-slim runtime), non-root UID 10001, healthcheck on `/healthz`.
- `.dockerignore` created.
- Docker smoke: image builds successfully; container starts and
  `curl http://127.0.0.1:18081/healthz` returns `{"status":"ok"}` (PASS).
  (Dockerfile builder base upgraded from `rust:1.86` to `rust:1.99` to
  satisfy dependency MSRVs.)

### Verification

| Check | Result | Notes |
|---|---|---|
| `cargo test --workspace` | **PASS** | All crate tests + 11 auth-matrix tests |
| `cargo clippy --workspace --all-targets -- -D warnings` | **PASS** | Clean |
| Frontend typecheck (`tsc --noEmit`) | **PASS** | Strict mode |
| Frontend build (`npm run build`) | **PASS** | 82 modules |
| Frontend token check (`check:tokens`) | **PASS** | 0 violations |
| Docker build (`Dockerfile.core`) | **PASS** | Image builds |
| Docker smoke (`/healthz`) | **PASS** | `{"status":"ok"}` |

### Not tested
- End-to-end browser smoke with `VITE_BACKEND_URL` set against the running
  backend (Task 5 Step 4) — not executed in this session.
- `cargo fmt --check` — not run (no CI yet).
- Docker container security posture (`docker inspect` assertions for non-root,
  cap_drop, no-new-privileges, read-only rootfs) — not asserted; image runs
  as UID 10001 per Dockerfile but inspect-level verification was not performed.

### Known hazards
- The Dockerfile's `MEDIAHAVEN_BIND=0.0.0.0:8080` binds all interfaces inside
  the container, which is expected for the container's published port but must
  not be used for a host-native run without `MEDIAHAVEN_DEV=1`.
- `Cargo.lock` was modified (new dependencies); ensure it is committed.
- Uncommitted design assets (`.workbuddy-ai/`, `.impeccable/`,
  `OTT Platform UI (Community)/`, `PRODUCT.md`, `docs/design/tv-ui/`) are
  out of backend scope and remain uncommitted.

### Next safe action
- Commit Task 4–6 changes.
- Update `roadmap.md` task statuses (P1-01, P1-05, P1-08 partially
  satisfied; backend foundation plan complete).
- Run end-to-end browser smoke with `VITE_BACKEND_URL` set to confirm the
  frontend renders seeded backend data.

---

## Static TV prototype redesign

`docs/design/tv-ui` was restyled as a separate, local-only design-review surface
for the supplied Home references. The prototype uses its existing seeded
catalogue and artwork; no reference screenshots, external content services, or
account UI were added. The React product app and roadmap phase status were not
changed.

### Verification

| Check | Result | Evidence |
|---|---|---|
| Home at 1920×1080 | **PASS** | Browser measured centered 795px navigation, 626px hero, Up Next at y=578, following shelf heading at y=915; screenshot inspected. |
| Home at 1280×720 | **PASS** | Browser measured centered 679px navigation, 502px hero, Up Next at y=470; screenshot inspected. At this height Up Next captions reach the lower viewport edge; following shelf is not expected to show. |
| Keyboard/remote navigation and focus | **PASS** | Arrow navigation, keyboard-visible focus ring, Movies overlay Escape restoration. |
| Discover/Search/details/player paths | **PASS** | Discover routing, Search for seeded Dune, title details, player entry, Escape restoration to details and prior Search result. |
| Reduced motion | **PASS** | Emulated `prefers-reduced-motion: reduce`; screen animation duration observed as 0.00001s. |
| Browser errors and local assets | **PASS** | No page/console errors or failed requests reported by the browser harness. |
| Account/sign-up UI scan | **PASS** | No matching copy in rendered page or prototype/product/surface brief. |
| JavaScript syntax and diff whitespace | **PASS** | `node --check` on the four prototype JS files; `git diff --check` on scoped files. |
| React app build / backend acceptance | **NOT RUN** | Out of scope; this change affects only the static prototype and its design documentation. |

The 1920×1080 composition now shows the next shelf heading and the top of its
artwork cards. At 1280×720 the hero and Up Next shelf remain visible, but the
next shelf does not appear in the first viewport. This is a visual reference
prototype only, not product-roadmap completion.

---

## Backend Phase 2 — library (partial)

### Completed this session

**media-library crate** (`crates/media-library`, new workspace member)
- `path_policy`: canonicalise root, symlink/reparse refusal, containment check,
  extension allowlist, title sanitisation (control chars dropped, truncation,
  fallback).
- `metadata`: size, extension, title from filename (no ffprobe; technical
  metadata deferred).
- `scanner`: iterative, depth-capped DFS; honours cancellation flag; skips
  locked/disappearing files; applies extension allowlist; computes SHA-256
  hash up to 1 GiB.

**Persistence**
- Migration `0002_library_roots_path_and_media_tracking.sql`: `path` on
  `library_roots`, `root_id` on `media_items`, `path` on `media_files`.
- `LibraryRepository` extended: `root_add`, `root_remove`, `root_update`,
  `media_replace_for_root`.
- `load_roots` now returns `path`.

**app-core**
- `LibraryRoot.path` field; `MediaUpsert` domain type.
- `LibraryService` methods for root management and media replacement.

**core HTTP**
- New routes: `/library_root_add`, `/library_root_remove`, `/library_scan`.
- `/library_scan` scans a root synchronously, persists results, updates root
  status/count, returns `{ scanned, cancelled, errors }`.

### Verification
| Check | Result |
|---|---|
| `cargo build --workspace` | PASS |
| `cargo test --workspace` | PASS (all) |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| media-library scanner tests (6) | PASS |

### Not tested
- End-to-end scan against a real folder via HTTP (not executed).
- Symlink/reparse-point refusal on Windows (unit-level containment tested,
  OS-level reparse not).
- Root removal file-count assertion (backend deletes rows only; no files
  touched by design).

---

## React route follow-up (2026-11-17)

Implemented and verified a focused route slice in `apps/desktop`:
- Added an Anime page and navigation entry; it includes only movies and shows
  explicitly tagged with the `Anime` genre and explains the empty state.
- Added routed Details pages with Back navigation to the prior location (or Home
  on direct entry). Player remains an overlay above Details.
- Added a direct Search navigation link while preserving the global search field.
- Added route regression tests for Search, Anime filtering/empty state, Details
  navigation, and Player overlay behavior; added jsdom scroll API shims used by
  the existing tests.

Verification from `apps/desktop`: `npm test -- --run` (13/13 passed),
`npm run typecheck`, `npm run lint`, `npm run check:tokens` (52 files, 0
violations), and `npm run build` all passed. `git diff --check` passed. The
initial npm invocation from the repository root failed because there is no root
`package.json`; rerunning from `apps/desktop` succeeded.

This is a partial frontend route slice, not completion of Phase 4: no visual
acceptance or full keyboard-only walkthrough was performed, backend-dependent
acceptance remains pending P4-08, and roadmap task statuses were not changed.
Unrelated Rust/backend and static-prototype work remains in the working tree and
was not altered by this route follow-up.

---

## Backend Phase 2 — library completion (2026-11-17)

Completed the remaining Phase 2 backend work per
`docs/superpowers/plans/2026-11-17-phase-2-backend-completion.md` (Tasks 1–6).
No frontend changes; no schema/migration changes.

### What changed

- **Task 1 — scanner stable id + kind** (`crates/media-library/src/scanner.rs`,
  `lib.rs`): `ScannedFile` now carries `id: String` (SHA-256 hex of the
  canonical absolute path) and `kind: MediaKind` (non-Option). Added
  `pub fn stable_id` and `pub fn classify_kind` (video + audio → Movie;
  artwork extensions → None → skipped). Kept the `is_media_extension`
  allowlist gate. `classify_kind` lowercases internally.
- **Task 2 — persistence upsert/prune** (`crates/persistence/src/sqlite/library_repo.rs`):
  `media_replace_for_root` now upserts by stable id (ON CONFLICT DO UPDATE),
  rejects empty ids up front, runs in one transaction, bulk-deletes previous
  `media_files` rows via subquery, and prunes stale ids in 500-id chunks.
  Fixes the cascade-delete bug where `Uuid::new_v4()` was substituted for
  empty ids, wiping watchlist/queue/progress rows on rescan.
- **Task 3 — `library_scan` rewiring** (`crates/core/src/routes.rs`): sets
  `id`/`kind` from scanned files; response now
  `{"scanned","cancelled","errors":[strings],"duplicates":[{"hash","paths"}]}`.
  Duplicates computed in the route via a borrowed `BTreeMap<&str, Vec<&str>>`
  over `&items`, groups with fewer than 2 paths dropped. `errors` are
  `"{path}: {message}"`.
- **Task 4 — scanner acceptance tests** (`crates/media-library/tests/roots.rs`,
  new): 8 tests — path escape, artwork skip, hostile filenames, vanishing file,
  cancel/rescan idempotency, duplicate hashes, unique 64-hex ids, empty root.
- **Task 5 — persistence acceptance tests** (`crates/persistence/tests/repository.rs`,
  +10): cascade preservation (watchlist/queue/progress/availability), prune,
  no media_files accumulation, empty-id rejection, LIKE metacharacter escaping,
  escape-clause behavioural proof, UTC round-trip, root-index EXPLAIN, and an
  `#[ignore]` 10k search budget test.
- **Task 6 — deviations + docs**: ADR-0010 (library entity deviations, proposed),
  ADR-0011 (metadata provider selection, proposed), README index rows, roadmap
  notes, this entry.

### Verification (observed)

| Check | Result |
|---|---|
| `cargo test -p media-library --test roots` | PASS (8/8) |
| `cargo clippy -p media-library --all-targets -- -D warnings` | PASS |
| `cargo test -p media-persistence` | PASS (14/14, 1 ignored) |
| `cargo test -p media-persistence -- --ignored` | PASS (1/1, 1.39 s wall) |
| `cargo clippy -p media-persistence --all-targets -- -D warnings` | PASS |

### Not tested (NOT RUN)

- Full workspace gate (re-run in this pass): `cargo build --workspace` PASS;
  `cargo test --workspace` PASS (all suites, 1 ignored); `cargo clippy --workspace
  --all-targets -- -D warnings` PASS; `cargo fmt --check` PASS.

---

## Architectural decisions recorded (2026-11-18)

Provisional approvals applied: ADR-0002 (mpv via JSON IPC, abstract interface, no libmpv in main app), ADR-0001 Part 2 (dedicated WSL2 + Docker Engine, no Docker Desktop), ADR-0006 (volume policy), ADR-0007 (secrets), ADR-0011 (TMDB primary + abstraction). O-01 (MEDIA_HAVEN internal name, branding configurable). CP-2 remains OPEN — no sign-off. ADR statuses updated to Accepted (provisional); README index updated. Roadmap P3-01 and P2-06 dependencies unblocked; P1-08 unblocked on runtime distribution. Implementation tasks now able to proceed: P1-08, P2-06 (crates/metadata), P3-01 (crates/playback with mpv JSON IPC).

---

## Backend Phase 3 — checkpoint persistence (2026-11-17)

Added a `playback_checkpoint` checkpoint path across the stack:
- `PlaybackCheckpointRequest` domain type (mediaId, positionSeconds, durationSeconds?).
- `LibraryRepository::playback_checkpoint` + `LibraryService::playback_checkpoint`.
- `/playback_checkpoint` route with finite/non-negative validation.
- Persistence impl writes progress_position/duration/updated_at transactionally;
  returns `MediaMissing` for unknown media.
- Test `playback_checkpoint_writes_progress` (persistence) verifies write + rejection.
Gates: persistence 15/15 (1 ignored), core 11/11, clippy clean. Player engine
integration and frontend command remain out of scope.

---

## Backend Phase 3 — completion/resume domain (2026-11-17)

Added pure playback decision logic to `app-core` (no player engine, no I/O):
`PlaybackConfig` (thresholds with documented defaults), `is_complete` and
`resume_position`. Tests (7) cover seek-to-end, mid-episode close, normal
completion, short watch, tiny/near-end resume skip, and resume return.
Files: `crates/app-core/src/domain/playback.rs`, `domain/mod.rs`, `lib.rs`.
Gates: `cargo test -p app-core` PASS, clippy PASS. Player integration,
persistence of checkpoints and P3-01 engine remain gated on CP-2/ADR-0002.
- End-to-end HTTP scan against a real folder (no curl smoke).
- Windows reparse-point refusal at OS level: `path_escape_is_refused` created
  the symlink successfully on this host, so the privileged-skip branch was NOT
  exercised; the assertion path ran instead.
- `cargo fmt --check` — PASS (run after `cargo fmt`; no diff).

### Risks / open decisions

- `collections` remains unimplemented (ADR-0010, proposed).
- Audio is stored as `Movie` (schema CHECK); no series synthesis.
- Stable ids derive from paths; moving a root resets per-item state.
- Metadata provider (P2-06) blocked on O-04; no artwork cache.
- No commit or push was made; working tree is dirty.

---

## Frontend redesign — Apple TV Liquid Glass (2026-11-18)

Human-directed full UI replacement after the Stitch/Tailwind attempts were
rejected. Implemented from the attached Apple TV-style brief: liquid glass,
tvOS focus behaviour, 60/80 pt safe insets, 40 pt gutters, generous row gaps.

### What changed

- Rebuilt `apps/desktop/src` on one token layer (`src/styles/tokens.css` plus
  `src/styles/base.css`). Tailwind utility markup, the Google-hosted fonts and
  all remote `aida-public` artwork URLs are gone. Space Grotesk and Roboto
  Condensed are self-hosted from `public/fonts`; the eight stills from
  `docs/design/tv-ui/assets` were copied to `public/artwork`.
- New primitives: floating glass `Nav`; auto-rotating full-bleed `Hero`;
  `Shelf` with centred large titles; `MediaCard` (16:9, resume progress);
  authored SVG `Icon` set; `Controls` (button, chip, progress, toggle,
  segmented); `Surfaces` (page header, glass panel, field row, empty state);
  `CategoryTile`.
- Screens: launch gate and profile selection (add/remove with confirmation),
  Home, Movies, TV Shows, Music, Anime, Library, My List, Search (live results,
  recent searches), Details (backdrop plus glass sheet, seasons/episodes/tracks,
  More Like This), Player (simulated transport, seek, skip intro, next episode,
  captions, full screen), Downloads (storage plus destructive remove behind an
  inline confirm), Settings (preferences persist), Setup (three-step wizard).
- Local state in `src/state/AppState.tsx` (profiles, My List, resume positions,
  preferences, downloads, recent searches) persisted to `localStorage` key
  `mediahaven.ui.v1`. No transport, no Rust commands, no network calls.
- Spatial focus in `src/hooks/useSpatialFocus.ts`: arrow keys move real DOM
  focus by geometry, Enter/Space activate, the player owns arrows while open.

### Verified this session

| Check | Result | Notes |
|---|---|---|
| `npx tsc --noEmit` | **PASS** | strict, including `exactOptionalPropertyTypes` |
| `npx eslint . --max-warnings 0` | **PASS** | no suppressions |
| `npm run check:tokens` | **PASS** | 49 UI files, 0 violations |
| `npm run build` | **PASS** | 85 modules; 338 kB JS / 69 kB CSS |
| `npx vitest run` | **PASS** | 2/2 (`src/App.test.tsx`: launch gate, profile entry) |
| Dev server | **PASS** | `http://localhost:5173/`, no console errors |
| DOM layout probes | **PASS** | no horizontal overflow at 739×700 or 1600×900; nav pill fully visible; hero CTA inside hero; secondary ink ≥ 4.5:1 on black |
| Manual visual review | **PARTIAL** | Search, Settings, Library, Downloads, Details and Player screens reviewed in the browser; Home capture was unreliable in the preview compositor; 1920×1080 not reviewed |

### Not verified / limitations

- Playback is simulated with artwork and a timed transport. There is no media
  engine, and captions exist only for the sample series.
- The tvOS Top Shelf extension is not implemented; this is a web build.
- No real library scan, no Rust command layer, no persistence beyond
  `localStorage`. Fixture catalogue lives in `src/data/catalog.ts`.
- Keyboard walkthrough across every screen was not completed; arrow focus was
  exercised on Home only, and the search field was exercised directly.
- Tailwind/PostCSS dev dependencies and their config files remain in the repo
  but are no longer imported. Removing them needs an approved `npm uninstall`.

### Risks

- Hero legibility depends on layered scrims; only the bundled artwork has been
  checked.
- Eight local stills are reused across rows; real imports will change the
  composition and may need wider artwork handling.
- A regression test suite for the new components does not exist yet beyond the
  two shell smoke tests.

### Next recommended action

Review the rebuilt UI at 1920×1080 on the TV, then either (a) restore the typed
command contract and mock transport as the data seam for P4-08, or (b) add
checkpoint coverage for this redesign to `roadmap.md` before more UI work.

---

## UI consolidation and backend wiring (2026-11-18)

### Removed

- `Sitch final coalation, all ui files/` (untracked Stitch bundle).
- `docs/design/tv-ui/` (static prototype, its copy of the artwork and tools).
- `docs/superpowers/plans/2026-11-17-static-tv-ui-redesign.md` and
  `2026-11-18-stitch-cinematic-ui-conversion.md`.
- The Music screen, album/track UI and the Music nav tab: the schema only
  stores movie/series/episode.
- Tailwind, PostCSS and Autoprefixer dev dependencies and both config files;
  the UI is CSS Modules over the token system.

### Wired to the real backend

- `apps/desktop/src/services/api.ts` — one signed client for every named
  operation (HMAC-SHA256 over timestamp, method, path and body; ADR-0005).
  `/readyz` is a signed GET, everything else a signed POST. No fixture or mock
  fallback exists.
- `apps/desktop/src/services/types.ts` mirrors the Rust DTOs;
  `services/adapt.ts` maps them to the view model and drops fields the backend
  does not have (rating, match, tagline).
- `state/AppState.tsx` loads `home`, `movies`, `shows`, `watchlist`, `roots`,
  `settings` and `downloads` after `/readyz`, exposes mutations, and keeps only
  UI preferences (reduce motion, skip intro, recent searches, active profile)
  in `localStorage`.
- Screens now read that snapshot: Home (hero + backend shelves + My List),
  Movies, TV Shows, Anime (genre filter), Details (detail + per-season
  episodes + watchlist), Player (`playback_start`, checkpoint every 5s and on
  exit), Search (debounced `library_search`), Library (roots/counts), Watchlist,
  Profiles (backend settings document), Settings (playback + diagnostics),
  Setup (root add + real scan), Downloads (read-only backend state).
- The app gates on `/readyz`: loading screen, or an explicit offline panel with
  the backend origin, the command to start it and a Retry button. It never shows
  invented titles when the service is down.
- `crates/core/src/seed.rs` expanded to 5 films, 3 series, 11 episodes,
  watchlist/queue entries and 3 profiles for the three seeded accounts, and now
  updates the sample root's item count.
- Fixed a resume bug found by the wiring: `playback_start` with a null start
  position used to overwrite the stored checkpoint with zero. It now falls back
  to `progress_position`; `repository.rs` asserts reopening resumes at 120 s.
- `SettingsPatch` in the persistence test was missing the `profiles` field
  (compile failure); the test now sets `profiles: None`.
- Docs: `docs/design/design-system.md` rewritten for the shipped tokens and
  components; `apps/desktop/README.md` replaced with real run instructions, the
  data flow and the honest limits. `docs/design/navigation-spec.md` still
  describes the focus model accurately.

### Verified

| Check | Result |
|---|---|
| `cargo fmt --check` | **PASS** (after `cargo fmt`) |
| `cargo clippy --workspace --all-targets -- -D warnings` | **PASS** |
| `cargo test --workspace` | **PASS** (run with `CARGO_TARGET_DIR=target/test`, see risks) |
| `npx tsc --noEmit` | **PASS** |
| `npx eslint . --max-warnings 0` | **PASS** |
| `npm run check:tokens` | **PASS** (53 files, 0 violations) |
| `npx vitest run` | **PASS** 2/2 (launch gate, profile entry) against a mocked API |
| `npm run build` | **PASS** (338 kB JS / 74 kB CSS) |
| Browser against live service | **PASS** — `/readyz` + seven boot operations 200; profiles, hero, shelves and My List from SQLite; details + season switch; no console errors; no horizontal overflow at 748 px |
| Playback persistence | **PASS** — reopened at the stored 15:00 mark, checkpoint wrote 920 s, read back through `library_episodes` |
| Watchlist round-trip | **PASS** — remove + add returned 200 and the list changed in the UI and on disk |
| Settings write | **PASS** — autoplay off via the UI read back as `false` from `/settings_get`, then restored |
| Search | **PASS** — “dune” returned exactly one result from `library_search` |
| Scan flow | **PASS** — `library_root_add` → `library_scan` (0 files, empty temp dir) → `library_root_remove`, all 200 |
| Startup count | **PASS** — all seven requests answered after a cold backend restart |

### Not verified / limitations

- Video playback is still simulated (4× clock over artwork). Sessions and
  checkpoints are real; there is no media engine.
- Downloads are read-only: the backend reports `enabled: false` and exposes no
  mutation, so the screen shows the disabled state.
- No music surface (schema has no album kind).
- Artwork still comes from the bundled local set; `ArtworkRef` is always empty
  until a metadata provider lands (ADR-0011).
- Per-profile isolation does not exist: profiles are stored, but progress and
  the watchlist are install-wide.
- Queue endpoints exist but no screen consumes them.
- Profile add/remove via the UI was not exercised (the provider path is the same
  `settings_set` call verified for playback settings).
- Visual review at 1920×1080 was not repeated after wiring; checks were DOM
  geometry and the preview at ~750 px.
- The preview click helper resolves stale coordinates in this panel, so several
  interactions were driven with DOM `.click()` rather than a real pointer.

### Risks

- The production token path is not implemented: there is still no Tauri shell
  to inject `VITE_MEDIAHAVEN_TOKEN`, and the dev fallback must never ship.
- CORS accepts loopback origins only, by design.
- The seed uses real film/TV titles with synthetic metadata; it is dev data
  behind `MEDIAHAVEN_SEED=1`.
- On this machine the MSVC linker intermittently fails with `LNK1104` when
  relinking test binaries in `target/debug` (file lock, likely AV). Using
  `CARGO_TARGET_DIR=target/test` works; normal builds are unaffected.
- `crates/core/src/seed.rs` is now the only source of demo content; deleting
  `.mediahaven-dev/` and re-running with `MEDIAHAVEN_SEED=1` rebuilds it.

### Running it

```sh
MEDIAHAVEN_DEV=1 MEDIAHAVEN_SEED=1 \
MEDIAHAVEN_DATA_DIR="$PWD/.mediahaven-dev" \
cargo run -p mediahaven-core      # 127.0.0.1:8080

cd apps/desktop && npm run dev    # 127.0.0.1:5173
```

## 2026-11-18 — Profile colours are real values, assigned by the backend

- `settings_set`/`settings_get` run `normalise_profiles`: a stored hex is
  lower-cased and kept; anything else (the empty string the UI sends, or an
  older non-colour value) is assigned from `PROFILE_PALETTE` by position. The
  palette mirrors the `--tone-*` tokens, so the design is unchanged. There is
  no format-specific migration: the UI can only produce hex, so no other
  format needs special handling.
- The UI no longer builds or sends CSS variables: profile creation posts
  `avatarColor: ""`. No colour literal exists in the UI source, and
  `check:tokens` still reports 53 files, 0 violations.
- Regression test `profiles_store_real_colours_not_css_variables` covers
  assignment, replacement of an invalid value and hex normalisation.
- Review pass: dropped the `var(--tone-N)` parsing and the UI's acceptance of
  that format, since the UI can no longer produce it; an unrecognised value is
  simply treated like an empty one. Palette assignment is positional, so a
  profile's stored hex is never recomputed once written.

Verified: `cargo fmt --check`, `cargo clippy --workspace --all-targets`,
`cargo test -p media-persistence profiles_store_real_colours`, `tsc`, eslint,
`check:tokens`, vitest (2/2). Against the live API: a profile posted with `""`
read back `#82755f`; an invalid colour was replaced with a palette value; the
original three profiles were restored unchanged.

Verified through the dialog in the running app: "Add profile" → name "Verify" →
"Create profile" stored `#82755f` in the backend settings, the new card rendered,
and "Manage profiles" → "Remove Verify" → "Remove profile" left exactly the
original Alex/Sam/Kids profiles (`#7e93a8`, `#a88e7e`, `#8aa88a`) with no console
errors; the active profile was reselected as Alex afterwards.

Not verified: visual review of the assigned avatar colours beyond the created
card rendering in the profile grid.

## 2026-11-18 — Backend completion master plan (planning only)

No implementation in this session. Plan written to
`docs/superpowers/plans/2026-11-18-backend-completion.md`.

What the audit found, stated plainly because earlier sections of this file are
stale:

- The Rust backend **does exist** and is wired to the frontend: 25 protected
  routes plus `/healthz`, SQLite persistence, scanner, watchlist, queue and
  settings. Earlier text in this file that says "the backend does not yet
  exist" / "No Rust, no database" is superseded.
- It runs on the **host** (`cargo run -p mediahaven-core`), not inside the
  mandatory Docker boundary. `infrastructure/containers/` holds artefacts that
  have never been run.
- `apps/desktop/src-tauri/` does not exist, so a production build has no token
  source and every request would fail with `missing_token`. The renderer talks
  HTTP directly with a development token, which ADR-0005 does not allow as the
  shipped shape.
- Playback is simulated in the UI (`SIMULATION_RATE = 4`); there is no engine
  and no stream route. Queue routes have no UI. Downloads are read-only.
- Document defect fixed 2026-11-18: ADR-0011's status line read "Accepted
  (provisional) — TMDB primary" while its Decision section said no provider was
  chosen. Both now say **Proposed, no provider approved**; TMDB is recorded as
  the recommended candidate only. The index row, the README open-decisions
  paragraph and the P2-06 dependency line were corrected to match. Nothing was
  marked accepted. (Superseded the same day: the human partner then accepted
  TMDB — see "O-04 accepted" below. This entry records only what was true before
  that answer.)
- Superseded: the earlier line in this file claiming provisional approval of
  "ADR-0011 (TMDB primary + abstraction)" does not describe a human decision
  and conflicts with the ADR body; treat it as an error, not as approval.
- Still open: `roadmap.md`'s current milestone describes Phase 0 with no
  backend. It was left untouched because changing a milestone is a human
  decision, and it is listed as a gate in the plan.

Verified this session: nothing new was built; the plan's own claims about
existing routes were re-checked earlier in the session by calling every read
path through the app's signed client (all answered). No metrics are claimed.

Not done: no roadmap status changed, no ADR accepted, no dependency installed,
no code written.

## 2026-11-18 — O-04 accepted: TMDB metadata provider

Documentation and decision only; no code, no dependency, no network call.

- The human partner accepted TMDB when asked to choose between "accept TMDB
  (with local NFO fallback)" and "local NFO only", answering "i have no problem
  with tmdb". O-04 is therefore **accepted**, not inferred by the agent.
- `docs/decisions/0011-metadata-provider-selection.md` is now `Accepted`
  (2026-11-18). It gained an acceptance record naming the human partner as the
  decider, and the Decision section now states the actual decision: TMDB primary,
  local NFO offline fallback, behind a `MetadataProvider` port. The obligations
  are written into the decision rather than left implicit: network use opt-in and
  off by default, only title/year sent, attribution rendered, key from the
  secret store, cache size cap with eviction, artwork sanitised.
- `docs/decisions/README.md`: index row is `Accepted`; the metadata provider was
  removed from the open-decisions paragraph.
- `roadmap.md`: P2-06 dependency line now points at the accepted ADR instead of
  an open choice; the O-04 register row records the acceptance and its
  conditions.
- `docs/superpowers/plans/2026-11-18-backend-completion.md`: decision D-3 marked
  resolved, D1 marked COMPLETE, D3 and D4 unblocked, dependency graph and the
  WS-D preamble updated. D2 (local NFO) still lands first so the offline path
  exists before egress.

Still open and unchanged: D-1 (container-first vs host-first sequencing),
D-2/O-06/O-07 (runtime distribution), D-4/O-05 (downloads legal sign-off),
D-5/O-09 (mpv licence), D-7 (token storage mechanism). No task from the plan has
started. Nothing was committed.

Verified: nothing executable changed, so no test run. The four documents were
re-read after editing to confirm the status, the decision and the roadmap all say
the same thing; that check was by inspection, not by a script.

## 2026-11-18 — O-04 accepted, remaining gates answered, Task D2 implemented

### Decisions recorded (all by the human partner, not the agent)

Asked one question per open gate; the answers:

| Gate | Answer | Landed in |
|---|---|---|
| D-1 sequencing | **Containerise first** | ADR-0013 (Accepted) |
| D-2 / O-06 / O-07 | **Dedicated WSL2 distro + Docker Engine; no reuse of the existing engine** | ADR-0001 Part 2 acceptance record |
| D-5 / O-09 | **LGPL mpv build** | ADR-0002 acceptance record |
| D-7 | **DPAPI via the `windows` crate; no `keyring`** | ADR-0014 (Accepted), amends ADR-0007 for the token |
| D-6 autoplay | **Off by default; Up Next offers the next episode** | Plan D-6 / C3 |
| D-4 / O-05 downloads | **Whole workstream deferred out of v1** | Plan WS-E marked deferred; roadmap O-05 |
| O-02 | **Windows 11 x64 only** | Development.md platform line; roadmap O-02 |
| O-01 naming | **MEDIA HAVEN is the project and app name** | Roadmap O-01 |

Evidence gathered for the runtime question, read-only: `docker --version` reports
29.9.3, `C:\\Program Files\\Docker` does not exist, and WSL lists a stopped
`docker-desktop` distro plus `OmniRushBox`. The machine therefore has *an* engine,
which is why O-07 needed an explicit answer.

### Task D2 — local NFO metadata (implemented)

New crate `crates/metadata` with:

- `NfoMetadataProvider` implementing a new `MetadataProvider` port
  (`crates/app-core/src/ports/metadata.rs`). One method, `lookup(path) ->
  Option<MetadataRecord>`; `Ok(None)` means no sidecar, `Err` means one was
  refused.
- A hand-written, bounded parser (`nfo.rs`): reads `<stem>.nfo` then `movie.nfo`
  beside the file, refuses >256 KiB, any `<!DOCTYPE>`/`<!ENTITY>`, non-UTF-8,
  control characters and unterminated elements; decodes only the five XML
  entities plus numeric references; caps title (512), synopsis (4000), genre
  (64 × 32) and drops impossible years/runtimes. No new dependency, no network,
  no writes, no symlink following.
- `MediaUpsert.metadata: Option<MetadataRecord>`; the scanner fills it, the
  routes let a sidecar title win over the filename, and
  `media_replace_for_root` writes year/synopsis/runtime/genres with `COALESCE`
  so a rescan without a sidecar cannot blank stored values.

### Commands and actual results

- `cargo fmt --all` — exit 0.
- `cargo clippy --workspace --all-targets -- -D warnings` — exit 0, no warnings.
- `cargo test -p media-metadata` — **8 passed**, 0 failed.
- `cargo test -p media-library` — **17 passed** across scanner/roots suites, 0
  failed.
- `cargo test -p media-persistence` — **17 passed, 1 ignored** (the pre-existing
  `searching_10k_items_is_within_budget`), including the new
  `scanned_sidecar_metadata_is_stored_and_survives_a_bare_rescan`.
- Linker note: the first two persistence runs failed with `LNK1104` on the test
  `.exe` in `target/review-test`; a fresh `CARGO_TARGET_DIR=target/review-persist`
  linked and passed. This is the known Windows file-lock issue, not a code
  failure.

### Not tested / not done

- No end-to-end check through the running app or UI: the scanner route and API
  were exercised only at crate level. No fixture library was scanned in the live
  app, so "NFO metadata appears on screen" is **NOT RUN**.
- Frontend unchanged; no TS typecheck or Vitest run was needed for this change.
- TMDB client, artwork cache, UI opt-in and egress flag are **not** started —
  those are D3/D4.
- Nothing committed.

### Task A2 — Tauri host scaffold (implemented)

`apps/desktop/src-tauri/` now exists:

- `Cargo.toml` — package `mediahaven-desktop`, lib target
  `mediahaven_desktop_lib`, workspace member; `tauri` 2.12.7 and `tauri-build`
  2.7.6 resolved through the workspace dependency table.
- `tauri.conf.json` — window titled `MEDIA_HAVEN` (1280×720, min 960×540),
  `devUrl` `http://localhost:5173`, `frontendDist` `../dist`, a CSP that allows
  only self, the local artwork protocol and the loopback API.
- `capabilities/default.json` — the `main` window gets `core:default` only. No
  filesystem, shell, dialog or HTTP plugin permission is granted.
- `src/lib.rs` — `get_app_info()` returning name, version, OS and arch for the
  smoke test; one unit test asserts the platform and version.
- `src/main.rs` — binary entry point, `windows_subsystem = "windows"` in
  release builds.
- `icons/` — generated from `apps/desktop/app-icon.png` (1024², monochrome
  play mark) with `npx tauri icon`; the source PNG is kept. Android/iOS icon
  folders were deleted: this product is Windows-only (O-02).
- `apps/desktop/package.json` gains `@tauri-apps/cli` 2.12.5 as a dev dependency
  and the `tauri` / `tauri:dev` scripts.

### Commands and actual results

- `npm install` in `apps/desktop` — exit 0, 2 packages added.
- `npx tauri --version` — `tauri-cli 2.12.5`.
- `cargo check -p mediahaven-desktop --all-targets` — exit 0.
- `cargo clippy --workspace --all-targets -- -D warnings` — exit 0.
- `cargo fmt --all` — exit 0.
- `cargo test -p mediahaven-desktop` — 1 passed (`app_info_reports_this_platform`).
- `cargo build -p mediahaven-desktop` — exit 0, `target/debug/mediahaven-desktop.exe`.
- Frontend untouched but checked: `typecheck` 0, `lint` 0, `check:tokens` 0
  violations across 53 files, `build` 0.
- Launch, against the Vite dev server already listening on `localhost:5173`:
  the process appeared with `MainWindowTitle: MEDIA_HAVEN`, `Responding: True`,
  ~29 MB working set, an `msedgewebview2.exe` child, and three established
  connections from that WebView to `127.0.0.1:8080` — the UI script ran and
  called the API, so the window loaded the real interface, not an error page.
  Stopping the app left **0** `mediahaven-desktop` processes. No
  `mediahaven-core` or `mpv` child was started by the host.

### Not tested / not done

- No screenshot or visual inspection: the window was observed through its
  title, responsiveness and the WebView's live API connections, **not** by
  looking at it.
- Cold start to first paint was **NOT measured**. The plan asks for a recorded
  number; the ~15 s pause above is a wait, not a measurement.
- The renderer still speaks HTTP directly to `127.0.0.1:8080` with the
  development token. Moving it behind named Tauri commands is A5/A7 and is
  **not** done; the CSP therefore still allows that loopback origin.
- The build was a debug `cargo build`, not `tauri build` or a bundle: signing,
  installer and `bundle.active` are out of scope here (WS-G).
- No supervisor, no token, no WSL2/Compose work: that is A3 and A4.

### Next

Task A3 (per-install token via DPAPI, ADR-0014) or A4 (supervisor lifecycle).
A3 must land before any command surface forwards to a containerised backend.

---

## 2026-11-18 — Tasks A3 and A4 implemented

### Task A3 — per-install token provisioning (ADR-0014)

New `apps/desktop/src-tauri/src/secret.rs`:

- `SecretToken` holds 32 bytes, deliberately **not** `Debug` and **not**
  `Serialize`, and zeroises its buffer on `Drop`. `as_hex()` returns a fresh
  bearer string; `as_bytes()` is read-only.
- `load_or_create_token(service, account)`: generate 256 bits from the OS CSPRNG
  (`BCryptGenRandom`), protect with DPAPI (`CryptProtectData`) under the current
  user, write to `%APPDATA%/MEDIA_HAVEN/<service>-<account>.blob` atomically
  (temp file + rename). Later calls unprotect and return the **same** token.
- Fail closed: a corrupt/unprotectable blob yields `SecretError::CredentialMismatch`
  and is never silently regenerated (R-25 — regenerating would orphan the
  running container's token).
- No `keyring`, no new dependency beyond the `windows` crate (ADR-0014). The
  `windows` crate was added to the workspace dependency table at version 0.62
  with the `Win32_Security_Cryptography` and `Win32_Foundation` features, and
  to the desktop crate; `thiserror` was added to the desktop crate for error
  types.

### Task A4 — supervisor lifecycle

New `apps/desktop/src-tauri/src/wsl.rs` and `supervisor.rs`:

- `Wsl::run(distro, args, timeout)` invokes `wsl.exe -d <distro> …` with
  **argument vectors only** — never a shell string, never `docker.exe` on
  Windows, no TCP daemon endpoint (ADR-0005 §3). A hard deadline with
  `try_wait` + kill covers a hung `wsl.exe`.
- `decode_output()` handles the real-world detail that `wsl.exe` writes UTF-16LE
  to **stdout**; reading only stderr would lose the whole error.
- `Supervisor::{ensure_running, wait_ready, stop, status}`: `compose up -d --wait`
  (30 s), loopback-port readiness poll (10 s, 250 ms backoff), `compose down`,
  and a live `SupervisorStatus` derived from the port. Failures map to the
  legible states in ADR-0005 §5 with user-safe messages (no path/token leak).
- The authenticated `/readyz` check is explicitly left to A5/A7, which owns
  HMAC signing; building it twice would violate ADR-0013's one-migration goal.

### Commands and actual results

- `cargo fmt --all` — exit 0.
- `cargo check -p mediahaven-desktop --all-targets` — exit 0.
- `cargo clippy -p mediahaven-desktop --all-targets -- -D warnings` — exit 0,
  no warnings (after fixing two clippy lints in the UTF-16 decoder).
- `cargo test -p mediahaven-desktop` — **12 passed, 0 failed**. New tests:
  token stable across calls, 256-bit hex shape, corrupt blob fails closed,
  non-Debug/redaction contract, drop path, UTF-16/UTF-8 decoding, nonexistent
  distro is a legible failure, closed-port NotReady/Stopped, and no path/token
  in status messages.

### Not tested / not done

- The DPAPI blob was exercised on this Windows host (round-trip, corrupt-blob,
  second-call). A **second Windows user** was not used to prove cross-profile
  unprotection fails — ADR-0014 requires that check and it is **NOT RUN**.
- No live `wsl … docker compose up` cycle: no compose file was rendered for the
  dedicated distro and the engine was not started (would need consent and A6).
  `wait_ready` was tested against a deliberately closed port only.
- `lib.rs` now declares the three modules and re-exports their public types, but
  no Tauri `invoke` command uses the token/supervisor yet — the window still
  talks HTTP directly. That is A5/A7.
- Nothing committed; the working tree's other uncommitted files are pre-existing.

### Review of A3/A4 and fixes (same session)

A read-only review found real defects; the cheap, verifiable ones were fixed and
re-verified. Remaining items are coupled to later tasks and are recorded, not
silently dropped:

Fixed and re-tested (17 desktop tests pass, clippy clean, fmt clean):
- `APPDATA` absent no longer falls back to `.` — `blob_path` now fails closed
  with `profile_unavailable`, so a token can never be written into the repo.
  Service/account are charset-validated so they cannot escape the blob dir.
- Zeroisation on drop now uses `write_volatile` + `compiler_fence` (a plain loop
  is a dead store LLVM may elide), and every error path zeroes its plaintext
  buffer before returning.
- `CRYPTPROTECT_UI_FORBIDDEN` set (no DPAPI prompt in a headless process);
  null/zero-length output blob guarded before `from_raw_parts`.
- Blob write fsyncs, cleans up its temp file on failure.
- The misleading no-op "redaction" test was replaced with a source-level guard
  proving `SecretToken` has no `Debug`/`Serialize` derive or manual impl.
- `compose up` is now **detached** (`-d`, no `--wait`): `--wait` blocks until the
  healthcheck's 40 s `start_period`, which exceeds this task's 30 s hard kill.
  Readiness stays in `wait_ready` (ADR-0005 §6).
- `stop()` failures now map to `stop_failed`, not `start_failed`.
- Engine output is redacted (32+ hex-char runs → `[redacted]`) before it can
  reach a user-facing state, pre-empting a future env-echo leak.

Deferred, with reasons (not claims of PASS):
- **C1 — token is not passed to `core`.** `compose.yaml` requires four vars
  including `MEDIAHAVEN_API_TOKEN`; `ensure_running` sets none, so a real
  `up` would fail. Injecting the token must happen without placing it in `argv`
  (process list) or a file — that is A6/A9's rendered compose override plus a
  `WSLENV`/in-distro env path, and needs engine consent. The token and supervisor
  are therefore correctly separated but not yet connected. **NOT RUN.**
- **C2 fully** — `wait_ready` still checks a TCP connect, not an authenticated
  `/readyz`; the signed probe belongs to A5/A7 which owns HMAC signing.
- **I3 — pipe draining.** `wsl.exe` stdout/stderr are not drained until exit;
  a very chatty child could fill the pipe and block until the timeout. The
  three commands used here emit little output; accepted for now, revisit if a
  compose error is observed to hang.
- **Second-Windows-user cross-profile DPAPI test** and any live `docker compose`
  cycle: need a second profile / engine consent. **NOT RUN.**

Research handoffs received: the A5 route map (24 real commands, no phantoms;
`/healthz`+`/readyz` are probes to exclude from parity; exact signing string and
12 error codes) and the A6 readiness report (Dockerfile.core likely broken since
A2 added the Tauri workspace member that `.dockerignore` excludes; no digests;
no Dockerfile.probe; engine consent required).

### Task A5 — named command surface (implemented and independently verified)

New `crates/contract` (`mediahaven-contract`) is the single source for the 24
renderer commands, the `Command` enum, the exact signing primitives
(`body_digest`/`canonical_message`/`sign`) and a `ScanOutcome` struct pinning the
`library_scan` inline-JSON shape. `apps/desktop/src-tauri/src/commands/` holds
all 24 `#[tauri::command]` functions plus a signed loopback `TcpStream`
forwarder; `lib.rs` registers all of them and no passthrough. Errors map to the
closed 12-code `AppErrorCode` set with fixed messages — backend text never
crosses the boundary.

New `apps/desktop/src/services/generated.ts` holds the `CommandName` union and
the 31 mirrored DTOs, marked as generated from `crates/contract`; automated
codegen is explicitly deferred to a future ADR (no codegen dependency added).
`api.ts`/`main.tsx`/`transport.ts` are untouched — that is A7.

Verification done this turn (not just reported by the implementing agent):
- `cargo fmt --all -- --check` → 0.
- `cargo clippy -p mediahaven-contract -p mediahaven-desktop --all-targets
  -- -D warnings` → 0, no warnings.
- `cargo test -p mediahaven-contract -p mediahaven-desktop` → contract 4/4,
  desktop 17/17.
- HMAC fixed vector in `parity.rs` re-computed independently with Python and
  matched byte-for-byte.
- TS type parity checked by script: all 31 `types.ts` exports present in
  `generated.ts`, plus `CommandName`.
- Confirmed from `crates/core/src/state.rs`/`server.rs` that the backend keys
  HMAC with the token **string** bytes, so the forwarder signing
  `bearer.as_bytes()` is correct — provided A6 injects the exact `as_hex()`
  string into `MEDIAHAVEN_API_TOKEN`.
- Parity test asserts the command list equals the 24 real renderer routes,
  forbids `http_request`/`api_request`/`docker_*`/probes, and that the enum
  round-trips with no duplicates.

Not done (out of A5 scope): A7 renderer transport swap, automated TS codegen,
a Tauri-builder "unknown command rejected" test, chunked-response decoding.

### Task A7 — renderer transport swap (implemented and verified)

The renderer now reaches the backend only through named Tauri commands in the
shipped host; the signed HTTP client is a dev-only fallback.
- New `services/errors.ts` (apiBase/ApiFailure/describeError, moved verbatim),
  `services/devClient.ts` (the only home of the dev token and `fetch`),
  `services/transport.ts` (`call()` — Tauri global → DEV-guarded dynamic import
  → `backend_unreachable`), and `services/transport.test.ts` (4 tests).
- `services/api.ts` is now a thin facade with the same method names and return
  types; each method calls a named command with snake_case outer args (Rust
  parameter names) and camelCase inner DTOs. `ready()` uses `settings_get`
  because `/readyz` is deliberately not a named command (A5).
- New `scripts/check-bundle.mjs` + `check:bundle` script. It scans only `.js`
  chunks (not sourcemaps) and asserts the dev token and HMAC signing markers
  are absent. **A subagent had changed `vite.config.ts` (disabled the preload
  polyfill, stripped sourcemap sources) to pass an over-broad `fetch(` grep;
  that was reverted** — changing production behaviour to satisfy a test is
  backwards. The narrow check proves the same thing without touching the build.

### A5 forwarder review — one real defect fixed

A read-only review of `commands/renderer.rs` confirmed all 24 request bodies
and the HMAC math are correct, but found the error mapping was by HTTP status
alone: 404 covers both `not_found` and `media_missing`, and 503 covers five
codes with different retryability. The forwarder now deserializes the backend's
exact `code` from the JSON body and falls back to the status map only if the
body is unparseable. New test `backend_code_is_preferred_over_status`. Backend
message text is still discarded (fixed messages only).

Verification this turn (all exit 0):
- Frontend: typecheck, lint, test (6/6), check:tokens, build, check:bundle.
- Rust: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo test --workspace` (desktop now 18 tests).

### A6 — file-level work done (build itself NOT RUN, engine consent pending)

- `Dockerfile.core`: now copies the Tauri member manifest so Cargo can load the
  workspace. **The subagent's first fix (manifest only) was proven wrong by a
  local simulation**: Cargo also resolves the declared lib target path at
  manifest-load time and failed with "can't find library mediahaven_desktop_lib".
  Fixed by adding `infrastructure/containers/desktop-lib-stub.rs`, copied to
  `apps/desktop/src-tauri/src/lib.rs` in the image. `.dockerignore` un-ignores
  the one manifest path.
- `Dockerfile.probe` (new): ffmpeg-builder stage → distroless `cc-debian12:nonroot`
  runtime with only ffprobe/ffmpeg + their `*.so*`, `USER 10002:10002`, no shell,
  no apt, no healthcheck, no secrets. Distroless `cc` (not `static`) because
  Debian's ffprobe is dynamically linked — the subagent corrected this; both
  base tags still need digests recorded at build time.
- Verified WITHOUT the engine: `cargo metadata --no-deps --offline` (exit 0) and
  `cargo build --release -p mediahaven-core --offline` (exit 0, 1m06s) against a
  reconstructed build context containing only root Cargo.toml/lock, `crates/`,
  the copied Tauri manifest and the lib stub. This proves the Dockerfile's
  copy set is sufficient. `docker compose config --quiet` with no env → exit 1
  with the four "required variable" errors (fail-closed, as intended).
- NOT RUN: `docker build`, digest recording, `docker inspect`, probe smoke —
  all need engine consent. Base-image digests, the `.dockerignore` negation
  against a real context, and distroless library completeness remain build-time
  gates.

### Task C1 — token injection into core (implemented, verified structurally)

The DPAPI token now reaches `core` at `compose up`, closing the 401 blocker —
in code, not yet on a live engine.
- `secret.rs`: `TOKEN_SERVICE`/`TOKEN_ACCOUNT` ("mediahaven"/"core-api") and
  `install_token()` are the single identity; `renderer.rs` deleted its private
  copies and imports them, so forwarder and supervisor cannot drift.
- `wsl.rs`: `run_with_env()` sets child-only env via `Command::env()` (no
  `std::env::set_var`, no global mutation); `run()` is a zero-env wrapper.
- `supervisor.rs`: `ensure_running()` loads the token, passes
  `MEDIAHAVEN_API_TOKEN=<as_hex()>` plus `WSLENV=MEDIAHAVEN_API_TOKEN` in the
  child env. The token is **never in argv** (not visible in the process list).
  `as_hex()` is an unzeroised `String` — documented `shortcut:` (upgrade if a
  zeroising hex buffer is added).
- Verified without an engine: 21 desktop tests (incl. token-in-env / not-in-
  argv / no-global-env-mutation, shared-identity source guard, env-vector
  shape), `cargo clippy --workspace --all-targets -- -D warnings` (0),
  `cargo fmt --all`.

NOT RUN: that `wsl.exe` actually forwards `MEDIAHAVEN_API_TOKEN` into the
distro and that `${MEDIAHAVEN_API_TOKEN}` resolves in compose. That is the
engine-side validation. Known WSLENV caveat: a value exported inside the distro
overrides the injected one — must be checked before relying on it.

### Next

A6 engine build (needs explicit consent) → A8 boundary evidence. A9 media-root
read-only mount rendering already landed (`c23b06f`). The code path is complete;
only live engine validation (build, digests, probe smoke, token forwarding,
mount-scope proof) remains NOT RUN.

---

## 2026-11-19 — documentation reconciliation and live verification

Four parallel read-only audits (code state, document state, governance/consent,
test/evidence gaps) were run, plus a live verification pass at HEAD `7d48507`.

Observed live results (exit code 0 for every command): `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`
(107 passed, 1 ignored), `npm run typecheck`, `npm run lint`, `npx vitest run`
(11/11), `npm run check:tokens` (61 files, 0 violations), `npm run build`
(91 modules). `git status --short` was empty when the run began.

Rulings made by the controller: no worktree was created for this docs-only change
because merging it back would need consent the current instruction does not give;
parallel agents were used because each owns disjoint files, which satisfies the
conflict-avoidance purpose of the one-agent-per-task rule; no ADR was self-accepted
and no checkpoint was marked passed.

NOT RUN and still blocking: Docker/WSL2 engine evidence, live token forwarding,
hardware playback matrix, TV-distance review, GitHub Actions run, installer matrix.

**Superseded / current state:** the three "A6 → A8 → A9" sequences above
(`## Next actions`, `## Agent handover`, and the older A5/C1 `### Next`) predate
commit `c23b06f`. A9 — the media-root read-only mount override — **landed** at that
commit. Only A6 (image builds, digests, probe smoke) and A8 (boundary evidence)
remain to be built, plus live `wsl.exe` token forwarding (C1) and mount-scope
proof, all still **NOT RUN** pending engine consent. No ADR or checkpoint status
is changed by this note.

## 2026-11-19 — scan-recovery slice (code change, verified)

Implemented F1 interrupted-scan recovery, planned in
`docs/superpowers/plans/2026-11-19-scan-recovery.md`:
- New `LibraryRepository::recover_interrupted_scans()` and a `LibraryService`
  wrapper; `core::server::run` calls it once at startup before serving.
- SQL updates only roots stuck in `scanning` → `error` with message
  "Scan interrupted by restart." It never deletes rows or media; it is idempotent.
- Regression test `interrupted_scan_is_marked_error_without_touching_media`
  confirms status and message, a no-op second pass, and an unchanged media row count.

Verified at this commit: `cargo fmt --all` 0; `cargo clippy --workspace
--all-targets -- -D warnings` 0; targeted test 1/1; full `cargo test --workspace`
0 with 108 passed and 1 ignored (the pre-existing 10k search budget test). The
first full-suite attempt hit the documented Windows LNK1104 file lock in
`target/debug`; a rerun with `CARGO_TARGET_DIR=target/verify-slice` passed clean.
Frontend was unchanged by this slice (last observed green at this commit:
typecheck, lint, Vitest 11/11, token check, bundle check, build).

Code check (`code-check-report.md`): no Critical/Important findings in the six
newest commits. One pre-existing Minor: `LibraryRoot.path` and
`ScanOutcome.duplicates[].paths` carry raw paths to the renderer — display-only,
not a playback source handle; recorded against ADR-0015's "no path in a DTO" wording.

NOT DONE: no commit (AGENTS.md requires permission), no engine/host action, no
Playwright change. Checkpoints and ADR statuses unchanged.

## 2026-11-19 — remaining-development plan, Docker build, parallel Lane C

Plan: `docs/superpowers/plans/2026-11-19-remaining-development.md` (Lane A
container boundary, Lane B playback behind the ADR-0015 human gate, Lane C
reliability/security/packaging). Subagent-driven development with parallel
execution was used; each agent owned disjoint files.

Lane A — **engine online, consent granted in-task**:
- `core` image built (exit 0), ID `sha256:3d773f6f…`, 146 MB. `probe` image built,
  ID `sha256:a0f3da2c…`, 390 MB. Two real defects found by running them and
  fixed: probe's `*.so*` copy missed `libpulsecommon`/`libblas` (replaced with an
  ldd-resolved `/libs` copy + `LD_LIBRARY_PATH`); core's root-owned volumes
  blocked UID 10001 (fixed with image-level `chown`); probe's bare ffprobe
  entrypoint caused a restart loop (now `/bin/sleep infinity` idle, ffprobe on
  PATH for exec). Full commands/output in `docs/security/boundary-evidence.md`.
- Live `docker compose up -d --wait` → exit 0, both healthy. Auth matrix: 9/9
  cases as designed (6 attacks → 401, 3 valid → 200). `docker inspect`: non-root,
  read-only rootfs, cap_drop ALL, no-new-privileges, resource limits, no Docker
  socket, loopback-only published port, probe internal-only with no gateway.
  `docker exec ffprobe -version` works. `docker compose down` → 0 orphans.
  A9 live mount-scope proof remains NOT RUN.

Lane C (parallel agents, disjoint files):
- C1/F3: `tests/security/traversal.rs` (10 tests, deliberate-red verified), CI
  step added (`.github/workflows/ci.yml`). Note: symlink vector not exercised on
  Windows; no GitHub Actions run.
- C3/F5: new `crates/diagnostics` with redaction bundle (3 tests). Fixed
  `.gitignore` so the crate is not swallowed by the `diagnostics/` export rule.
- C4/F4: `docs/security/socket-inventory.md` — read-only baseline; during/after
  app-session rows are NOT RUN.
- Playwright: no runner installed; installed-run needs consent. Wrote
  `tests/e2e/README.md` procedure; smoke NOT RUN. Plugin/config untouched.

Final verification (all exit 0): `cargo fmt --all -- --check`, `cargo clippy
--workspace --all-targets -- -D warnings`, `cargo test --workspace` (121 passed,
1 ignored), frontend typecheck/lint/Vitest 11-11/token check (61 files, 0)/build,
`git diff --check`, `bash tests/dependency_graph.sh`. Used
`CARGO_TARGET_DIR=target/verify-final` to dodge the known LNK1104 lock.

NOT DONE / gates: no commit; ADR-0015 acceptance (blocks Lane B/playback);
O-08 signing cert + package ID (block packaging); ADRs 0004/0005/0008/0010;
O-03 offline guarantee; CP sign-offs; A9 live mount proof; TMDB key + egress
consent; Playwright install consent. No checkpoint passed, no ADR self-accepted.

## 2026-11-20 — blanket human acceptance recorded (ADR-0016)

The human partner wrote, verbatim, "i accept whatever you need me to accept".
This was recorded **specifically and reversibly** in `docs/decisions/0016-blanket-acceptance-2026-11-20.md`
with each decision named, so any single item can be revoked. Applied and attributed:

- ADR-0004, 0005, 0008, 0010, 0015 → **Accepted** (status lines updated in each
  ADR and the index). ADR-0015's internal "Proposed" body text was corrected so
  code may depend on it. ADR-0009 body corrected to match its already-Accepted
  index row; ADR-0012 (profiles, already implemented) accepted.
- O-03 → fully offline after install (local playback/browsing never need a network).
- O-08 → ship **unsigned** in v1, publish the SmartScreen caveat; signed build
  needs a future human budget decision.
- O-01 residual → package ID `com.mediahaven.desktop`.

`roadmap.md` O-01/O-03/O-08 rows and the packaging README updated to match.
No checkpoint was passed by this (a human still reviews each CP summary).

With ADR-0015 accepted, Lane B playback is unblocked. mpv is NOT installed on
this host and installing it needs explicit consent (`AGENTS.md`), so the adapter
is being built and tested headlessly against a fake IPC transport; the live
"a real file plays" smoke remains **NOT RUN** until an mpv binary is available.

## 2026-11-20 — playback adapter + host/renderer wiring (Lane B, implemented)

Plans: `docs/superpowers/plans/2026-11-20-playback-host-wiring.md` (written with
writing-plans) and `2026-11-19-remaining-development.md` Lane B. Built with
subagent-driven development (adapter agent, then one coordinated wiring agent).

- **B2 adapter** — new `crates/playback` workspace member: `mpv::Player` port,
  `MpvPlayer` (argument-vector spawn, 5 s IPC connect, kills child on Drop),
  `IpcTransport` trait + in-memory fake, and `resolve_source` (ADR-0015 host-side
  container-path → host-file resolution, fails closed on stale/escape/missing).
  11 headless tests + 1 `#[ignore]` live-mpv test.
- **B3 wiring** — backend `GET /playback_source/:media_id` (host-internal; GET so
  the 24-command POST parity test is untouched); `LibraryRepository::media_source_path`;
  Tauri `PlayerState` resolves via A9's `render_mounts().host_path_map()`, opens
  mpv, emits `player://state` (display-only `{timePos,pause,volume,mediaId}`) every
  500 ms, and exposes named `player_control` (play/pause/seek/volume) — no path or
  token in any renderer-facing DTO or event. Renderer `player.ts` listens to the
  event and drives `PlayerPage` from engine state; the 4× simulation is now a
  dev-only fallback when no Tauri engine stream exists. New `PlayerPage.test.tsx`.

Verified at this commit (all exit 0, fresh `CARGO_TARGET_DIR=target/verify-final2`):
`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace` (**136 passed, 2 ignored** — route parity still 24/24),
`npm run typecheck`, `npm run lint`, `npx vitest run` (**13/13**),
`npm run check:tokens` (63 files, 0), `npm run build`, `npm run check:bundle`
(no dev token / signed HTTP client). `git diff --check` 0.

NOT RUN: live mpv playback, codec/hardware-decode matrix, real WebView event
emission — mpv is absent (install needs consent). **CP-3 remains OPEN.**
Integration assumption needing live validation: stored scan paths must use the
`/media/root-<n>` container form that `resolve_source` matches against the A9 map;
this can only be proven with a live scan + mpv. Agent notes (not defects):
mpv spawn failure reports `engine unavailable` (not `source unavailable`); unmute
resets volume to 100; rapid reopen can briefly spawn two status pollers.

## 2026-11-20 — live mpv playback verified; real IPC defect fixed

Consent granted for installing mpv. winget had no mpv package and choco lacked
admin rights, so a portable official mpv v0.42.0 x64 build was downloaded to the
approved temp dir (`C:\Users\Stanley\AppData\Local\Temp\opencode\mpv-portable`)
— testing only, not a shipped artefact. The "full" build is GPL-flavoured; O-09
still requires an LGPL-configured build for release (unresolved).

A 5-second H.264 test file was generated with the probe container's ffmpeg.
Running the ignored live test exposed a **real defect**: mpv emits async event
lines (`start-file`) on the same IPC channel, and `MpvPlayer::command()` treated
the first line as the reply — desyncing command/response pairing. On Windows the
`File` pipe read has no timeout, so this hung for 423 s. Fixed in
`crates/playback/src/mpv.rs`: commands now send the proper mpv envelope
`{"command":[...],"request_id":N}`, `command()` skips lines with an `event`
field or a different `request_id`, and caps reads at 64 to avoid an infinite
wait. Added regression test `event_lines_are_skipped_when_matching_a_reply`.

Verified: `cargo test -p media-playback` headless 0 (12 passed, 1 ignored);
live `cargo test -p media-playback -- --ignored` with `MEDIAHAVEN_MPV` and
`MEDIAHAVEN_TEST_MEDIA` set → **PASS in 0.90 s**, no orphan mpv process. This
closes B2/B3 engine-path verification. The codec/hardware matrix (B6) and the
container-path-format integration assumption are still being gathered by the
parallel evidence agents.

## 2026-11-20 — parallel evidence + D3 + frontend defect fix

Four consented parallel agents were dispatched (A9/Phase 0 measurements,
Playwright smoke, frontend CP-4 evidence, TMDB D3). Results so far:

- **D3 (TMDB provider)** — implemented off-by-default in `crates/metadata`
  (`tmdb.rs`, `cache.rs`): opt-in gate (zero requests when disabled), 30-day
  cache with byte cap + oldest-first eviction, injectable fetch seam (no HTTP
  client dependency — adding one needs an ADR), sanitisation, `TMDB_ATTRIBUTION`,
  env key lookup. 8 fake-fetch tests + 8 NFO tests pass; fmt/clippy 0. No live
  TMDB call (no key). P2-06 NOT complete (real HTTP transport, UI opt-in toggle,
  artwork route and attribution rendering remain).
- **Frontend CP-4 evidence** (`docs/testing/frontend-evidence.md`) — 3/7 items
  observed, 4 NOT RUN with reasons. Contrast: 19/25 pairs ≥4.5:1; real misses
  on `--paper` over `--tone-3/4` (4.42/4.21:1). Found `--paper-80` used as text
  colour in 4 CSS modules but undefined in `tokens.css`. **Fixed** by adding a
  `--paper-80` alias token (0.8 alpha) in `tokens.css`; token check 0 violations,
  build 0. Backend 10k search PASS (1.68 s test body); Vitest 13/13. NOTE: no
  focus-navigation suite exists at HEAD (removed in the UI consolidation), so
  P4-03 has zero automated focus coverage — a real gap. Full keyboard walkthrough,
  TV-distance/dropped frames, frontend 10k render, and containerised-backend E2E
  remain NOT RUN with reasons. CP-4 NOT passed.

The A9/Phase-0 and Playwright agents were killed by server restarts and
re-dispatched; their workspace containers were left running by the first attempt
and the second was asked to finish and tear down.

### Parallel evidence results (all agents complete)

- **Playwright smoke: PASS** — `npm i -D @playwright/test` + `npx playwright install chromium`
  (consented), `playwright.config.ts` + `tests/e2e/smoke.spec.ts`, `playwright test` →
  4 passed (18.4 s), no orphan node/cargo processes. Vite binds `::1` on this host, so
  the config adds a Playwright-only `--host 127.0.0.1` (no `vite.config.ts` change).
  Backend used the prebuilt binary, not a fresh cargo build. 10 pre-existing npm audit
  vulnerabilities reported (out of scope).
- **A9 mount scope: PASS** (`docs/security/boundary-evidence.md`) — core had exactly
  core-data, staging and one `/media/root-0` `ro` bind; no socket, not privileged;
  writes to the mount returned "Read-only file system". P0-13: 5/6 PASS (only
  `127.0.0.1:18082`, no portproxy, no 2375/2376, NAT not mirrored; second-machine
  LAN test NOT RUN — no second machine). P0-10/P0-14 measured: core 10.45 MiB,
  probe 2.13 MiB, cold start median 6654 ms, WSL2 MemTotal 8,073,875,456 B.
  P0-11: VAAPI/NVDEC/Vulkan all FAIL on this machine (Intel Iris Xe only; nvdec
  failure is a hardware-availability observation). No display/audio in container.
  Teardown PASS, no orphans (controller re-verified and cleaned a leftover probe).
  **CP-0 remains OPEN** — human reviews; full dedicated-engine comparison, playback-
  active footprint and second-machine LAN proof are NOT RUN.
- **A9 integration risk (UNTESTED):** `render_mounts` uses `std::fs::canonicalize`,
  which on Windows prefixes `\\?\`; whether Docker accepts a literal `\\?\`-prefixed
  bind source was not tested (the live override used the prefix-free equivalent).
- Image IDs drifted since A6 (sources/Dockerfiles changed) — rebuild/re-record at CP time.

CP-3 and CP-4 remain OPEN (human review + NOT RUN items). No checkpoint passed by an agent.

### A9 integration risk — fixed (real defect)

The A9 evidence flagged that `render_mounts` uses `std::fs::canonicalize`, which
on Windows returns `\\?\C:\...`. Confirmed live: `docker run -v "\\?\C:\...:/data:ro"`
fails with "invalid spec: too many colons" (exit 125). Fixed in
`apps/desktop/src-tauri/src/mounts.rs`: a `strip_verbatim_prefix()` helper strips
`\\?\` (and maps `\\?\UNC\` → `\\`) ONLY for the Compose bind source; the canonical
path stays in the persisted map for `resolve_source` containment. Added
`verbatim_prefix_is_stripped_from_the_bind_source` regression test (Windows).
`cargo clippy -p mediahaven-desktop --all-targets -- -D warnings` 0,
`cargo test -p mediahaven-desktop` 0 (31 tests, incl. the new one). This closes
the main A9 integration risk; the live override had used the prefix-free form,
so this makes the generated form match what was verified.

## 2026-11-21 â€” release blockers + IMDb + installer (parallel SDD wave)

Two plans written with writing-plans and executed with subagent-driven parallel
agents (disjoint file ownership): docs/superpowers/plans/2026-11-21-release-blockers.md
and 2026-11-21-imdb-metadata.md. Rulings in the two SDD ledgers under
.superpowers/sdd/2026-11-21-*.

Built:
- R-01 option A (operator-supplied source per item): item_sources migration 0007,
  port/service/repo, validation, tie of download row media_id by torrent hash,
  fail-closed NULL verification guard, source_for reads the store, Details UI
  Sources field, and the controller-added source_set chain (core host route +
  renderer forwarder + handler registration). Reviewed Approved (minors only).
- Security wiring: probe gate (real -v .../downloads:ro bind, automatic trigger,
  bridge to SQLite), Defender gate (argv MpCmdRun, fail-closed parser, detail()
  + playback_start), all fail-closed. Previously reviewed SHIP WITH DISCLOSED RISKS.
- IMDb provider: offline operator-TSV path (title.basics/ratings/age_ratings),
  opt-in via MEDIAHAVEN_IMDB_DIR (zero reads when off), signed /host/imdb_lookup,
  named command, additive display DTO fields, card + Details rendering with
  attribution, opt-in privacy doc. Reviewed Approved. OMDb HTTP remains a seam
  (new ADR needed) â€” not wired.
- Packaging: unsigned per-user NSIS installer config + release workflow + licence
  notice + checksums. Controller fixed tauri.conf.json (package ID
  com.mediahaven.desktop, bundle.active true, NSIS target, version 0.1.0),
  package-lock version, and the build.ps1 output path bug. REAL installer built
  and verified: target/release/bundle/nsis/MEDIA_HAVEN_0.1.0_x64-setup.exe
  (3.60 MiB) + SHA256SUMS.txt + THIRD_PARTY_NOTICES.md. build.ps1 gate exit 0.

Verified (final gate, all exit 0): cargo fmt --check, clippy --workspace -D warnings,
cargo test --workspace (260 passed, 2 ignored), frontend typecheck/lint/Vitest 27,
check:tokens (72 files, 0), build, check:bundle, git diff --check.

Disclosed risks: live IMDb/OMDb NOT RUN (no operator files/key); live qB acquisition
and real-media cleanup NOT RUN (no lawful source); card-level IMDb fields remain
None from list endpoints (Details works); opt-in is env-based not persisted;
SBOM NOT RUN (no tool, documented); installer non-deterministic across builds
(hash changed between runs, documented); second-machine LAN + TV hardware + CP
sign-offs remain human gates. No checkpoint passed by an agent.

## 2026-11-21 Task 1 — Block executable downloads (pre-download guard)

Human-priority security change, ADR-0024, plan
`docs/superpowers/plans/2026-11-21-block-executable-downloads.md`.

Implemented:
- `executables.rs`: pure, case-insensitive `has_executable_file` + the 29-entry
  `EXECUTABLE_EXTENSIONS` blocklist; final and inner dotted segments, both `/`
  and `\` separators. Re-exported from `lib.rs`.
- `qbittorrent.rs`: `add_form` now includes `paused=true`; `TorrentApi::resume`
  added (default no-op so the untouched `core` fake keeps compiling; QbClient
  overrides it with `/api/v2/torrents/resume`, acquisition fakes record calls).
- `orchestrator.rs`: `AcquireError::Rejected` (fixed, path-free message);
  `acquire` adds paused, polls `files()` 5x200 ms, rejects an empty/unreadable
  list or any executable-bearing name with `remove(hash, false)` and no resume,
  and only resumes an all-media task before the existing status/store/inspection
  path. Existing ffprobe and Defender gates were not changed and still run on
  every completed download.
- ADR-0024, roadmap R-22, regression tests in `tests/executables.rs`,
  `tests/qb_fake.rs`, and the orchestrator/cleanup fakes.

Verified (observed): `cargo test -p media-acquisition` 64 passed / 0 failed;
`cargo fmt --all -- --check` exit 0; `cargo clippy -p media-acquisition
--all-targets -- -D warnings` exit 0; `cargo check -p mediahaven-core
--all-targets` exit 0 (default `resume` keeps the untouched crate compiling).

Honest limits: this is a **name-based** pre-download guard — a deliberately
renamed executable is caught only by the existing post-download probe/Defender
gates. All acquisition tests used fakes; no qB instance, torrent or executable
was used or downloaded. **Live qB rejection is NOT RUN** pending a lawful source
and human review. No scraper/indexer/catalogue, no new dependency, no live
network/host change.

## 2026-11-21 — End-to-end artwork verification (observed)

Ran the updated backend from this branch on `127.0.0.1:8081` with
`MEDIAHAVEN_DEV=1`, `MEDIAHAVEN_SEED=1` and a throwaway data dir; stopped it
after the check. Signed `POST /library_home` returned 10 seeded items, 7 with a
relative `/artwork/<64-hex>` URL (Dune, Arrival, The Tomorrow War, Star Wars:
The Force Awakens, Blade Runner 2049 and two repeats). `GET /artwork/<hash>`
for Arrival returned HTTP 200, `content-type: image/jpeg`,
`x-content-type-options: nosniff`, `cache-control: public, max-age=604800`,
53,600 bytes, magic `FF-D8-FF-E0`; 5 poster files were cached. This proves the
API→cache→HTTP image path works end to end with real keyless IMDb data.
Still NOT RUN: a real WebView/browser screenshot and a full-library seed.

## 2026-11-22 — Operator TMDB token, live verification (observed)

With an operator-supplied TMDB API key in the backend process environment, ran
the updated backend on `127.0.0.1:8083` with seed data and stopped it after the
check. Signed `POST /library_home` returned 10 items, 7 with artwork URLs.
Arrival's hash was `a78d5e13...` versus the earlier keyless-IMDb `dfb0d8b5...`,
which proves TMDB supplied the image. `GET /artwork/<hash>` returned HTTP 200,
`content-type: image/jpeg`, `x-content-type-options: nosniff`, 68,524 bytes,
magic `FF-D8-FF-E0`; 5 posters were cached. The token string did not appear in
the API response. The key was stored in the Windows user environment only,
never in the repo, a DTO, a log or an error.

Still NOT RUN: a real browser/WebView screenshot with the token entered through
Settings, a live Docker Compose start that injects the DPAPI secret into `core`,
and a full-library seed. `Supervisor::ensure_running` currently has no
production Tauri caller, so the installed-app startup path is a separate
follow-up; this check verifies the `core` path directly.

## 2026-11-22 — TMDB discovery catalogue, live verification (observed)

Ran the updated backend on `127.0.0.1:8085` with the operator TMDB key and
stopped it afterward. Signed `POST /discover_catalog` returned, for each feed,
20 entries with 12 hashed artwork URLs: movies (first Iron Man), TV (first
American Horror Story), anime via `with_genres=16&with_original_language=ja`
(first Pokémon). `GET /artwork/<hash>` for a movie poster returned HTTP 200,
`image/jpeg`, 80,984 bytes. An invalid `kind` returned 400. The token did not
appear in any response. Anime coverage remains an approximation, and no real
browser/WebView screenshot of the Discover toggle was taken.

## 2026-11-22 — Startup, Prowlarr inputs and source search (observed)

Built: app starts the backend on mount; Settings stores TMDB key, Prowlarr
address and Prowlarr key in DPAPI; both Prowlarr values are injected into the
backend; five provider definitions with attribution are bundled; one-click
source setup adds YTS, Nyaa and The Pirate Bay (EZTV is Cloudflare-blocked and
AnimeTosho has no reachable Torznab host); Details has a "Find sources" panel.

Live verification with the operator's Prowlarr and key:
- `POST /host/indexers_seed` → three already present, two skipped with fixed
  reasons; no address or key in the response.
- `POST /host/source_search` for Arrival → 10 results, top `Arrival 2016
  1080p BluRay x264 DTS-JYK` with 439 seeders and a `magnet:?xt=urn:btih:` link.
- Same for Dune → 10 results, top `Dune Part Two (2024)` with 1,276 seeders.
- No `9117` address and no `http://` in either response.

Three live-only defects were found and fixed after the first empty searches:
Torznab supplies `infohash` instead of a magnet (now converted), release titles
often begin with "The" (now a word-aligned phrase match), and the HTTP client
timeout was 2 s (now 20 s). The backend was stopped after the checks.

Still NOT RUN: a real packaged-app cold start through WSL/Docker; a live click
that actually downloads a chosen torrent; a real browser/WebView screenshot.

## 2026-11-22 — Documentation and release handoff

Pulled the two documentation commits the user pushed to `master`
(`797be2b`, `342addf`) and merged them into this branch with `origin/master`
as the second parent. Kept our newer `progress.md` / `roadmap.md` wording where
they overlapped.

Updated `progress.md`, `roadmap.md`, and created `whats-left.md` in plain
language. It lists every open item, with the live gates already observed
clearly separated from the ones still untested. No code changed in this entry.

## 2026-11-22 — Anime source resolved; two decisions recorded

Live check: Prowlarr has a native Cardigann definition `animetosho-xyz`
(display name "Anime Tosho", base `https://animetosho.net/`). It was added
through `/api/v1/indexer` with the same payload shape as the other three and
returned id 4. The earlier "AnimeTosho has no working address" note was wrong:
`feed.animetosho.net` answers and returns JSON with `magnet_uri` and
`info_hash`; the failing host was `feed.animetosho.org`. RARBG and Torrentio
from the Miauflix list have no Prowlarr definition, so they cannot be seeded
without a separate search engine.

Decisions recorded in `whats-left.md`: OMDb age ratings are skipped for v1
(no key, setting remains "Not available"); checkpoint CP-0..CP-7 are signed by
the human partner only, with the agent preparing evidence.

Live recheck after wiring: `POST /host/indexers_seed` reported all four sites
as already present (`yts`, `nyaasi`, `thepiratebay`, `animetosho-xyz`) and only
`eztv-api` as not in Prowlarr. This confirms the native definition name matches
the real Prowlarr instance.

## 2026-11-23 — Installer and end-to-end torrent test

Built the unsigned per-user NSIS installer with
`infrastructure/packaging/windows/build.ps1`:
`target/release/bundle/nsis/MEDIA_HAVEN_0.1.0_x64-setup.exe`, 3.63 MiB,
SHA-256 `41a9b747b10977618bc91a95d802cb7e4e605919ac9f1a37f4f9152e3b325c09`.
SBOM bundling was NOT RUN (no `sbom/bom.json`). Nothing was published.

Ran a full download path with two locally created torrents, a local tracker,
and two throwaway qBittorrent containers:
- safe `sample.mkv` (86,016 bytes) added through the app's signed
  `/host/acquisition_add`, added paused, passed the file-name guard, resumed,
  and downloaded. qB state `stalledUP`, progress 1, file present.
- unsafe `installer.exe` torrent added through the same API was rejected with
  `400 invalid_request`; no `.exe` task or file remained.

Live test found a real bug: `QbClient::resume` used qB's `/api/v2/torrents/resume`
(404) instead of `/api/v2/torrents/start` (200). Fixed and pinned with a unit
test; installer rebuilt afterwards. Full detail:
`docs/testing/end-to-end-2026-11-23.md`.

Still NOT RUN: external torrent download, live ffprobe/Defender gates, browser
screenshot, packaged cold start, second-machine LAN check, checkpoint sign-off.

## 2026-11-23 — Browser click-through

Drove the Vite UI with Playwright against the real backend. Profile selection,
Home, Movies, Details, Discover, Anime, Setup and Player all rendered. Added a
local folder through the UI, scanned one file, opened it, clicked Play, and
reached the player preview with a running timer.

Four real bugs were found and fixed: snake_case vs camelCase command arguments,
a wrapped `source_search` body, movie Play links derived from episode lists,
and a missing Anime navigation tab. All 59 frontend tests and six gates pass.
Full report: `docs/testing/browser-qa-2026-11-23.md`.

Still NOT RUN: native mpv playback (not installed), axe-core, visual baseline,
packaged cold start, second-machine LAN check, checkpoint sign-off.


## 2026-11-23 - Installed app, full stack and folder import

Installed mpv and ffmpeg (portable, no admin) and verified mpv decodes a real
clip. Built and installed the NSIS app, then cold-started it through the new
Windows Docker engine mode: all three containers came up with the app's own
DPAPI token. TMDB discovery (movie/tv/anime) and Prowlarr source search returned
live data.

Found a real gap: the installed app could not import a host media folder because
the mount wiring was never implemented (deferred task). Wired it up: host-side
mount records, stable container paths, override staging in startup, root
add/remove remount with rollback, and host-path resolution for mpv. Review
found a startup-blocking stale-path regression and three consistency gaps; all
fixed and rechecked (63 tests).

Live evidence: cold start stages the override; docker inspect shows the test
folder and downloads mounted read-only; backend registers the container path;
scan found 2 files; playback_start returned a session; a stale record was
filtered without blocking startup. Details:
\docs/testing/mount-integration-2026-11-23.md\.

Installer rebuilt (unsigned, per-user), SHA-256
\41a9b747b10977618bc91a95d802cb7e4e605919ac9f1a37f4f9152e3b325c09\.
Human test guide: \docs/testing/human-test-guide.md\.

Still NOT RUN: UI click -> Tauri command -> mpv window (human test), WSL engine
mode, axe-core, visual baseline, second-machine LAN check, checkpoint sign-off.
