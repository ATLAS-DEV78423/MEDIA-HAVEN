# TMDB Artwork Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Populate real movie/show posters from TMDB into the local library UI, off by default, cached on disk, attributed, and fully offline once cached.

**Architecture:** The existing `TmdbProvider` in `crates/metadata` already implements the opt-in gate, key check, sanitisation, 30-day text cache and `poster_path` parsing; only the HTTPS transport is a deliberate `unconfigured_fetch` seam. This plan fills that seam with an injected `reqwest` blocking client (called via `spawn_blocking` so no tokio worker blocks), adds a small hashed image cache under the core data dir, serves cached bytes from a public loopback `GET /artwork/:key` route keyed by an opaque SHA-256 hex digest (never a path), enriches the already-existing `artwork: Vec<ArtworkRef>` DTO field at the route layer, and makes the frontend prefer that URL over the bundled placeholder art. No scraper, indexer or content source is bundled; the API key comes from the ADR-0007 secret store via `MEDIAHAVEN_TMDB_API_KEY`.

**Tech Stack:** Rust (reqwest 0.13.5 with rustls + blocking; reqwest and the rustls stack are already in `Cargo.lock`, though enabling its features may add a few small transitive crates such as `hyper-rustls`; sha2 and uuid from the workspace), axum, React 19 + Vitest. No new npm dependency.

**Spec:** ADR-0011 (TMDB primary, opt-in/off-by-default, attribution, size-capped cache), ADR-0007 (secrets injected from Credential Manager, never in repo/log), ADR-0005 (renderer commands are named and path-free), `Development.md` §4.3 and §7.4.12, `AGENTS.md` (no new dependency without ADR; no commit without instruction).

## Global Constraints

- **OPT-IN ONLY, off by default.** `MEDIAHAVEN_TMDB_OPTIN=1` AND a non-blank `MEDIAHAVEN_TMDB_API_KEY` must both be present. Either absent ⇒ zero requests, zero file reads, `artwork` stays empty, bundled art renders.
- Only a **title, year and the poster CDN path** leave the machine. Never a filesystem path, folder name, library content, token or host detail.
- **No bundled content source, indexer, catalogue, scraper or downloader.** This is metadata artwork only.
- No new dependency without an ADR (Task 1 writes ADR-0023 first). reqwest is already locked at 0.13.5; use rustls, never native-tls/OpenSSL (the core ships in a Linux container).
- **No path or secret in renderer DTOs.** DTO carries a relative URL (`/artwork/<64-hex>`) only; the frontend prefixes `apiBase`.
- Attribution string is exact: `"This product uses the TMDB API but is not endorsed or certified by TMDB."`
- Cache: 30-day text TTL (existing), 2 MiB per image, 64 MiB total image cap, oldest-first eviction. Failure degrades to bundled art, never breaks a view.
- `PASS` / `FAIL` / `NOT RUN` describe observed runs. **No live TMDB call in any test** — fake fetch only. A live call is NOT RUN until the operator supplies a key and opts in.
- Playwright config, `vite.config.ts`, WSL2/Docker/host settings and git commits/pushes are untouched.

---

## File Structure

| File | Responsibility |
|---|---|
| `docs/decisions/0023-tmdb-artwork-transport.md` | New ADR: reqwest dependency, loopback `img-src` widening, opt-in and cache limits. |
| `Cargo.toml`, `crates/metadata/Cargo.toml` | Declare `reqwest` + `sha2` for the metadata crate. |
| `crates/metadata/src/tmdb.rs` | Fill `unconfigured_fetch`, add image fetch + hashed `ArtworkCache`. |
| `crates/metadata/tests/tmdb.rs` | Extend with transport/URL, image type-check, byte-cap, cache and eviction tests (fakes only). |
| `crates/core/src/state.rs` | Add opt-in TMDB provider + artwork dir to `AppState`. |
| `crates/core/src/server.rs` | Wire provider and artwork dir from config. |
| `crates/core/src/routes.rs` | Public `GET /artwork/:key`; enrich `artwork` DTOs via one `spawn_blocking` batch. |
| `apps/desktop/src-tauri/tauri.conf.json` | Add loopback origin to CSP `img-src`. |
| `apps/desktop/src/services/adapt.ts` | Prefer backend poster URL over `artFor`; keep bundled fallback. |
| `apps/desktop/src/components/cards/TmdbAttribution.tsx` | Small attributed caption, rendered where TMDB art is shown. |
| `roadmap.md`, `progress.md` | Record task status and evidence. |

---

## Task 1: ADR + TMDB transport and image cache

**Files:**
- Create: `docs/decisions/0023-tmdb-artwork-transport.md`
- Modify: `Cargo.toml`, `crates/metadata/Cargo.toml`, `crates/metadata/src/tmdb.rs`, `crates/metadata/src/lib.rs`, `crates/metadata/tests/tmdb.rs`

**Interfaces:**
- Consumes: existing `TmdbOptions { enabled, api_key, cache_dir, max_bytes }`, `FetchFn = dyn Fn(&str, Option<i64>, &str) -> Result<String, String> + Send + Sync`, `TmdbLookup { record, poster_path }`, `parse_result`, `sanitize_poster`, `Cache`.
- Produces:
  - `pub fn http_search_fetch() -> Box<FetchFn>` — real v3 search transport (still never used unless the provider is enabled and a key injected).
  - `pub struct ArtworkCache` with `new(dir: PathBuf, max_bytes: u64)`, `get(&self, key: &str) -> Option<(Vec<u8>, String)>`, `put(&self, key: &str, bytes: &[u8], content_type: &str) -> std::io::Result<()>`, and `pub fn artwork_key(poster_path: &str) -> String` returning 64 lowercase hex chars.
  - `pub fn fetch_artwork_bytes(poster_path: &str) -> Result<(Vec<u8>, String), String>` — downloads `https://image.tmdb.org/t/p/w500<poster_path>`, caps at 2 MiB, detects JPEG/PNG magic bytes, returns `(bytes, "image/jpeg" | "image/png")`.
  - Re-export `ArtworkCache`, `artwork_key`, `fetch_artwork_bytes`, `http_search_fetch` from `crates/metadata/src/lib.rs`.

- [ ] **Step 1: Write ADR-0023 (status Proposed).**

```markdown
# ADR-0023: TMDB artwork transport and loopback image serving

- **Status:** Proposed
- **Date:** 2026-11-21
- **Related:** ADR-0011 (TMDB primary, opt-in), ADR-0007 (secrets), AGENTS.md

## Context
ADR-0011 accepted TMDB for posters but deliberately left the HTTPS transport
unimplemented: no ADR permitted an HTTP client crate. The renderer also needs a
way to display cached images: its CSP currently lists only `asset:` and `data:`
in `img-src`, while `connect-src` already allows `http://127.0.0.1:8080`.

## Decision
- Add `reqwest` 0.13.5 (already in Cargo.lock; rustls TLS + blocking features,
  never native-tls) as a workspace dependency of `crates/metadata` only. Blocking
  calls are wrapped in `tokio::task::spawn_blocking` by `core` so no async worker
  blocks.
- Serve cached artwork from a public `GET /artwork/:key` route on the existing
  loopback listener. `:key` is the SHA-256 hex of the sanitised TMDB poster
  path, so it is opaque and cannot traverse a path. Image bytes are public TMDB
  content; no token or host detail is exposed.
- Widen the Tauri CSP `img-src` by exactly one origin, `http://127.0.0.1:8080`,
  matching the origin already allowed in `connect-src`.
- Opt-in stays off unless `MEDIAHAVEN_TMDB_OPTIN=1` and a non-blank key from the
  ADR-0007 store is present. Only title, year and poster path leave the machine.
- Per image: 2 MiB cap, JPEG/PNG magic-byte check, SHA-256 key, 64 MiB total
  cache with oldest-first eviction. TMDB attribution is rendered wherever its
  art appears.

## Consequences
Positive: real posters with zero runtime scraper, offline after first cache.
Negative: an HTTP client and a loopback image origin are now in the trust
surface; both are bounded by opt-in, loopback bind and opaque keys. Live calls
remain NOT RUN until an operator supplies a key and opts in.
```

- [ ] **Step 2: Confirm the locked reqwest rustls feature name.**

Run: `$p = Get-ChildItem "$env:USERPROFILE\.cargo\registry\src\*\reqwest-0.13.5\Cargo.toml" | Select-Object -First 1; Select-String -Path $p -Pattern '^rustls-tls|^blocking' | ForEach-Object { $_.Line }`

Expected: lines naming `rustls-tls` (or `rustls-tls-webpki-roots`) and `blocking`. Use the exact `rustls` feature name printed in the next step.

- [ ] **Step 3: Declare the dependencies.**

In workspace `Cargo.toml` `[workspace.dependencies]`, add (substituting the feature name from Step 2 if it differs):

```toml
reqwest = { version = "0.13", default-features = false, features = ["rustls-tls", "blocking"] }
```

In `crates/metadata/Cargo.toml` `[dependencies]`, add:

```toml
reqwest.workspace = true
sha2.workspace = true
```

- [ ] **Step 4: Write failing tests** in `crates/metadata/tests/tmdb.rs` (fake data only, no network):

```rust
#[test]
fn artwork_key_is_opaque_sha256_hex() {
    let key = media_metadata::tmdb::artwork_key("/abc123.jpg");
    assert_eq!(key.len(), 64);
    assert!(key.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    assert_ne!(key, media_metadata::tmdb::artwork_key("/other.jpg"));
}

#[test]
fn image_bytes_are_type_checked_and_capped() {
    // A fake transport is injected via fetch_artwork_bytes_with(fetch) in Step 5.
    let jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
    assert_eq!(classify_image(&jpeg), Some("image/jpeg"));
    assert_eq!(classify_image(&[0x89, 0x50, 0x4E, 0x47]), Some("image/png"));
    assert_eq!(classify_image(b"<script>alert(1)</script>"), None);
    assert!(too_large(2 * 1024 * 1024 + 1));
}

#[test]
fn artwork_cache_stores_and_evicts_oldest_first() {
    let dir = std::env::temp_dir().join(format!("mh-art-{}", uuid::Uuid::new_v4()));
    let cache = ArtworkCache::new(dir.clone(), 8); // tiny cap forces eviction
    cache.put("aaa", b"12345", "image/jpeg").unwrap();
    cache.put("bbb", b"12345", "image/jpeg").unwrap();
    assert!(cache.get("aaa").is_none()); // oldest evicted
    let (bytes, kind) = cache.get("bbb").expect("kept entry");
    assert_eq!(bytes, b"12345");
    assert_eq!(kind, "image/jpeg");
    let _ = std::fs::remove_dir_all(dir);
}
```

Add `uuid.workspace = true` to `crates/metadata/Cargo.toml` dev-dependencies if it is not already there for tests.

- [ ] **Step 5: Run tests to verify they fail.**

Run: `cargo test -p media-metadata --test tmdb`
Expected: compile errors naming `artwork_key`, `classify_image`, `too_large`, `ArtworkCache` as missing.

- [ ] **Step 6: Implement in `crates/metadata/src/tmdb.rs`.**

Add imports:

```rust
use sha2::{Digest, Sha256};
```

Append these items (the real search transport; the byte cap and type check are pure helpers so tests need no network):

```rust
/// Real TMDB v3 search transport. Only title/year/key are used; no path.
pub fn http_search_fetch() -> Box<FetchFn> {
    Box::new(|title, year, key| {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|err| err.to_string())?;
        let mut request = client
            .get("https://api.themoviedb.org/3/search/movie")
            .query(&[("api_key", key), ("query", title)]);
        if let Some(year) = year {
            request = request.query(&[("year", year.to_string())]);
        }
        let response = request.send().map_err(|err| err.to_string())?;
        response.error_for_status().map_err(|err| err.to_string())?.text().map_err(|err| err.to_string())
    })
}

/// 2 MiB per poster; anything larger is refused before it is cached.
pub const MAX_ARTWORK_BYTES: usize = 2 * 1024 * 1024;

fn too_large(len: usize) -> bool { len > MAX_ARTWORK_BYTES }

pub fn artwork_key(poster_path: &str) -> String {
    let digest = Sha256::digest(poster_path.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Accept only real JPEG/PNG headers; never cache HTML or script as an image.
fn classify_image(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) { Some("image/jpeg") }
    else if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) { Some("image/png") }
    else { None }
}

/// Download a poster from the TMDB image CDN. Pure URL construction from the
/// already-sanitised `poster_path`; no path from the local filesystem is used.
pub fn fetch_artwork_bytes(poster_path: &str) -> Result<(Vec<u8>, String), String> {
    let path = sanitize_poster(poster_path).ok_or_else(|| "invalid poster path".to_string())?;
    let url = format!("https://image.tmdb.org/t/p/w500{path}");
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|err| err.to_string())?;
    let response = client.get(&url).send().map_err(|err| err.to_string())?;
    let bytes = response.error_for_status().map_err(|err| err.to_string())?.bytes().map_err(|err| err.to_string())?;
    if too_large(bytes.len()) { return Err("poster exceeds size cap".to_string()); }
    let kind = classify_image(&bytes).ok_or_else(|| "poster is not an image".to_string())?;
    Ok((bytes.to_vec(), kind.to_string()))
}

/// Hashed, size-capped image cache under the core data dir.
pub struct ArtworkCache {
    dir: PathBuf,
    max_bytes: u64,
}

impl ArtworkCache {
    pub fn new(dir: PathBuf, max_bytes: u64) -> Self { Self { dir, max_bytes } }

    pub fn get(&self, key: &str) -> Option<(Vec<u8>, String)> {
        let path = self.dir.join(key);
        let bytes = std::fs::read(&path).ok()?;
        let kind = classify_image(&bytes)?.to_string();
        // Reading updates recency so eviction is genuinely oldest-first.
        let _ = std::fs::File::open(&path).and_then(|file| file.set_modified(std::time::SystemTime::now()));
        Some((bytes, kind))
    }

    pub fn put(&self, key: &str, bytes: &[u8], content_type: &str) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        if classify_image(bytes).is_none() { return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "not an image")); }
        let path = self.dir.join(key);
        std::fs::write(&path, bytes)?;
        let used = std::fs::read_dir(&self.dir)?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| entry.metadata().ok().map(|meta| (entry.path(), meta.len(), meta.modified().ok())))
            .fold(0u64, |sum, (_, len, _)| sum.saturating_add(len));
        if used > self.max_bytes {
            let mut entries: Vec<_> = std::fs::read_dir(&self.dir)?
                .filter_map(|entry| entry.ok())
                .filter_map(|entry| entry.metadata().ok().map(|meta| (entry.path(), meta.modified().ok(), meta.len())))
                .collect();
            entries.sort_by_key(|(_, modified, _)| *modified);
            for (path, _, len) in entries {
                if used <= self.max_bytes { break; }
                if std::fs::remove_file(&path).is_ok() { used = used.saturating_sub(len); }
            }
        }
        let _ = content_type; // content type is re-derived from magic bytes on read
        Ok(())
    }
}
```

`shortcut: content_type is re-derived from magic bytes on read, so the stored file needs no metadata sidecar; upgrade only if non-image formats are added.`

- [ ] **Step 7: Re-export from `crates/metadata/src/lib.rs`.**

```rust
pub use tmdb::{
    artwork_key, fetch_artwork_bytes, http_search_fetch, ArtworkCache, TmdbLookup, TmdbOptions,
    TmdbProvider, MAX_ARTWORK_BYTES, TMDB_ATTRIBUTION,
};
```

- [ ] **Step 8: Run tests and verify they pass.**

Run: `cargo test -p media-metadata --test tmdb`
Expected: all previous tmdb tests plus the three new ones PASS; no network access.

- [ ] **Step 9: Verify formatting and lint for the crate.**

Run: `cargo fmt --all && cargo clippy -p media-metadata --all-targets -- -D warnings`
Expected: exit 0.

---

## Task 2: Core wiring — opt-in provider, image route, DTO enrichment

**Files:**
- Modify: `crates/core/src/state.rs`, `crates/core/src/server.rs`, `crates/core/src/routes.rs`

**Interfaces:**
- Consumes: `TmdbProvider::lookup_full(title, year) -> Option<TmdbLookup>` (returns `{ record, poster_path }`); `api_key_from_env()`, `http_search_fetch()`, `ArtworkCache::new`, `artwork_key`, `fetch_artwork_bytes` from Task 1; `server::default_data_dir()`.
- Produces:
  - `AppState::tmdb: Arc<TmdbProvider>`, `AppState::artwork_dir: Arc<PathBuf>`
  - `AppState::with_tmdb(self, provider: TmdbProvider) -> Self`, `AppState::with_artwork_dir(self, dir: PathBuf) -> Self`
  - `state::tmdb_provider_from_env(data_dir: &Path) -> TmdbProvider`
  - `enrich_artwork(&AppState, Vec<MediaSummary>) -> Vec<MediaSummary>` (internal)
  - Public route `GET /artwork/:key` returning image bytes or 404.

- [ ] **Step 1: Write a failing state test** in `crates/core/src/state.rs`:

```rust
#[test]
fn default_state_has_a_disabled_tmdb_provider() {
    let state = AppState::new(service_for_test(), "test-token");
    assert!(!state.tmdb.options().enabled);
    assert!(state.tmdb.lookup_full("Anything", None).is_none());
}
```

(`service_for_test()` is the existing test helper used by the IMDb state test; reuse it.)

- [ ] **Step 2: Run it to verify it fails.**

Run: `cargo test -p mediahaven-core state::tests::default_state_has_a_disabled_tmdb_provider`
Expected: compile error — no field `tmdb` on `AppState`.

- [ ] **Step 3: Extend `AppState` in `crates/core/src/state.rs`.**

```rust
use std::path::Path;
use media_metadata::{api_key_from_env, http_search_fetch, TmdbOptions, TmdbProvider};

pub const TMDB_OPTIN_ENV: &str = "MEDIAHAVEN_TMDB_OPTIN";

#[derive(Clone)]
pub struct AppState {
    // ...existing fields unchanged...
    pub imdb: Arc<ImdbProvider>,
    /// Opt-in TMDB provider. `new` builds it disabled; the server attaches the
    /// operator-configured one via `tmdb_provider_from_env`.
    pub tmdb: Arc<TmdbProvider>,
    /// Directory of hashed, cached poster bytes. Set by the server; defaults
    /// to a temp dir so existing `AppState::new(...)` test callers still work.
    pub artwork_dir: Arc<PathBuf>,
}

impl AppState {
    pub fn new(service: LibraryServiceHandle, token: impl Into<String>) -> Self {
        Self {
            // ...existing fields unchanged...
            tmdb: Arc::new(TmdbProvider::new(TmdbOptions::default())),
            artwork_dir: Arc::new(std::env::temp_dir().join("media-haven-artwork")),
        }
    }

    pub fn with_tmdb(mut self, provider: TmdbProvider) -> Self {
        self.tmdb = Arc::new(provider);
        self
    }

    pub fn with_artwork_dir(mut self, dir: PathBuf) -> Self {
        self.artwork_dir = Arc::new(dir);
        self
    }
}

/// Build the TMDB provider only when the operator opted in AND supplied a key.
pub fn tmdb_provider_from_env(data_dir: &Path) -> TmdbProvider {
    let opted_in = std::env::var(TMDB_OPTIN_ENV).as_deref() == Ok("1");
    let (enabled, api_key) = match (opted_in, api_key_from_env()) {
        (true, Some(key)) => (true, Some(key)),
        _ => (false, None),
    };
    TmdbProvider::with_fetch(
        TmdbOptions {
            enabled,
            api_key,
            cache_dir: data_dir.join("tmdb-cache"),
            max_bytes: media_metadata::tmdb::DEFAULT_MAX_CACHE_BYTES,
        },
        http_search_fetch(),
    )
}
```

- [ ] **Step 4: Wire provider and artwork dir in `crates/core/src/server.rs`.**

Replace the state construction line:

```rust
let artwork_dir = config.data_dir.join("artwork");
let state = AppState::new(service, config.token)
    .with_imdb(crate::state::imdb_provider_from_env())
    .with_tmdb(crate::state::tmdb_provider_from_env(&config.data_dir))
    .with_artwork_dir(artwork_dir);
```

- [ ] **Step 5: Add the public image route in `crates/core/src/routes.rs`.**

Add to `public_router()`:

```rust
Router::new()
    .route("/healthz", get(healthz))
    .route("/artwork/:key", get(artwork));
```

Add the handler:

```rust
/// Serve a cached poster. `:key` is a 64-char SHA-256 hex digest, never a path,
/// so a request cannot traverse the artwork directory. Public by design: the
/// bytes are public TMDB images and the listener is loopback-only.
async fn artwork(State(state): State<AppState>, Path(key): Path<String>) -> Response {
    let valid = key.len() == 64 && key.bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase());
    if !valid {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let path = state.artwork_dir.as_ref().join(&key);
    let bytes = match tokio::fs::read(&path).await {
        Ok(bytes) => bytes,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    let content_type = if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) { "image/jpeg" } else { "image/png" };
    (
        [(header::CONTENT_TYPE, content_type), (header::CACHE_CONTROL, "public, max-age=604800")],
        bytes,
    )
        .into_response()
}
```

Add `use axum::http::header;` at the top of `routes.rs`.

- [ ] **Step 6: Add the batch enrichment helper in `crates/core/src/routes.rs`.**

```rust
/// Fill the existing `artwork` DTO field with a relative, opaque poster URL.
/// One `spawn_blocking` for the whole batch: the TMDB provider and image cache
/// are synchronous and may do disk I/O, but no tokio worker may block.
async fn enrich_artwork(state: &AppState, mut items: Vec<MediaSummary>) -> Vec<MediaSummary> {
    if !state.tmdb.options().enabled {
        return items;
    }
    let tmdb = state.tmdb.clone();
    let artwork_dir = state.artwork_dir.clone();
    let filled = tokio::task::spawn_blocking(move || {
        let cache = media_metadata::ArtworkCache::new(
            artwork_dir.as_ref().clone(),
            media_metadata::tmdb::DEFAULT_MAX_CACHE_BYTES,
        );
        for item in &mut items {
            let Some(found) = tmdb.lookup_full(&item.title, item.year) else { continue };
            let Some(poster_path) = found.poster_path else { continue };
            let key = media_metadata::artwork_key(&poster_path);
            if cache.get(&key).is_none() {
                let Ok((bytes, kind)) = media_metadata::fetch_artwork_bytes(&poster_path) else { continue };
                if cache.put(&key, &bytes, &kind).is_err() { continue; }
            }
            item.artwork.push(ArtworkRef {
                variant: ArtworkVariant::Poster,
                url: Some(format!("/artwork/{key}")),
                alt: format!("{} poster", item.title),
            });
        }
        items
    })
    .await;
    filled.unwrap_or_else(|_| items)
}
```

- [ ] **Step 7: Apply enrichment at the four library handlers that feed artwork.**

In `library_home`, after the service call returns the `HomeScreen`, map the hero media and every shelf's `items`:

```rust
let mut screen = state.service.home().await?.into_response_model(); // existing call, unchanged
screen.hero = match screen.hero {
    Some(mut hero) => {
        hero.media = enrich_artwork(&state, vec![hero.media]).await.remove(0);
        Some(hero)
    }
    None => None,
};
for shelf in &mut screen.shelves {
    shelf.items = enrich_artwork(&state, std::mem::take(&mut shelf.items)).await;
}
```

> Use the actual existing service-call variable name in each handler; the snippet shows the wrapping pattern, not a replacement for the handler body.

Apply the same `enrich_artwork` call to:
- `library_movies` — map the returned `Vec<MediaSummary>`.
- `library_shows` — map the returned `Vec<SeriesDetail>` by adapting the helper to accept `SeriesDetail` (it has `title`, `year` and `artwork` fields) or by calling the summary helper on a `MediaSummary` built from each series; keep one helper signature by adding `impl IntoIterator<Item = &mut ArtworkRef>` only if the compiler requires it. The smallest change is a second wrapper `enrich_series_artwork` that mirrors the loop and writes `detail.artwork`.
- `library_detail` — enrich the returned `MediaDetail` (movie or series) before serialising.
- `library_search` — enrich `results.items`.

- [ ] **Step 8: Write a failing route test** in `crates/core/src/routes.rs` (no network):

```rust
#[tokio::test]
async fn artwork_route_rejects_non_hash_keys() {
    let app = crate::server::build_router(AppState::new(service(), "test-token"));
    let response = app.oneshot(
        axum::http::Request::builder().uri("/artwork/../../etc/passwd")
            .body(axum::body::Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn disabled_tmdb_adds_no_artwork() {
    let state = AppState::new(service(), "test-token");
    let items = vec![/* one MediaSummary from the existing test fixture */];
    assert!(enrich_artwork(&state, items).await[0].artwork.is_empty());
}
```

- [ ] **Step 9: Run core tests, fmt and clippy.**

Run: `cargo test -p mediahaven-core && cargo fmt --all -- --check && cargo clippy -p mediahaven-core -p media-metadata --all-targets -- -D warnings`
Expected: PASS, exit 0. Command/route parity tests remain green — `/artwork/:key` is a GET added to `public_router`, so it does not change the 24 POST renderer commands.

---

## Task 3: Frontend consumption, attribution and docs

**Files:**
- Modify: `apps/desktop/src-tauri/tauri.conf.json`, `apps/desktop/src/services/adapt.ts`, `apps/desktop/src/pages/Settings/SettingsPage.tsx`, `roadmap.md`, `progress.md`
- Create: `apps/desktop/src/components/cards/TmdbAttribution.tsx`, `apps/desktop/src/services/adapt.artwork.test.ts`

**Interfaces:**
- Consumes: `ArtworkRef[]` on `MediaSummary` / `MediaDetail` (DTO `url` is a relative `/artwork/<hash>`); `apiBase` from `services/errors.ts`; `artFor(id, title)` from `data/catalog.ts`.
- Produces: `artFromDto(artwork, id, title) -> { hero, still }`; `TMDB_ATTRIBUTION_FRONTEND` constant and `TmdbAttribution` component.

- [ ] **Step 1: Write the failing frontend test** `apps/desktop/src/services/adapt.artwork.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { artFromDto } from "./adapt";

const summary = (artwork: { variant: string; url: string | null; alt: string }[]) =>
  ({ id: "m1", title: "Arrival", artwork }) as never;

describe("artFromDto", () => {
  it("uses the backend poster URL when present", () => {
    const art = artFromDto(
      summary([{ variant: "poster", url: "/artwork/abc", alt: "Arrival poster" }]).artwork,
      "m1", "Arrival",
    );
    expect(art.still).toBe("http://127.0.0.1:8080/artwork/abc");
    expect(art.hero).toBe("http://127.0.0.1:8080/artwork/abc");
  });

  it("falls back to bundled art when the backend sends no artwork", () => {
    const art = artFromDto([], "m1", "Arrival");
    expect(art.still).toMatch(/^\/artwork\//);
    expect(art.hero).toMatch(/^\/artwork\//);
  });

  it("never renders a javascript: or absolute third-party URL", () => {
    const art = artFromDto(
      summary([{ variant: "poster", url: "javascript:alert(1)", alt: "bad" }]).artwork,
      "m1", "Arrival",
    );
    expect(art.still).toMatch(/^\/artwork\//);
  });
});
```

- [ ] **Step 2: Run it to verify it fails.**

Run: `npm test -- --run src/services/adapt.artwork.test.ts` (from `apps/desktop`)
Expected: FAIL — `artFromDto` is not exported.

- [ ] **Step 3: Implement `artFromDto` in `apps/desktop/src/services/adapt.ts`.**

```ts
import { apiBase } from "./errors";
import type { ArtworkRef } from "./types";

const RELATIVE_ARTWORK = /^\/artwork\/[0-9a-f]{64}$/;

/** Backend poster URLs are relative and hashed. Anything else is ignored so a
 * hostile DTO cannot turn a poster into an external or script URL. */
export function artFromDto(artwork: ArtworkRef[] | undefined, id: string, title: string) {
  const url = artwork?.find((entry) => entry.variant === "poster")?.url;
  const bundled = artFor(id, title);
  return url && RELATIVE_ARTWORK.test(url)
    ? { hero: `${apiBase}${url}`, still: `${apiBase}${url}` }
    : bundled;
}
```

Then replace the three `art: artFor(...)` assignments in `toMedia`, `seriesToMedia` and `detailToMedia` with `art: artFromDto(summary.artwork, summary.id, summary.title)` (using the appropriate variable in each function).

- [ ] **Step 4: Add the attribution component** `apps/desktop/src/components/cards/TmdbAttribution.tsx`:

```tsx
/** Mirrors TMDB_ATTRIBUTION in crates/metadata. Rendered only when a TMDB
 * poster URL is present, so offline bundled art shows no network attribution. */
export const TMDB_ATTRIBUTION_FRONTEND =
  "This product uses the TMDB API but is not endorsed or certified by TMDB.";

export function TmdbAttribution() {
  return <span className="tmdb-attribution">{TMDB_ATTRIBUTION_FRONTEND}</span>;
}
```

Render it in `apps/desktop/src/components/hero/Hero.tsx` (below the stage) and `apps/desktop/src/pages/Details/MediaDetailsPage.tsx` (below the artwork), each gated on a TMDB poster being present (`detail?.artwork?.some((a) => a.url)`). Add a one-line `.tmdb-attribution { font-size: 0.7rem; opacity: 0.7; }` rule to the matching existing CSS module; use the design tokens for any size/color value rather than a new hard-coded colour.

- [ ] **Step 5: Widen the CSP by one origin** in `apps/desktop/src-tauri/tauri.conf.json`:

Change:
```
"img-src 'self' asset: http://asset.localhost data:"
```
to:
```
"img-src 'self' asset: http://asset.localhost data: http://127.0.0.1:8080"
```

This is the same loopback origin already in `connect-src`; no other CSP directive changes.

- [ ] **Step 6: Update the Settings privacy text** in `apps/desktop/src/pages/Settings/SettingsPage.tsx`.

Replace the sentence `Local NFO and TMDB metadata is unchanged.` with:

```
TMDB posters are off by default. If enabled, only a title, year and poster path
are sent to TMDB; images are cached locally and attributed. Local NFO and IMDb
behaviour is unchanged.
```

- [ ] **Step 7: Update `roadmap.md` and `progress.md`.**

Add a `P6-07 TMDB artwork (opt-in)` task row under Phase 6 with status `[~]` (code complete, live key call NOT RUN), acceptance criteria: opt-in gate, hashed cache, attribution, offline fallback, no path/key in DTO, and a note that ADR-0023 is Proposed pending human acceptance. Add a `progress.md` entry naming the observed commands/results from Tasks 1–3 and the single next action: operator supplies a TMDB key and sets `MEDIAHAVEN_TMDB_OPTIN=1`, then a live call is run and recorded.

- [ ] **Step 8: Run the full frontend and Rust gates.**

Run (from `apps/desktop`): `npm run typecheck && npm run lint && npm test -- --run && npm run check:tokens && npm run build && npm run check:bundle`
Expected: PASS, exit 0; the new test file is included in the test count.

Run (from repo root): `cargo fmt --all -- --check && cargo clippy -p media-metadata -p mediahaven-core --all-targets -- -D warnings && cargo test -p media-metadata -p mediahaven-core`
Expected: PASS, exit 0.

- [ ] **Step 9: Honest verification note.**

Record in `progress.md`: live TMDB search and live image download are **NOT RUN** (no key supplied, no opt-in). All verification used fakes and loopback only. Do not claim a poster was seen in the UI unless a browser check with a real key was actually performed.

---

## Sequencing

Task 1 → Task 2 → Task 3 (shared types and cache). No task passes a checkpoint, installs software, changes Docker/WSL/host settings, or commits. Execute with `subagent-driven-development`: one fresh subagent per task, two-stage review between tasks.

## Self-review (completed by planner)

- **Spec coverage:** ADR-0011 opt-in (Tasks 1–2), attribution (Task 3), size-capped cache (Task 1), sanitised paths (reused `sanitize_poster`), offline fallback (bundled art), no bundling (no scraper), secret in store (ADR-0007 env), path-free DTO (hashed relative URL), TV-readable placeholder (unchanged `artFor` fallback) — all covered.
- **Placeholder scan:** no TBD/TODO; every code step contains concrete code. The one conditional is the reqwest rustls feature name, which Step 2 resolves from the local locked crate before Step 3 writes it.
- **Type consistency:** `artwork_key`, `ArtworkCache`, `fetch_artwork_bytes`, `http_search_fetch`, `with_tmdb`, `with_artwork_dir`, `enrich_artwork`, `artFromDto`, `TMDB_ATTRIBUTION_FRONTEND` keep one spelling across tasks. DTO `url` is the relative `/artwork/<64-hex>` in both the Rust producer and the TS validator.
