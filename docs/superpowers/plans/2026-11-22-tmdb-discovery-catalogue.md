# TMDB Discovery Catalogue Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Discover mode to Movies, TV Shows and Anime that browses TMDB's popular/trending catalogue with real posters, using the operator's existing TMDB token — without bundling any site list or indexer.

**Architecture:** A `DiscoverProvider` in `crates/metadata` wraps TMDB's `discover/movie`, `discover/tv` and `discover/tv?with_genres=16&with_original_language=ja` endpoints behind an injectable fetch seam with the existing 30-day size-capped `Cache`. One new renderer command `discover_catalog(kind, page)` returns `CatalogScreen` DTOs whose poster URLs are the same hashed `/artwork/<64-hex>` URLs used by the library, with bytes fetched and cached on demand inside one `spawn_blocking`. The renderer adds a "Discover" toggle to the existing Movies/Shows/Anime browse pages with display-only catalogue cards and the TMDB attribution line. Anime is approximated with TMDB filters (Animation genre + Japanese original language) — coverage is imperfect and stated honestly.

**Tech Stack:** Rust (`reqwest` 0.13.5 already present, `serde_json`, existing `Cache`/`ArtworkCache`/`artwork_key`/`fetch_artwork_bytes`), axum contract commands, React 19 + existing CSS tokens. No new dependency.

**Spec:** Human request 2026-11-22: "yes please and can we not just use the tmdb for that aswell, im pretty sure tmdb also covers anime"; ADR-0026 (operator-supplied TMDB token), ADR-0005 (named commands, no passthrough), `Development.md` §4.3/§7; `AGENTS.md` (no content-source bundling, no new dependency, tokens only).

## Global Constraints

- Uses ONLY the operator-supplied TMDB token already wired through ADR-0026. No token ⇒ Discover returns `available: false` and the UI shows "add a TMDB token in Settings"; no request leaves the machine.
- No scraper, indexer, catalogue file, site list or Cardigann definition is bundled. TMDB is the sole upstream; it is a metadata provider, not a download source.
- Discovery results are **display-only**: no auto-download, no playback, no mapping into the local library. No new acquisition path.
- One renderer command only: `discover_catalog`. Keep the contract/route/Tauri/generated.ts surfaces in sync; route parity test must stay green (now 25 commands).
- Reuse artwork controls: 2 MiB per image, 64 MiB total cache, `artwork_key` SHA-256 hex, 64-lowercase-hex route validation before any filesystem join, relative `/artwork/<hash>` DTO URL, `nosniff`, `onError` fallback.
- Cap each discovery page at 20 entries; cache the payload for 30 days with the existing `Cache`. Failures degrade to an empty Discover section, never break a browse page.
- TMDB attribution exact: `"This product uses the TMDB API but is not endorsed or certified by TMDB."`
- `PASS`/`FAIL`/`NOT RUN` honest; no live TMDB call in tests (fakes only).
- Commits permitted; no push/merge/install; no Playwright/vite config changes.

---

## File Structure

| File | Responsibility |
|---|---|
| `docs/decisions/0027-tmdb-discovery.md` (new) | Accepted ADR: TMDB browse catalogue, display-only, anime approximation, no bundling. |
| `docs/decisions/README.md` | Index ADR-0027. |
| `crates/metadata/src/discover.rs` (new) | `DiscoverProvider` + injectable fetch + JSON parsing + `CatalogEntry`. |
| `crates/metadata/src/lib.rs` | Re-export. |
| `crates/metadata/tests/discover.rs` (new) | Fake-fetch parse, anime query shape, hostile payload sanitisation. |
| `crates/contract/src/lib.rs` | Add `discover_catalog` to COMMANDS/Command/FromStr/as_str. |
| `crates/app-core/src/domain/media.rs` | `CatalogEntry`, `CatalogScreen`, `CatalogKind` DTOs. |
| `crates/core/src/routes.rs`, `crates/core/src/state.rs`, `crates/core/src/server.rs` | Command handler, provider wiring, poster caching. |
| `apps/desktop/src-tauri/src/commands/renderer.rs`, `lib.rs` | Tauri command + registration. |
| `apps/desktop/src/services/generated.ts`, `types.ts`, `api.ts` | Command union + DTOs + `api.discover()`. |
| `apps/desktop/src/components/catalog/CatalogShelf.tsx` (new) | Display-only catalogue cards + attribution. |
| `apps/desktop/src/pages/Movies/MoviesPage.tsx`, `Shows/ShowsPage.tsx`, `Anime/AnimePage.tsx` | Discover toggle. |
| `roadmap.md`, `progress.md` | Status, evidence, NOT RUN. |

---

## Task 1: ADR + DiscoverProvider in metadata crate

**Files:** create `docs/decisions/0027-tmdb-discovery.md`, `crates/metadata/src/discover.rs`, `crates/metadata/tests/discover.rs`; modify `crates/metadata/src/lib.rs`, `docs/decisions/README.md`.

**Interfaces:**
- Consumes: `crate::cache::Cache`, `crate::tmdb::api_key_from_env`, `serde_json`, `reqwest` (all already deps).
- Produces:
  - `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)] pub struct CatalogEntry { pub id: String, pub kind: CatalogKind, pub title: String, pub year: Option<i64>, pub synopsis: Option<String>, pub poster_path: Option<String>, pub vote_average: Option<f64> }`
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)] pub enum CatalogKind { Movie, Tv, Anime }`
  - `pub type DiscoverFetchFn = dyn Fn(CatalogKind, u32, &str) -> Result<String, String> + Send + Sync`
  - `pub struct DiscoverProvider { options: DiscoverOptions, fetch: Box<DiscoverFetchFn>, cache: Cache }` with `new()`, `with_fetch(...)`, `enabled()`, `discover(kind, page) -> Vec<CatalogEntry>`
  - `pub fn discover_url(kind: CatalogKind, page: u32) -> &'static str` — exact relative endpoint templates (see code).
  - `pub fn http_discover_fetch(kind, page, key) -> Result<String, String>` using `?api_key=` and fixed URL-free errors.

- [ ] **Step 1: ADR-0027 as Accepted (2026-11-22).**

```markdown
# ADR-0027: TMDB discovery catalogue (display-only)

- **Status:** Accepted (2026-11-22) — human: use TMDB for the catalogue too.
- **Related:** ADR-0026 (operator token), ADR-0005 (named commands), AGENTS.md (no content-source bundling)
- **Decision:** Movies/TV/Anime browse pages gain a Discover mode backed by TMDB's public discover endpoints, using the operator's token. Anime is approximated with `with_genres=16&with_original_language=ja`; TMDB has no dedicated anime type and coverage is incomplete. Results are display-only: never downloaded, played, or written into the local library. Nothing is bundled beyond the upstream provider client. Attribution and the existing artwork cache rules apply.
- **Revisit:** if the human wants a dedicated anime source (e.g. Jikan/AniList) or downloadable catalog entries, write a superseding ADR.
```

Index it in `docs/decisions/README.md`.

- [ ] **Step 2: Write failing tests** in `crates/metadata/tests/discover.rs` (fakes only):

```rust
use media_metadata::discover::{CatalogKind, DiscoverProvider};

const MOVIE_JSON: &str = r#"{"page":1,"total_results":2,"results":[
 {"id":1,"title":"Real Film","release_date":"2020-05-01","overview":"A story.","poster_path":"/p1.jpg","vote_average":7.2},
 {"id":2,"title":"Bad <b>title</b>\u0001","release_date":"not-a-date","overview":"x","poster_path":"http://evil/x","vote_average":99}]}"#;

#[test]
fn parses_and_sanitises_movie_page() {
    let provider = DiscoverProvider::with_fetch(Box::new(|_, _, _| Ok(MOVIE_JSON.to_string())));
    let entries = provider.discover(CatalogKind::Movie, 1);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].title, "Real Film");
    assert_eq!(entries[0].year, Some(2020));
    assert_eq!(entries[0].poster_path.as_deref(), Some("/p1.jpg"));
    assert!(entries[1].title.chars().all(|c| !c.is_control()));
    assert_eq!(entries[1].poster_path, None); // non-CDN path rejected
}

#[test]
fn disabled_provider_makes_no_fetch() {
    let provider = DiscoverProvider::with_options(DiscoverOptions {
        enabled: false,
        api_key: None,
        cache_dir: std::env::temp_dir().join("mh-discover-disabled-test"),
    });
    assert!(provider.discover(CatalogKind::Movie, 1).is_empty());
}

#[test]
fn anime_uses_japanese_animation_filters() {
    let url = media_metadata::discover::discover_url(CatalogKind::Anime, 2);
    assert!(url.contains("with_genres=16"));
    assert!(url.contains("with_original_language=ja"));
}
```

- [ ] **Step 3: Run to verify they fail.**

Run: `cargo test -p media-metadata --test discover`
Expected: compile errors naming the missing module/API.

- [ ] **Step 4: Make `sanitize_poster` public** in `crates/metadata/src/tmdb.rs` (currently `fn sanitize_poster`; change to `pub fn sanitize_poster`). This is a one-word visibility change so the discover module can reuse the existing CDN-path validation instead of duplicating it. Do not change its behaviour.

- [ ] **Step 5: Implement `crates/metadata/src/discover.rs`.**

```rust
//! Display-only TMDB discovery. Only the operator's key, a kind and a page
//! number are sent; no library path or content is included.

use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration;

use crate::cache::Cache;
use crate::model::{MAX_SYNOPSIS_CHARS, MAX_TITLE_CHARS};
use crate::tmdb::{api_key_from_env, sanitize_poster, DEFAULT_MAX_CACHE_BYTES};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CatalogKind { Movie, Tv, Anime }

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    pub id: String,
    pub kind: CatalogKind,
    pub title: String,
    pub year: Option<i64>,
    pub synopsis: Option<String>,
    pub poster_path: Option<String>,
    pub vote_average: Option<f64>,
}

pub type DiscoverFetchFn = dyn Fn(CatalogKind, u32, &str) -> Result<String, String> + Send + Sync;

pub fn discover_url(kind: CatalogKind, page: u32) -> &'static str {
    match kind {
        CatalogKind::Movie => "/3/discover/movie?sort_by=popularity.desc&page={page}",
        CatalogKind::Tv => "/3/discover/tv?sort_by=popularity.desc&page={page}",
        CatalogKind::Anime => "/3/discover/tv?with_genres=16&with_original_language=ja&sort_by=popularity.desc&page={page}",
    }
}

pub struct DiscoverOptions {
    pub enabled: bool,
    pub api_key: Option<String>,
    pub cache_dir: PathBuf,
}

pub struct DiscoverProvider {
    options: DiscoverOptions,
    fetch: Box<DiscoverFetchFn>,
    cache: Cache,
}

impl DiscoverProvider {
    pub fn new() -> Self {
        let api_key = api_key_from_env();
        Self::with_options(DiscoverOptions {
            enabled: api_key.is_some(),
            api_key,
            cache_dir: std::env::temp_dir().join("media-haven-discover-cache"),
        })
    }

    pub fn with_options(options: DiscoverOptions) -> Self {
        Self::with_fetch(options, Box::new(http_discover_fetch))
    }

    pub fn with_fetch(options: DiscoverOptions, fetch: Box<DiscoverFetchFn>) -> Self {
        let cache = Cache::new(options.cache_dir.clone(), DEFAULT_MAX_CACHE_BYTES);
        Self { options, fetch, cache }
    }

    pub fn enabled(&self) -> bool { self.options.enabled }

    pub fn discover(&self, kind: CatalogKind, page: u32) -> Vec<CatalogEntry> {
        if !self.options.enabled { return Vec::new(); }
        let Some(key) = self.options.api_key.as_deref().filter(|k| !k.trim().is_empty()) else { return Vec::new() };
        let cache_key = format!("discover|{}|{}", kind as u8, page);
        let json = self.cache.get(&cache_key)
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .or_else(|| (self.fetch)(kind, page, key).ok())
            .unwrap_or_default();
        let entries = parse_page(kind, &json);
        if !entries.is_empty() && self.cache.get(&cache_key).is_none() {
            let _ = self.cache.put(&cache_key, json.as_bytes());
        }
        entries
    }
}

pub fn http_discover_fetch(kind: CatalogKind, page: u32, key: &str) -> Result<String, String> {
    let template = discover_url(kind, page).replace("{page}", &page.to_string());
    let url = format!("https://api.themoviedb.org{template}&api_key={key}");
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|_| "client unavailable".to_string())?;
    client.get(url).send().map_err(|_| "discover request failed".to_string())?
        .error_for_status().map_err(|_| "discover request failed".to_string())?
        .text().map_err(|_| "discover response unreadable".to_string())
}

fn clean(raw: &str, max: usize) -> Option<String> {
    let text: String = raw.chars().filter(|c| !c.is_control()).collect();
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() { None } else { Some(text.chars().take(max).collect()) }
}

fn parse_page(kind: CatalogKind, json: &str) -> Vec<CatalogEntry> {
    let value: Value = match serde_json::from_str(json) { Ok(v) => v, Err(_) => return Vec::new() };
    let results = match value.get("results").and_then(Value::as_array) { Some(r) => r, None => return Vec::new() };
    results.iter().filter_map(|hit| {
        let title_field = if matches!(kind, CatalogKind::Movie) { "title" } else { "name" };
        let title = clean(hit.get(title_field)?.as_str()?, MAX_TITLE_CHARS)?;
        let year = hit.get("release_date").or_else(|| hit.get("first_air_date"))
            .and_then(Value::as_str).and_then(|d| d.get(0..4))
            .and_then(|p| p.parse::<i64>().ok()).filter(|y| (1870..=2200).contains(y));
        Some(CatalogEntry {
            id: hit.get("id")?.as_i64()?.to_string(),
            kind,
            title,
            year,
            synopsis: hit.get("overview").and_then(Value::as_str).and_then(|s| clean(s, MAX_SYNOPSIS_CHARS)),
            poster_path: hit.get("poster_path").and_then(Value::as_str).and_then(sanitize_poster),
            vote_average: hit.get("vote_average").and_then(Value::as_f64).filter(|v| (0.0..=10.0).contains(v)),
        })
    }).take(20).collect()
}
```

`shortcut: poster_path is re-checked with the TMDB sanitiser, so a hostile payload cannot become a path or URL; upgrade only if another artwork source is added.`

- [ ] **Step 6: Re-export from `crates/metadata/src/lib.rs`.**

```rust
pub use discover::{http_discover_fetch, CatalogEntry, CatalogKind, DiscoverOptions, DiscoverProvider};
```

- [ ] **Step 7: Verify Task 1.**

Run: `cargo test -p media-metadata --test discover && cargo fmt --all -- --check && cargo clippy -p media-metadata --all-targets -- -D warnings`
Expected: PASS; no network.

---

## Task 2: Command, DTOs, route and poster caching

**Files:** modify `crates/contract/src/lib.rs`, `crates/app-core/src/domain/media.rs`, `crates/core/src/routes.rs`, `crates/core/src/state.rs`, `crates/core/src/server.rs`, `apps/desktop/src-tauri/src/commands/renderer.rs`, `apps/desktop/src-tauri/src/lib.rs`.

**Interfaces:**
- Consumes: `DiscoverProvider`, `CatalogEntry`, `CatalogKind` from Task 1; existing `artwork_key`, `fetch_artwork_bytes`, `ArtworkCache`, `DEFAULT_MAX_CACHE_BYTES`, `TMDB_ATTRIBUTION`.
- Produces: command `discover_catalog`, DTO `CatalogScreen { available: bool, entries: Vec<CatalogEntryDto> }` where `CatalogEntryDto` adds `artwork: Vec<ArtworkRef>`.

- [ ] **Step 1: Extend the contract.**

In `crates/contract/src/lib.rs`:
- Add `"discover_catalog"` to `COMMANDS` (25 total).
- Add `DiscoverCatalog` to `Command`, its `as_str()` arm `"discover_catalog"`, and its `FromStr` arm.

- [ ] **Step 2: Add DTOs in `crates/app-core/src/domain/media.rs`.**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CatalogKindDto { Movie, Tv, Anime }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntryDto {
    pub id: String,
    pub kind: CatalogKindDto,
    pub title: String,
    pub year: Option<i64>,
    pub synopsis: Option<String>,
    pub rating: Option<f64>,
    pub artwork: Vec<ArtworkRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogScreen {
    pub available: bool,
    pub entries: Vec<CatalogEntryDto>,
}
```

- [ ] **Step 3: Wire provider into `AppState`.** In `crates/core/src/state.rs`:

```rust
use media_metadata::{DiscoverOptions, DiscoverProvider};

pub discover: Arc<DiscoverProvider>,
...
discover: Arc::new(DiscoverProvider::with_options(DiscoverOptions {
    enabled: false, api_key: None,
    cache_dir: std::env::temp_dir().join("media-haven-discover-cache"),
})),
...
pub fn with_discover(mut self, provider: DiscoverProvider) -> Self { self.discover = Arc::new(provider); self }
```

In `crates/core/src/server.rs`, before state construction:

```rust
let discover = DiscoverProvider::with_options(DiscoverOptions {
    enabled: media_metadata::tmdb::api_key_from_env().is_some(),
    api_key: media_metadata::tmdb::api_key_from_env(),
    cache_dir: config.data_dir.join("discover-cache"),
});
```

then `.with_discover(discover)`.

- [ ] **Step 4: Add the route handler in `crates/core/src/routes.rs`.**

Add request type and route:

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DiscoverArgs { kind: String, page: u32 }
```

In `protected_router()`: `.route("/discover_catalog", post(discover_catalog))`.

Handler (one `spawn_blocking` for the batch):

```rust
async fn discover_catalog(
    State(state): State<AppState>,
    ApiJson(args): ApiJson<DiscoverArgs>,
) -> Result<Json<CatalogScreen>, ApiError> {
    let kind = match args.kind.as_str() {
        "movie" => CatalogKind::Movie,
        "tv" => CatalogKind::Tv,
        "anime" => CatalogKind::Anime,
        _ => return Err(invalid("unknown catalogue kind")),
    };
    let page = args.page.clamp(1, 20);
    let available = state.discover.enabled();
    if !available {
        return Ok(Json(CatalogScreen { available: false, entries: Vec::new() }));
    }
    let discover = state.discover.clone();
    let artwork_dir = state.artwork_dir.clone();
    let entries = tokio::task::spawn_blocking(move || {
        let cache = media_metadata::ArtworkCache::new(
            artwork_dir.as_ref().clone(),
            media_metadata::tmdb::DEFAULT_MAX_CACHE_BYTES,
        );
        discover.discover(kind, page).into_iter().map(|entry| {
            let mut artwork = Vec::new();
            if let Some(path) = entry.poster_path {
                let key = media_metadata::artwork_key(&path);
                if cache.get(&key).is_none() {
                    if let Ok((bytes, content_type)) = media_metadata::fetch_artwork_bytes(&path) {
                        let _ = cache.put(&key, &bytes, &content_type);
                    }
                }
                if cache.get(&key).is_some() {
                    artwork.push(ArtworkRef {
                        variant: ArtworkVariant::Poster,
                        url: Some(format!("/artwork/{key}")),
                        alt: format!("{} poster", entry.title),
                    });
                }
            }
            CatalogEntryDto {
                id: entry.id,
                kind: match entry.kind {
                    CatalogKind::Movie => CatalogKindDto::Movie,
                    CatalogKind::Tv => CatalogKindDto::Tv,
                    CatalogKind::Anime => CatalogKindDto::Anime,
                },
                title: entry.title,
                year: entry.year,
                synopsis: entry.synopsis,
                rating: entry.vote_average,
                artwork,
            }
        }).collect::<Vec<_>>()
    }).await.unwrap_or_default();
    Ok(Json(CatalogScreen { available: true, entries }))
}
```

Keep the existing `GET /artwork/:key` 64-hex validation unchanged — it already serves these hashed URLs safely.

- [ ] **Step 5: Add the Tauri command.** In `apps/desktop/src-tauri/src/commands/renderer.rs`:

```rust
#[tauri::command]
pub fn discover_catalog(kind: &str, page: u32) -> Result<Value, CommandError> {
    post("discover_catalog", json!({ "kind": kind, "page": page }))
}
```

Register `commands::discover_catalog` in `generate_handler!` in `lib.rs`.

- [ ] **Step 6: Write a failing route test** in `routes.rs` using `with_discover(DiscoverProvider::with_fetch(...))` with a fake JSON page and a pre-seeded JPEG cache file; assert the DTO has a relative hashed URL and contains no `image.tmdb.org`, no poster path and no API key.

- [ ] **Step 7: Verify Task 2.**

Run: `cargo fmt --all -- --check && cargo clippy -p mediahaven-contract -p mediahaven-core --all-targets -- -D warnings && cargo test -p mediahaven-contract -p mediahaven-core`
Expected: PASS; parity test now reports 25 commands.

---

## Task 3: Frontend discover mode, attribution and docs

**Files:** modify `apps/desktop/src/services/generated.ts`, `types.ts`, `api.ts`; create `apps/desktop/src/components/catalog/CatalogShelf.tsx` and its CSS module + test; modify `MoviesPage.tsx`, `ShowsPage.tsx`, `AnimePage.tsx`, `roadmap.md`, `progress.md`.

- [ ] **Step 1: Update the generated command union.**

Add `"discover_catalog"` to `CommandName` in `apps/desktop/src/services/generated.ts`.

- [ ] **Step 2: Add DTOs and API method.**

In `types.ts`:

```ts
export type CatalogKind = "movie" | "tv" | "anime";
export interface CatalogEntry {
  id: string;
  kind: CatalogKind;
  title: string;
  year: number | null;
  synopsis: string | null;
  rating: number | null;
  artwork: ArtworkRef[];
}
export interface CatalogScreen { available: boolean; entries: CatalogEntry[]; }
```

In `api.ts`:

```ts
discover: (kind: CatalogKind, page = 1) =>
  call<CatalogScreen>("discover_catalog", { kind, page }),
```

- [ ] **Step 3: Write a failing component test** for `CatalogShelf`: given a fake `CatalogScreen` with one entry whose artwork URL is `/artwork/<64-hex>`, it renders the title, a poster `<img>` whose src starts with `apiBase`, and the exact TMDB attribution text; given `available: false`, it renders the "add a TMDB token" message and no cards.

- [ ] **Step 4: Implement `CatalogShelf.tsx`.**

Use existing tokens/CSS module patterns. Each card is display-only (no `Link` to `/details`, because catalog entries are not in the local library). Render `artwork[0].url` with `apiBase` prefix and a guarded `onError` that hides the image; show title, year and rating when present. Render `TMDB_ATTRIBUTION_FRONTEND` from `components/cards/TmdbAttribution` beneath the grid. `available: false` → a quiet message pointing to `/settings`. Loading and empty states must not break the page.

- [ ] **Step 5: Add a Discover toggle to the three browse pages.**

Reuse `Segmented` from `components/ui/Controls` with options `My library` / `Discover`. On `Discover`, call `api.discover(kind)` in a `useEffect`, hold result in local state, and render `<CatalogShelf kind={...} screen={...} />`. Do not change `BrowseView` itself. Anime page description must say the catalogue is TMDB's Japanese-animation filter and may miss anime; keep its existing library filter for the local mode.

- [ ] **Step 6: Docs.** Add a `roadmap.md` P6-09 `[~]` row and a `progress.md` entry naming observed gates and "live catalogue with token NOT RUN"; note Anime coverage is an approximation.

- [ ] **Step 7: Verify Task 3.**

Run from `apps/desktop`: `npm run typecheck && npm run lint && npm test -- --run && npm run check:tokens && npm run build && npm run check:bundle`
Expected: PASS.

---

## Parallel-execution note

Subagent-Driven Development forbids multiple implementation agents writing the same checkout at once. Parallel execution here means: batch same-shape edits into one dispatch (Task 2's four command-surface files are one batch), and run independent review/seeding work concurrently where it does not touch files. Tasks 1 → 2 → 3 remain sequential because each consumes the previous task's exact interfaces.

## Self-review (completed by planner)

- **Spec coverage:** movies/TV/anime catalogue with poster images (Tasks 1–3), no key beyond ADR-0026, no bundling, display-only, attribution, cache/caps, honest NOT RUN — all covered.
- **Placeholder scan:** no TBD/TODO; every code step has concrete code. The one codebase-specific detail (Tauri handler registration) is named with its exact file.
- **Type consistency:** `CatalogKind`, `CatalogEntry`, `DiscoverProvider`, `discover_catalog`, `CatalogScreen`, `CatalogEntryDto`, `CatalogKindDto`, `api.discover`, `CatalogShelf` keep one spelling across tasks. DTO URL stays `/artwork/<64-hex>` in Rust and TS.
