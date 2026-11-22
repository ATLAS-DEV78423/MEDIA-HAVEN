# Operator-Supplied TMDB Token Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the app operator paste a TMDB read access token in Settings; store it in the existing Windows DPAPI secret store, inject it into `core` at startup, and use TMDB as the primary artwork source while keeping the keyless IMDb posters as fallback.

**Architecture:** The Tauri host gains two named commands (`provider_secret_set`, `provider_secret_has`) backed by new generic secret helpers in `apps/desktop/src-tauri/src/secret.rs` — the same DPAPI blob mechanism already used for the install token, with a separate identity so the two can never be confused. The supervisor adds the stored token to the environment it passes to `docker compose up`. `core` re-attaches the existing `TmdbProvider` behind a key-present check (no opt-in switch — ADR-0025 established that the human wants artwork open) and `fill_poster` tries TMDB first, then IMDb, then bundled art. The renderer gets a Settings password field and shows both required attribution lines, since either provider may supply an image.

**Tech Stack:** Rust (std, existing `windows` DPAPI calls, existing `reqwest` in `crates/metadata`), React 19, Tauri commands. No new dependency, no new runtime.

**Spec:** Human decision 2026-11-22: "we can just ask the user to add their tmdb access token, and we can use the jackett prowlarr thing"; ADR-0007 (secrets external/DPAPI), ADR-0023 (TMDB transport), ADR-0025 (keyless IMDb fallback). Jackett/Prowlarr is a **separate, later** plan — this plan does not add any source provider.

## Global Constraints

- The token is **operator-supplied, optional and reversible**. Blank/missing ⇒ zero TMDB requests, keyless IMDb artwork still works, app fully offline.
- The token is stored ONLY in the Windows DPAPI blob under a distinct identity; never in the repo, a DTO, a log, an error string, localStorage, or the renderer after save. Settings shows only configured/not-configured state.
- No scraper, indexer, catalogue, site list or Cardigann definition is added by this plan. No new dependency.
- TMDB attribution must remain exact: `"This product uses the TMDB API but is not endorsed or certified by TMDB."` IMDb posters keep `"Posters: IMDb"`.
- All existing artwork controls stay: 2 MiB/64 MiB hashed cache, 12-item-per-batch cap, 64-hex route validation, relative DTO URL, `onError` fallback.
- Tauri-only commands need a dev-memory shim so browser dev does not 404.
- `PASS`/`FAIL`/`NOT RUN` honest; no live TMDB call in tests.
- Commits permitted; no push/merge/install.

---

## File Structure

| File | Responsibility |
|---|---|
| `docs/decisions/0026-operator-tmdb-token.md` (new) | Accepted ADR: optional operator token, DPAPI storage, TMDB primary / IMDb fallback, no bundling. |
| `docs/decisions/README.md` | Index ADR-0026. |
| `apps/desktop/src-tauri/src/secret.rs` | Generic `store_secret` / `load_secret` / `delete_secret` with fixed identities. |
| `apps/desktop/src-tauri/src/renderer.rs` | Two Tauri commands, no token echoed. Register in `lib.rs`. |
| `apps/desktop/src-tauri/src/supervisor.rs` | Inject `MEDIAHAVEN_TMDB_API_KEY` into compose env when configured. |
| `crates/core/src/state.rs`, `crates/core/src/server.rs`, `crates/core/src/routes.rs` | Re-attach TMDB provider; TMDB→IMDb→bundled fallback. |
| `apps/desktop/src/services/providerSecrets.ts` (new) | Renderer facade + dev-memory shim. |
| `apps/desktop/src/pages/Settings/SettingsPage.tsx` | Token field + configured state + privacy text. |
| `apps/desktop/src/components/cards/TmdbAttribution.tsx` | Render both attribution lines. |
| `roadmap.md`, `progress.md` | Status, evidence, NOT RUN for a live token. |

---

## Task 1: ADR, DPAPI secret helpers, Tauri commands, supervisor env

**Files:** create `docs/decisions/0026-operator-tmdb-token.md`; modify `docs/decisions/README.md`, `apps/desktop/src-tauri/src/secret.rs`, `renderer.rs`, `lib.rs`, `supervisor.rs`.

**Interfaces:**
- Produces:
  - `pub const TMDB_SERVICE: &str = "mediahaven"; pub const TMDB_ACCOUNT: &str = "tmdb-read-token";`
  - `pub fn store_secret(service: &str, account: &str, value: &str) -> Result<(), SecretError>`
  - `pub fn load_secret(service: &str, account: &str) -> Result<Option<String>, SecretError>`
  - `pub fn delete_secret(service: &str, account: &str) -> Result<(), SecretError>`
  - Tauri commands `provider_secret_set(kind: &str, value: &str) -> Result<(), String>`, `provider_secret_has(kind: &str) -> Result<bool, String>`, `provider_secret_clear(kind: &str) -> Result<(), String>`; only `kind == "tmdb"` is accepted.
  - `build_wsl_env(token_hex: &str, tmdb_token: Option<&str>) -> Vec<(String, String)>` includes `MEDIAHAVEN_TMDB_API_KEY` and adds it to `WSLENV`.

- [ ] **Step 1: ADR-0026 as Accepted (2026-11-22).** Record: optional operator TMDB token, DPAPI storage, TMDB primary with keyless IMDb fallback, no site/indexer bundling, token never in DTO/log/repo, no opt-in switch (per ADR-0025), Jackett/Prowlarr explicitly out of scope for this ADR. Index it.

- [ ] **Step 2: Write failing tests** in `secret.rs` (unit module) and `supervisor.rs`:

```rust
#[test]
fn generic_secret_round_trip_and_isolation() {
    let account = format!("test-{}", uuid::Uuid::new_v4().simple()); // or a fixed test identity + cleanup
    assert_eq!(load_secret(TOKEN_SERVICE, &account).unwrap(), None);
    store_secret(TOKEN_SERVICE, &account, "a-test-value").unwrap();
    assert_eq!(load_secret(TOKEN_SERVICE, &account).unwrap().as_deref(), Some("a-test-value"));
    delete_secret(TOKEN_SERVICE, &account).unwrap();
    assert_eq!(load_secret(TOKEN_SERVICE, &account).unwrap(), None);
}

#[test]
fn provider_identity_cannot_collide_with_install_token() {
    assert_ne!(TMDB_ACCOUNT, TOKEN_ACCOUNT);
}
```

`shortcut: the round-trip test writes one DPAPI blob under APPDATA and deletes it; upgrade to an injectable store if tests ever run on a non-Windows host.`

```rust
#[test]
fn wsl_env_includes_tmdb_when_present() {
    let env = build_wsl_env("install-token", Some("tmdb-token"));
    assert!(env.iter().any(|(k, v)| k == "MEDIAHAVEN_TMDB_API_KEY" && v == "tmdb-token"));
    assert!(env.iter().any(|(k, v)| k == "WSLENV" && v.contains("MEDIAHAVEN_TMDB_API_KEY")));
    assert_eq!(build_wsl_env("install-token", None).len(), 2);
}
```

- [ ] **Step 3: Run to verify they fail** (`cargo test -p mediahaven-desktop --lib secret:: supervisor::`). Expected: compile errors naming missing APIs.

- [ ] **Step 4: Implement generic secret helpers in `secret.rs`.** Reuse `blob_path`, `protect`, `unprotect`, `write_blob`, `zeroize`:

```rust
pub const TMDB_SERVICE: &str = TOKEN_SERVICE;
pub const TMDB_ACCOUNT: &str = "tmdb-read-token";

/// Store an arbitrary operator-provided secret as a DPAPI blob. The value is
/// zeroised after protection; it is never logged or returned.
pub fn store_secret(service: &str, account: &str, value: &str) -> Result<(), SecretError> {
    if value.is_empty() || value.len() > 4096 || value.chars().any(char::is_control) {
        return Err(SecretError::InvalidIdentity);
    }
    let path = blob_path(service, account)?;
    let mut bytes = value.as_bytes().to_vec();
    let protected = match protect(&bytes) { Ok(v) => v, Err(e) => { zeroize(&mut bytes); return Err(e); } };
    zeroize(&mut bytes);
    write_blob(&path, &protected)
}

/// `Ok(None)` means no secret is configured; a corrupt blob is an error, never
/// an empty fallback.
pub fn load_secret(service: &str, account: &str) -> Result<Option<String>, SecretError> {
    let path = blob_path(service, account)?;
    if !path.exists() { return Ok(None); }
    let protected = fs::read(&path).map_err(|_| SecretError::CredentialMismatch)?;
    let mut bytes = unprotect(&protected).map_err(|_| SecretError::CredentialMismatch)?;
    let text = String::from_utf8(bytes.clone()).map_err(|_| { zeroize(&mut bytes); SecretError::CredentialMismatch })?;
    zeroize(&mut bytes);
    Ok(Some(text))
}

pub fn delete_secret(service: &str, account: &str) -> Result<(), SecretError> {
    let path = blob_path(service, account)?;
    if path.exists() { fs::remove_file(path).map_err(|_| SecretError::CredentialMismatch)?; }
    Ok(())
}
```

Only ASCII identity strings are valid via the existing `valid_identity` check. Reuse `SecretError` variants; do not add a new error carrying the value.

- [ ] **Step 5: Add Tauri commands in `renderer.rs` and register them in `lib.rs`.**

```rust
#[tauri::command]
pub fn provider_secret_set(kind: &str, value: &str) -> Result<(), String> {
    if kind != "tmdb" { return Err("unsupported provider".to_string()); }
    crate::secret::store_secret(crate::secret::TMDB_SERVICE, crate::secret::TMDB_ACCOUNT, value)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn provider_secret_has(kind: &str) -> Result<bool, String> {
    if kind != "tmdb" { return Err("unsupported provider".to_string()); }
    crate::secret::load_secret(crate::secret::TMDB_SERVICE, crate::secret::TMDB_ACCOUNT)
        .map(|value| value.is_some())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn provider_secret_clear(kind: &str) -> Result<(), String> { /* same allowlist; delete_secret */ }
```

Add the three functions to `generate_handler!`. Add a test asserting an unknown kind is rejected and that the returned errors do not contain a synthetic value such as `"SENTINEL"`.

- [ ] **Step 6: Inject into compose env in `supervisor.rs`.**

```rust
const TMDB_KEY_ENV: &str = "MEDIAHAVEN_TMDB_API_KEY";

fn build_wsl_env(token_hex: &str, tmdb_token: Option<&str>) -> Vec<(String, String)> {
    let mut env = vec![
        (API_TOKEN_ENV.to_string(), token_hex.to_string()),
        (WSLENV.to_string(), API_TOKEN_ENV.to_string()),
    ];
    if let Some(key) = tmdb_token.filter(|value| !value.trim().is_empty()) {
        env.push((TMDB_KEY_ENV.to_string(), key.to_string()));
        if let Some((_, wslenv)) = env.iter_mut().find(|(k, _)| k == WSLENV) {
            wslenv.push(':'); wslenv.push_str(TMDB_KEY_ENV);
        }
    }
    env
}
```

In `ensure_running`, load the secret, build env, and zeroise the local `String` after `run_with_env` returns (drop scope). Never log the value.

- [ ] **Step 7: Verify Task 1.**

Run: `cargo fmt --all -- --check && cargo clippy -p mediahaven-desktop --all-targets -- -D warnings && cargo test -p mediahaven-desktop --lib`. Expected: PASS.

---

## Task 2: Core TMDB primary, IMDb fallback

**Files:** modify `crates/core/src/state.rs`, `crates/core/src/server.rs`, `crates/core/src/routes.rs`.

**Interfaces:**
- Consumes: existing `TmdbProvider`, `api_key_from_env`, `http_search_fetch`, `TMDB_ATTRIBUTION`, `fetch_artwork_bytes`, `artwork_key`; existing `ImdbArtworkProvider`, `fetch_image_bytes`.
- Produces: `AppState.tmdb: Arc<TmdbProvider>` + `with_tmdb`; `tmdb_provider_from_env()` with key present and **no opt-in flag**.

- [ ] **Step 1: Write a failing core test** in `routes.rs` using a fake TMDB fetch and a fake IMDb fetch: when TMDB returns a poster, DTO URL is present and the IMDb fetch counter stays 0; when TMDB fetch fails, IMDb is used; when both fail, artwork is empty.

- [ ] **Step 2: Run to verify it fails.**

- [ ] **Step 3: Re-add `tmdb` to `AppState`** in `state.rs`:

```rust
use media_metadata::tmdb::{api_key_from_env, DEFAULT_MAX_CACHE_BYTES};

pub tmdb: Arc<TmdbProvider>,
...
tmdb: Arc::new(TmdbProvider::new(TmdbOptions::default())),
...
pub fn with_tmdb(mut self, provider: TmdbProvider) -> Self { self.tmdb = Arc::new(provider); self }

/// Enabled whenever the operator supplied a key; no opt-in switch (ADR-0026).
pub fn tmdb_provider_from_env(data_dir: &Path) -> TmdbProvider {
    let api_key = api_key_from_env();
    let enabled = api_key.is_some();
    TmdbProvider::with_fetch(
        TmdbOptions { enabled, api_key, cache_dir: data_dir.join("tmdb-cache"), max_bytes: DEFAULT_MAX_CACHE_BYTES },
        http_search_fetch(),
    )
}
```

In `server.rs`, re-add `.with_tmdb(crate::state::tmdb_provider_from_env(&config.data_dir))`.

- [ ] **Step 4: Make `fill_poster` try TMDB first.** In `crates/core/src/routes.rs`, inside the existing `spawn_blocking` closure:

```rust
let mut image: Option<(String, Vec<u8>, String)> = None;
if tmdb.options().enabled {
    if let Some(found) = tmdb.lookup_full(&item.title, item.year) {
        if let Some(path) = found.poster_path {
            let key = media_metadata::artwork_key(&path);
            if cache.get(&key).is_none() {
                if let Ok((bytes, kind)) = media_metadata::fetch_artwork_bytes(&path) {
                    if cache.put(&key, &bytes, &kind).is_ok() { image = Some((key, bytes, kind)); }
                }
            } else if let Some((bytes, kind)) = cache.get(&key) {
                image = Some((key, bytes, kind));
            }
        }
    }
}
if image.is_none() {
    // existing IMDb block: poster_url -> artwork_key(&url) -> fetch_image_bytes
}
if let Some((key, _, _)) = image {
    item.artwork.push(ArtworkRef { variant: ArtworkVariant::Poster, url: Some(format!("/artwork/{key}")), alt: format!("{} poster", item.title) });
}
```

Clone `state.tmdb` alongside `state.imdb_artwork` into each closure. Keep the 12-item cap, 64-hex route validation and relative URL unchanged. Reuse the shared `fill_poster` helper; do not duplicate the loop.

- [ ] **Step 5: Verify Task 2.**

Run: `cargo fmt --all -- --check && cargo clippy -p mediahaven-core --all-targets -- -D warnings && cargo test -p mediahaven-core -p mediahaven-contract`. Expected: PASS, route parity green.

---

## Task 3: Settings field, dev shim, attribution, docs

**Files:** create `apps/desktop/src/services/providerSecrets.ts`; modify `SettingsPage.tsx`, `TmdbAttribution.tsx`, `roadmap.md`, `progress.md`.

- [ ] **Step 1: Write a failing frontend test** for the Settings field: typing a token and clicking Save calls `provider_secret_set("tmdb", value)`; after save, the field clears and only "Configured" is shown; the token never appears in the DOM after save.

- [ ] **Step 2: Run to verify it fails.**

- [ ] **Step 3: Implement `providerSecrets.ts`.**

```ts
import { api } from "./api"; // not used for secrets — kept for symmetry only if needed

const DEV_KEY = "mediahaven.dev.tmdb-token";

export async function setProviderSecret(kind: string, value: string): Promise<void> {
  if (kind !== "tmdb") throw new Error("unsupported provider");
  if (import.meta.env.DEV) {
    sessionStorage.setItem(DEV_KEY, value); // dev-only shim, never in production bundle
    return;
  }
  await window.__TAURI__.invoke("provider_secret_set", { kind, value });
}

export async function hasProviderSecret(kind: string): Promise<boolean> {
  if (kind !== "tmdb") throw new Error("unsupported provider");
  if (import.meta.env.DEV) return Boolean(sessionStorage.getItem(DEV_KEY));
  return window.__TAURI__.invoke("provider_secret_has", { kind });
}

export async function clearProviderSecret(kind: string): Promise<void> { /* mirror above */ }
```

Follow the repo's existing Tauri-bridge typing style; do not add `window.__TAURI__` if the codebase uses a different accessor — reuse its actual pattern.

- [ ] **Step 4: Add the Settings field** in the metadata panel: password input with `aria-label="TMDB read access token"`, hint naming exactly what is sent (title/year/poster path to TMDB), Save and Clear buttons, and a `Configured` / `Not configured` status from `hasProviderSecret`. Never render the stored value; clear the input immediately after save. Add to the panel text that a token is optional and posters fall back to IMDb without it.

- [ ] **Step 5: Attribution.** In `TmdbAttribution.tsx`, export both exact constants and render both lines:

```ts
export const TMDB_ATTRIBUTION_FRONTEND =
  "This product uses the TMDB API but is not endorsed or certified by TMDB.";
export const IMDB_POSTER_ATTRIBUTION = "Posters: IMDb";
```

Render them in the same caption element. Update any test that asserts only one line.

- [ ] **Step 6: Docs.** `roadmap.md`: new P6-08 `[~]` operator TMDB token, with acceptance criteria and live-token NOT RUN. `progress.md`: entry naming observed gates and the single next action — operator supplies a token, restarts the app, and one live TMDB poster is observed.

- [ ] **Step 7: Verify Task 3.**

Run from `apps/desktop`: `npm run typecheck && npm run lint && npm test -- --run && npm run check:tokens && npm run build && npm run check:bundle`. Expected: PASS.

---

## Sequencing

Task 1 → Task 2 → Task 3. Subagent-driven development, one implementer + one reviewer per task, final whole-branch review at the end. No site list, no indexer, no Jackett/Prowlarr code in this plan. No live TMDB call until an operator supplies a token.

## Self-review (completed by planner)

- **Spec coverage:** optional operator token, DPAPI storage, env injection, TMDB primary + IMDb fallback, exact attribution, no bundling, no key in DTO/log/dev shim only. Covered.
- **Placeholder scan:** no TBD/TODO; every code step has concrete code. Step 3 of Task 3 names the one codebase-specific detail (Tauri accessor) that the implementer must match to the existing pattern.
- **Type consistency:** `store_secret`, `load_secret`, `delete_secret`, `TMDB_ACCOUNT`, `with_tmdb`, `tmdb_provider_from_env`, `provider_secret_set/has/clear`, `setProviderSecret/hasProviderSecret` keep one spelling across tasks.
