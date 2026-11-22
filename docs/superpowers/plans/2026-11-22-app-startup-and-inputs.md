# App Startup And Inputs Plan

> **For agentic workers:** REQUIRED SUB-SKILL: subagent-driven-development. Steps use checkboxes.

**Goal:** Make the installed app start its own backend with the keys the user saved, and let the user enter a Prowlarr address and key in Settings.

**Plain-language summary:** Right now the app never starts the part that reads the library. It also only asks for a TMDB key. After this plan, opening the app starts everything, and Settings has three boxes: TMDB key, Prowlarr address, Prowlarr key. The app saves all three on this computer and uses them automatically.

**Architecture:** Reuse the existing `Supervisor` code in the Tauri host. Add one command `runtime_start` that builds the supervisor from environment and calls `ensure_running` + `wait_ready`. Call that command once when the UI mounts. Extend the existing secret store allowlist to accept `prowlarr_url` and `prowlarr_key`, and pass both into the compose environment next to the TMDB key.

**Tech Stack:** Tauri commands, existing DPAPI secret store, axum contract, React 19. No new dependency.

**Spec:** user instruction 2026-11-22: "completely plan out and fix the backend and wire everything cleanly... plug and play after they give their inputs like their Prowlarr URL and their api key for TMDB". ADR-0005, ADR-0007, ADR-0026.

## Global Constraints

- No secret in repo, image, DTO, log or error string. Fixed error strings only.
- Loopback binding only. No host feature install, no admin request.
- No new dependency, no new runtime, no site/indexer added by this plan (next plan owns that).
- Contract command count stays 25 in this plan (no new renderer command; `runtime_start` is a host-only Tauri command).
- `PASS`/`FAIL`/`NOT RUN` honest. No live WSL/Docker start in tests.

---

## Task 1: Host command that starts the backend

**Files:** modify `apps/desktop/src-tauri/src/supervisor.rs`, `commands/renderer.rs`, `lib.rs`; create `apps/desktop/src/services/runtime.ts` and `runtime.test.ts`; modify `apps/desktop/src/App.tsx`.

**Interfaces:**
- Produces: `#[tauri::command] pub fn runtime_start() -> Result<String, String>` returning a fixed status string `"ready"`, `"setup-required"` or `"failed"`.
- Produces: `startRuntime(): Promise<string>` in renderer, with a dev shim returning `"dev"`.

- [ ] Step 1: write failing test in `runtime.test.ts` asserting `startRuntime()` resolves to `"dev"` in dev mode and that the command name is `"runtime_start"`.

- [ ] Step 2: run `npm test -- --run src/services/runtime.test.ts` — expect failure (module missing).

- [ ] Step 3: implement `runtime.ts`:

```ts
import { call } from "./transport";

export async function startRuntime(): Promise<string> {
  if (import.meta.env.DEV) return "dev";
  return call<string>("runtime_start", {});
}
```

(`call` uses the existing `__TAURI_INTERNALS__` bridge; a host-only command has no HTTP route, so dev uses the shim.)

- [ ] Step 4: implement the host command in `renderer.rs`:

```rust
#[tauri::command]
pub fn runtime_start() -> Result<String, String> {
    let distro = std::env::var("MEDIAHAVEN_WSL_DISTRO")
        .unwrap_or_else(|_| "MEDIA_HAVEN".to_string());
    let address: std::net::SocketAddr = "127.0.0.1:8080".parse().map_err(|_| "setup-required".to_string())?;
    let supervisor = crate::supervisor::Supervisor::new(
        distro,
        std::path::PathBuf::from("infrastructure/containers/compose.yaml"),
        address,
    );
    match supervisor.ensure_running().and_then(|_| supervisor.wait_ready()) {
        Ok(()) => Ok("ready".to_string()),
        Err(crate::supervisor::SupervisorError::EngineAbsent) => Ok("setup-required".to_string()),
        Err(_) => Ok("failed".to_string()),
    }
}
```

Register it in `generate_handler!` in `lib.rs`. Add a test asserting the three outcomes map to fixed strings with no path or env value in the output.

- [ ] Step 5: call it once from `App.tsx` in a mount `useEffect` that ignores errors (the UI still shows its existing error state):

```ts
useEffect(() => { void startRuntime().catch(() => undefined); }, []);
```

- [ ] Step 6: run `npm run typecheck && npm run lint && npm test -- --run && cargo check -p mediahaven-desktop --all-targets`. Expected PASS.

---

## Task 2: Prowlarr inputs and compose environment

**Files:** modify `apps/desktop/src-tauri/src/secret.rs`, `commands/renderer.rs`, `supervisor.rs`, `infrastructure/containers/compose.yaml`, `apps/desktop/src/pages/Settings/SettingsPage.tsx`, `apps/desktop/src/services/providerSecrets.ts`, `apps/desktop/src/services/providerSecrets.test.ts`, `roadmap.md`, `progress.md`.

**Interfaces:**
- `kind` allowlist becomes `"tmdb" | "prowlarr_url" | "prowlarr_key"`.
- `build_wsl_env(token_hex: &str, tmdb: Option<&str>, prowlarr_url: Option<&str>, prowlarr_key: Option<&str>) -> Vec<(String,String)>` adds `MEDIAHAVEN_PROWLARR_URL` and `MEDIAHAVEN_PROWLARR_API_KEY` plus their `WSLENV` names.

- [ ] Step 1: failing tests: `provider_secret` accepts the two new kinds and still rejects others without echoing values; `build_wsl_env` includes both new vars only when present.

- [ ] Step 2: implement the allowlist in secret.rs with constants:
```rust
pub const PROWLARR_URL_ACCOUNT: &str = "prowlarr-url";
pub const PROWLARR_KEY_ACCOUNT: &str = "prowlarr-key";
```
and a helper `fn account_for(kind: &str) -> Option<&'static str>`. Update all three commands to use it.

- [ ] Step 3: extend `build_wsl_env` exactly as the interface says, keeping the no-extra-vars case unchanged.

- [ ] Step 4: add to compose `core.environment`:
```yaml
MEDIAHAVEN_PROWLARR_URL: "${MEDIAHAVEN_PROWLARR_URL:-}"
MEDIAHAVEN_PROWLARR_API_KEY: "${MEDIAHAVEN_PROWLARR_API_KEY:-}"
```
Add a compose assertion test following the existing pattern in `renderer.rs`.

- [ ] Step 5: Settings page: add `Prowlarr address` text input and `Prowlarr API key` password input with Save/Clear and configured status, mirroring the TMDB token field. Never render saved values; clear drafts on success.

- [ ] Step 6: extend `providerSecrets.ts` kind union to the three names; keep the dev shim keyed per kind.

- [ ] Step 7: docs: ADR-0028 for the startup wiring and extra inputs (Accepted, human instruction 2026-11-22); P6-10 entry; progress entry.

- [ ] Step 8: gates: `cargo fmt --all -- --check`, `cargo clippy -p mediahaven-desktop --all-targets -- -D warnings`, `cargo test -p mediahaven-desktop --lib`, frontend six gates.

---

## Self-review

- Every requirement has a task. No placeholders. Names `runtime_start`, `startRuntime`, `account_for`, `build_wsl_env` are consistent. No new dependency. Secrets never in source.
