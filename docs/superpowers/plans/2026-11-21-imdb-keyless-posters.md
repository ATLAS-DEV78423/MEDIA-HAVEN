# Keyless IMDb Posters Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the unusable TMDB artwork path with keyless IMDb poster lookup, so real posters appear in the UI without any API key.

**Architecture:** A new `ImdbArtworkProvider` in `crates/metadata` queries IMDb's keyless public suggestion JSON (`https://v2.sg.media-imdb.com/suggestion/<letter>/<query>.json`), picks a real title entry (`id` starts with `tt`, title/year match), and returns its `imageUrl` only if it passes a strict `https://m.media-amazon.com/images/M/` allowlist. The existing `ArtworkCache`, `classify_image`, `MAX_ARTWORK_BYTES`, `artwork_key`, public `GET /artwork/:key` route, 64-hex DTO validation, 12-item-per-batch cap and frontend `artFromDto`/`onError` fallback are all reused unchanged. The TMDB provider stays in the crate but is no longer wired into `core`; no code is deleted. Attribution changes to "Posters: IMDb".

**Tech Stack:** Rust with the already-present `reqwest` 0.13.5 (rustls + blocking + query) and `serde_json`. No new dependency, no new runtime (no Python/Cinemagoer process), no new npm package.

**Spec:** Human choices 2026-11-21: "i cant use the tmdb thing" / "No API key" / "Try Cinemagoer/IMDb instead". ADR-0011 (TMDB primary) and ADR-0023 (TMDB transport) are superseded **for artwork only**; ADR-0007 (secrets) stays for the API token; `Development.md` §4.3/§7.4.12; `AGENTS.md` (no dependency without ADR, no runtime, no bundled content source, tokens only).

## Global Constraints

- **No API key and no Python runtime.** The IMDb suggestion endpoint is keyless; all work stays in the existing Rust core using the already-locked `reqwest`.
- **No runtime HTML scraping.** Use the JSON suggestion endpoint only. Accept and record in the ADR that this endpoint is unofficial and may change (ToS/availability risk), with bundled art as the fallback.
- Image URLs must be `https://m.media-amazon.com/images/M/…`, ≤512 chars, no `..`, no control characters. Any other host/scheme is rejected before a request is made.
- Reuse every existing artwork control: 2 MiB per image, 64 MiB total cache, oldest-first eviction, 12-item-per-request cap, 64-lowercase-hex `:key` validation before filesystem join, relative `/artwork/<hash>` DTO URL, `onError` fallback to bundled art.
- Failure anywhere (no match, bad URL, timeout, non-image bytes, full cache) degrades to bundled art. Never break a view, never claim a poster that was not cached.
- No path, token or host detail in renderer DTOs; no secret in source. `PASS`/`FAIL`/`NOT RUN` honest.
- Commits permitted on this worktree; no push/merge/install.

---

## File Structure

| File | Responsibility |
|---|---|
| `docs/decisions/0025-imdb-keyless-posters.md` (new) | Accepted ADR: IMDb suggestion endpoint, keyless, unofficial-endpoint risk, supersedes artwork parts of ADR-0011/0023. |
| `docs/decisions/README.md` | Index ADR-0025. |
| `crates/metadata/src/imdb_artwork.rs` (new) | Provider, injectable fetch seam, suggestion JSON parsing, URL allowlist, image download. |
| `crates/metadata/src/lib.rs` | Re-export the new API. |
| `crates/metadata/tests/imdb_artwork.rs` (new) | Fake-fetch tests: title/year match, `tt` filter, URL allowlist, image cap/type. |
| `crates/core/src/state.rs`, `crates/core/src/server.rs` | Wire the IMDb artwork provider instead of TMDB. |
| `crates/core/src/routes.rs` | `fill_poster` uses IMDb URL + `fetch_image_bytes`; update the enrichment test. |
| `apps/desktop/src/components/cards/TmdbAttribution.tsx` | Attribution text → "Posters: IMDb". |
| `apps/desktop/src/pages/Settings/SettingsPage.tsx` | Honest privacy/setup text (no TMDB, no key). |
| `roadmap.md`, `progress.md` | Status, evidence, NOT RUN for a full library seed. |

---

## Task 1: ADR + keyless IMDb artwork provider

**Files:** create `docs/decisions/0025-imdb-keyless-posters.md`, `crates/metadata/src/imdb_artwork.rs`, `crates/metadata/tests/imdb_artwork.rs`; modify `crates/metadata/src/lib.rs`, `docs/decisions/README.md`.

**Interfaces:**
- Consumes: `classify_image`, `MAX_ARTWORK_BYTES` from `tmdb.rs` (both already `pub`); `reqwest`, `serde_json` already deps.
- Produces:
  - `pub type ArtworkFetchFn = dyn Fn(&str, Option<i64>) -> Result<String, String> + Send + Sync`
  - `pub struct ImdbArtworkProvider` with `new()`, `with_fetch(Box<ArtworkFetchFn>)`, `poster_url(&self, &str, Option<i64>) -> Option<String>`
  - `pub fn imdb_search_fetch(title: &str, year: Option<i64>) -> Result<String, String>` (URL only; errors are already URL-free constants)
  - `pub fn validate_image_url(url: &str) -> bool`
  - `pub fn fetch_image_bytes(url: &str) -> Result<(Vec<u8>, String), String>`

- [ ] **Step 1: Write ADR-0025 as Accepted (2026-11-21).**

```markdown
# ADR-0025: Keyless IMDb posters (supersedes artwork opt-in)

- **Status:** Accepted (2026-11-21) — human: no TMDB key available; use IMDb.
- **Supersedes:** the artwork clauses of ADR-0011 and ADR-0023 only. IMDb ratings/OMDb remain opt-in; local NFO stays the offline metadata fallback.
- **Decision:** Poster URLs come from IMDb's keyless public suggestion JSON, parsed in the Rust core with the already-present reqwest client. No API key, no Python runtime, no HTML scraping. Image bytes must come from `m.media-amazon.com/images/M/`; they are cached with the existing 2 MiB/64 MiB hashed cache. Attribution: "Posters: IMDb".
- **Risk accepted:** the suggestion endpoint is unofficial and can change or be rate-limited; failures degrade to bundled art. The human chose this over a keyed provider.
- **Preserved:** no path/key in DTOs, loopback-only serving, 64-hex key validation, 12-item cap, onError fallback.
```

Index it in `docs/decisions/README.md`.

- [ ] **Step 2: Write failing tests** in `crates/metadata/tests/imdb_artwork.rs` (no network):

```rust
use media_metadata::imdb_artwork::{validate_image_url, ImdbArtworkProvider};

const FAKE_URL: &str = "https://m.media-amazon.com/images/M/MV5Btest._V1_.jpg";

#[test]
fn picks_a_real_title_with_matching_year() {
    // fake fetch returns a JSON body with one non-title entry, one wrong-year
    // title, and one matching title with an imageUrl
    let provider = ImdbArtworkProvider::with_fetch(Box::new(|_title, _year| Ok(SAMPLE.to_string())));
    assert_eq!(provider.poster_url("Arrival", Some(2016)).as_deref(), Some(FAKE_URL));
}

#[test]
fn rejects_non_imdb_image_hosts() {
    assert!(validate_image_url(FAKE_URL));
    assert!(!validate_image_url("https://evil.example/a.jpg"));
    assert!(!validate_image_url("http://m.media-amazon.com/images/M/a.jpg"));
    assert!(!validate_image_url("https://m.media-amazon.com/../etc/x.jpg"));
    assert!(!validate_image_url("javascript:alert(1)"));
}

#[test]
fn provider_fails_closed_on_fetch_error_or_bad_url() {
    let bad = ImdbArtworkProvider::with_fetch(Box::new(|_, _| Ok("https://evil.example/x".into())));
    assert_eq!(bad.poster_url("x", None), None);
    let err = ImdbArtworkProvider::with_fetch(Box::new(|_, _| Err("boom".into())));
    assert_eq!(err.poster_url("x", None), None);
}
```

- [ ] **Step 3: Run to verify they fail.**

Run: `cargo test -p media-metadata --test imdb_artwork`
Expected: compile error — module/API missing.

- [ ] **Step 4: Implement `crates/metadata/src/imdb_artwork.rs`.**

```rust
//! Keyless IMDb poster lookup. Only a title and year are sent to IMDb's public
//! suggestion endpoint; no library path or content is included.

use serde_json::Value;
use std::time::Duration;

use crate::tmdb::{classify_image, MAX_ARTWORK_BYTES};

pub type ArtworkFetchFn = dyn Fn(&str, Option<i64>) -> Result<String, String> + Send + Sync;

const IMAGE_PREFIX: &str = "https://m.media-amazon.com/images/M/";

pub fn validate_image_url(url: &str) -> bool {
    url.starts_with(IMAGE_PREFIX)
        && url.len() <= 512
        && !url.contains("..")
        && !url.chars().any(char::is_control)
}

pub struct ImdbArtworkProvider {
    fetch: Box<ArtworkFetchFn>,
}

impl ImdbArtworkProvider {
    pub fn new() -> Self {
        Self::with_fetch(Box::new(imdb_search_fetch))
    }

    pub fn with_fetch(fetch: Box<ArtworkFetchFn>) -> Self {
        Self { fetch }
    }

    pub fn poster_url(&self, title: &str, year: Option<i64>) -> Option<String> {
        let url = (self.fetch)(title, year).ok()?;
        validate_image_url(&url).then_some(url)
    }
}

fn percent_encode(raw: &str) -> String {
    raw.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (byte as char).to_string(),
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

/// Fetch the KEYLESS suggestion JSON and return only the image URL string.
/// All errors are fixed, URL-free strings so no key or identifier can leak.
pub fn imdb_search_fetch(title: &str, year: Option<i64>) -> Result<String, String> {
    let clean = title.trim();
    if clean.is_empty() { return Err("empty title".to_string()); }
    let letter = clean.chars().next().unwrap_or('a').to_ascii_lowercase();
    let url = format!(
        "https://v2.sg.media-imdb.com/suggestion/{letter}/{}.json",
        percent_encode(clean)
    );
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|_| "client unavailable".to_string())?;
    let body = client
        .get(url)
        .send()
        .map_err(|_| "IMDb request failed".to_string())?
        .error_for_status()
        .map_err(|_| "IMDb request failed".to_string())?
        .text()
        .map_err(|_| "IMDb response unreadable".to_string())?;
    let value: Value = serde_json::from_str(&body).map_err(|_| "invalid IMDb response".to_string())?;
    let list = value.get("d").and_then(Value::as_array).ok_or("no IMDb results")?;
    let wanted = clean.to_ascii_lowercase();
    let is_title = |hit: &Value| {
        hit.get("id").and_then(Value::as_str).map(|id| id.starts_with("tt")).unwrap_or(false)
    };
    let year_ok = |hit: &Value| {
        year.map(|y| hit.get("y").and_then(Value::as_i64) == Some(y)).unwrap_or(true)
    };
    let hit = list
        .iter()
        .find(|hit| {
            is_title(hit)
                && year_ok(hit)
                && hit.get("l").and_then(Value::as_str).map(|l| l.to_ascii_lowercase() == wanted).unwrap_or(false)
        })
        .or_else(|| list.iter().find(|hit| is_title(hit) && year_ok(hit)))
        .ok_or("no IMDb title match")?;
    hit.get("i")
        .and_then(|image| image.get("imageUrl"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "IMDb title has no image".to_string())
}

/// Download only an allowlisted IMDb poster, bounded and type-checked.
pub fn fetch_image_bytes(url: &str) -> Result<(Vec<u8>, String), String> {
    if !validate_image_url(url) { return Err("image URL refused".to_string()); }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|_| "client unavailable".to_string())?;
    let bytes = client
        .get(url)
        .send()
        .map_err(|_| "image request failed".to_string())?
        .error_for_status()
        .map_err(|_| "image request failed".to_string())?
        .bytes()
        .map_err(|_| "image response unreadable".to_string())?;
    if bytes.len() > MAX_ARTWORK_BYTES { return Err("poster exceeds size cap".to_string()); }
    let kind = classify_image(&bytes).ok_or_else(|| "poster is not an image".to_string())?;
    Ok((bytes.to_vec(), kind.to_string()))
}
```

`shortcut: the IMDb suggestion endpoint is unofficial; a change in its JSON shape degrades to bundled art rather than breaking the view — upgrade only when an official keyless feed exists.`

- [ ] **Step 5: Re-export from `crates/metadata/src/lib.rs`.**

```rust
pub use imdb_artwork::{
    fetch_image_bytes, imdb_search_fetch, validate_image_url, ArtworkFetchFn, ImdbArtworkProvider,
};
```

- [ ] **Step 6: Verify Task 1.**

Run: `cargo test -p media-metadata --test imdb_artwork && cargo fmt --all -- --check && cargo clippy -p media-metadata --all-targets -- -D warnings`
Expected: PASS; no network used in tests.

---

## Task 2: Wire IMDb into core, update attribution and docs

**Files:** modify `crates/core/src/state.rs`, `crates/core/src/server.rs`, `crates/core/src/routes.rs`, `apps/desktop/src/components/cards/TmdbAttribution.tsx`, `apps/desktop/src/pages/Settings/SettingsPage.tsx`, `roadmap.md`, `progress.md`.

**Interfaces:**
- Consumes: `ImdbArtworkProvider`, `fetch_image_bytes` from Task 1; existing `ArtworkCache`, `artwork_key`, `DEFAULT_MAX_CACHE_BYTES`, `MAX_ENRICH_PER_REQUEST`, `fill_poster`.
- Produces: `AppState::imdb_artwork: Arc<ImdbArtworkProvider>` and `AppState::with_imdb_artwork(...)` test seam.

- [ ] **Step 1: Write a failing state test** in `crates/core/src/state.rs`:

```rust
#[test]
fn imdb_artwork_provider_is_available_without_a_key() {
    let state = AppState::new(service_for_test_blocking(), "test-token");
    assert!(state.imdb_artwork.poster_url("", None).is_none()); // empty title is refused
}
```

(`service_for_test_blocking()` is the existing test helper adapted for a non-async test; reuse the file's current helper.)

- [ ] **Step 2: Run to verify it fails** (`cargo test -p mediahaven-core state::tests::imdb_artwork_provider_is_available_without_a_key`).

Expected: compile error — no field `imdb_artwork`.

- [ ] **Step 3: Replace the TMDB field in `AppState`.**

Remove `TMDB_OPTIN_ENV` and the `tmdb` field / `with_tmdb` builder. Add:

```rust
use media_metadata::{fetch_image_bytes, ImdbArtworkProvider};

pub imdb_artwork: Arc<ImdbArtworkProvider>,
...
imdb_artwork: Arc::new(ImdbArtworkProvider::new()),
...
pub fn with_imdb_artwork(mut self, provider: ImdbArtworkProvider) -> Self {
    self.imdb_artwork = Arc::new(provider);
    self
}
```

Delete `tmdb_provider_from_env` (no key, no opt-in). In `server.rs`, remove `.with_tmdb(...)`, keep `.with_imdb(...)` and `.with_artwork_dir(...)`.

- [ ] **Step 4: Change `fill_poster` in `crates/core/src/routes.rs`.**

Replace the TMDB lookup and image fetch with:

```rust
let Some(url) = imdb_artwork.poster_url(&item.title, item.year) else { return };
let key = media_metadata::artwork_key(&url);
if cache.get(&key).is_none() {
    let Ok((bytes, kind)) = fetch_image_bytes(&url) else { return };
    if cache.put(&key, &bytes, &kind).is_err() { return; }
}
```

Clone `state.imdb_artwork` into each `spawn_blocking` closure alongside `artwork_dir`. Keep the 12-item cap, the pre-join 64-hex route validation, the relative `/artwork/{key}` DTO URL and all existing tests. Rewrite the enrichment test `enabled_tmdb_adds_only_a_relative_hashed_url` into `imdb_artwork_adds_only_a_relative_hashed_url` using `with_imdb_artwork(ImdbArtworkProvider::with_fetch(Box::new(|_, _| Ok(FAKE_URL.into()))))` and a pre-seeded JPEG byte file so no network is used; keep its key-free DTO assertion.

- [ ] **Step 5: Update attribution and settings text.**

In `apps/desktop/src/components/cards/TmdbAttribution.tsx`, change the constant value to:

```ts
export const TMDB_ATTRIBUTION_FRONTEND = "Posters: IMDb";
```

Keep the component name and import sites unchanged (renaming is not needed). In `SettingsPage.tsx`, replace the TMDB sentence with: `Posters come from IMDb when available; no API key is used. Only a title and year are looked up, and images are cached locally.` Keep the surrounding IMDb/OMDb opt-in text intact.

- [ ] **Step 6: Update `roadmap.md` and `progress.md`.**

P6-07 becomes `[~] P6-07 - Keyless IMDb posters`, noting ADR-0025 supersedes the artwork parts of ADR-0011/0023, no key, no Python, unofficial endpoint risk, and that a full-library live seed is **NOT RUN**. Add a `progress.md` entry naming the verified commands and the single next action: run one real lookup against a real library title and confirm the cached poster renders.

- [ ] **Step 7: Verify Task 2.**

Run: `cargo fmt --all -- --check && cargo clippy -p mediahaven-core -p media-metadata --all-targets -- -D warnings && cargo test -p mediahaven-core -p media-metadata`, then from `apps/desktop`: `npm run typecheck && npm run lint && npm test -- --run && npm run check:tokens && npm run build && npm run check:bundle`.
Expected: all PASS; route parity green; no new dependency.

---

## Sequencing

Task 1 → Task 2. Execute with subagent-driven-development, one implementer and one reviewer per task. No live IMDb seed, no push/merge, no checkpoint passed.

## Self-review (completed by planner)

- **Spec coverage:** no key (keyless endpoint), no Python, no runtime HTML scraping, movies+TV, allowlisted images, cached locally, attribution, bundled fallback — all covered.
- **Placeholder scan:** no TBD/TODO; every code step contains concrete code.
- **Type consistency:** `ImdbArtworkProvider`, `poster_url`, `fetch_image_bytes`, `validate_image_url`, `with_imdb_artwork` keep one spelling; DTO URL shape and 64-hex validation are unchanged from the reviewed artwork code.
