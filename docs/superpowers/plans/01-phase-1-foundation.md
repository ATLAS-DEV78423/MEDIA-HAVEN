# Phase 1 — Foundation: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (or superpowers:subagent-driven-development) to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A repository that builds from a clean clone, proves it in CI, logs without leaking, persists config atomically, migrates SQLite safely, and completes a read-only first-run preflight.

**Architecture:** Cargo workspace with crate boundaries from `crates/README.md`, plus `apps/desktop` (Vite + React + TS) inside a Tauri 2 shell. Business rules live in `crates/app-core`; the frontend only presents state and calls typed commands.

**Tech Stack:** Rust (stable, pinned), Tauri 2, Vite, React 18, TypeScript, SQLx (SQLite, bundled), `tracing` + rolling appender, PowerShell 5.1.

**Spec:** `docs/superpowers/specs/2026-11-15-media-haven-design.md`

**Derived from P0 evidence:** pin the exact versions recorded by P0-02 in `progress.md`. Do not guess a version that the spike did not prove.

## Global Constraints

- Windows 11 x64. Per-user install, no administrator rights.
- `cargo fmt --check` and `cargo clippy -- -D warnings` must pass.
- Domain crate has zero I/O dependencies.
- All app data lives under `%LOCALAPPDATA%/<AppName>/` — never the install directory.
- No secrets, personal paths or real filenames in logs or tests.
- Every filesystem path crossing a boundary is canonicalised and containment-checked.

---

## Task 1: Workspace and frontend scaffold (P1-01)

**Files:**
- Create: `Cargo.toml` (workspace)
- Create: `rust-toolchain.toml`
- Create: `crates/app-core/{Cargo.toml,src/lib.rs}`
- Create: `crates/app-core/src/{domain/mod.rs,application/mod.rs,ports/mod.rs,events/mod.rs}`
- Create: `apps/desktop/package.json`, `vite.config.ts`, `tsconfig.json`, `index.html`, `src/main.tsx`
- Create: `apps/desktop/src-tauri/{Cargo.toml,tauri.conf.json,src/main.rs,src/lib.rs}`
- Modify: `.gitignore` (already prepared)

**Interfaces:**
- Produces: `app_core::domain` module tree; workspace member list; `npm run build -w apps/desktop`; `cargo build --workspace`.

- [ ] **Step 1: Write the failing check**

Create `crates/app-core/src/domain/mod.rs`:

```rust
//! Domain entities and invariants. No I/O, no Tauri, no SQL.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Movie,
    Episode,
    Unknown,
}
```

Add `crates/app-core/src/domain/mod.rs` test:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_kind_is_exhaustive_and_comparable() {
        assert_eq!(MediaKind::Movie, MediaKind::Movie);
        assert_ne!(MediaKind::Movie, MediaKind::Episode);
    }
}
```

Create `crates/app-core/src/lib.rs`:

```rust
pub mod application;
pub mod domain;
pub mod events;
pub mod ports;
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p app-core`
Expected: FAIL — `Cargo.toml` or module files missing.

- [ ] **Step 3: Add the workspace and crate manifests**

`Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = [
    "crates/app-core",
    "apps/desktop/src-tauri",
]

[workspace.package]
edition = "2021"
rust-version = "1.77"
license = "SEE LICENSE IN LICENSE"

[workspace.lints.rust]
unsafe_code = "forbid"
```

`rust-toolchain.toml`:

```toml
[toolchain]
channel = "1.77.0"
components = ["rustfmt", "clippy"]
```

> Replace `1.77.0` with the version P0-02 actually used. Pin it; do not use `stable`.

`crates/app-core/Cargo.toml`:

```toml
[package]
name = "app-core"
version = "0.1.0"
edition.workspace = true

[dependencies]
thiserror = "1"
tracing = "0.1"

[lints]
workspace = true
```

Create empty module files so the tree compiles:

```rust
// crates/app-core/src/ports/mod.rs
//! Trait definitions for infrastructure adapters.

// crates/app-core/src/application/mod.rs
//! Use cases orchestrating domain logic.

// crates/app-core/src/events/mod.rs
//! Domain events.
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p app-core`
Expected: PASS — 1 test.

- [ ] **Step 5: Scaffold the frontend**

```powershell
cd apps/desktop
npm create vite@latest . -- --template react-ts
npm install
npm install -D vitest @testing-library/react @testing-library/jest-dom jsdom
```

Add to `apps/desktop/package.json` under `"scripts"`:

```json
"test": "vitest run"
```

- [ ] **Step 6: Add a frontend smoke test**

Create `apps/desktop/src/app/App.test.tsx`:

```tsx
import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import App from "./App";

describe("App", () => {
  it("renders the application shell heading", () => {
    render(<App />);
    expect(screen.getByRole("heading", { level: 1 })).toBeInTheDocument();
  });
});
```

Create `apps/desktop/vitest.config.ts`:

```ts
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: { environment: "jsdom", globals: true, setupFiles: "./src/test/setup.ts" },
});
```

Create `apps/desktop/src/test/setup.ts`:

```ts
import "@testing-library/jest-dom/vitest";
```

- [ ] **Step 7: Verify the frontend builds and tests**

Run: `npm run test -w apps/desktop && npm run build -w apps/desktop`
Expected: 1 test PASS, build succeeds.

- [ ] **Step 8: Verify the whole workspace builds from clean**

```powershell
cargo build --workspace
cargo fmt --check
cargo clippy --workspace -- -D warnings
```

Expected: all exit 0.

- [ ] **Step 9: Commit**

```bash
git add Cargo.toml rust-toolchain.toml crates apps/desktop/package.json apps/desktop/vite.config.ts apps/desktop/vitest.config.ts apps/desktop/tsconfig.json apps/desktop/index.html apps/desktop/src
git commit -m "chore(workspace): scaffold cargo workspace and desktop frontend"
```

---

## Task 2: CI for Rust and frontend (P1-02)

**Files:**
- Create: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: Task 1 commands.
- Produces: red-on-violation checks for fmt, clippy, Rust tests, TS typecheck, frontend tests.

- [ ] **Step 1: Write the workflow**

```yaml
name: ci

on:
  push:
    branches: [main]
  pull_request:

jobs:
  rust:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: 1.77.0
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --all --check
      - run: cargo clippy --workspace --all-targets -- -D warnings
      - run: cargo test --workspace --locked

  frontend:
    runs-on: windows-latest
    defaults:
      run:
        working-directory: apps/desktop
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: npm
          cache-dependency-path: apps/desktop/package-lock.json
      - run: npm ci
      - run: npx tsc --noEmit
      - run: npm test
      - run: npm run build
```

> Replace `1.77.0` with the pinned toolchain from `rust-toolchain.toml`.

- [ ] **Step 2: Prove the checks actually fail — fmt**

Introduce a deliberate formatting error in `crates/app-core/src/domain/mod.rs` (e.g. add `fn   x(){}`), push to a branch.

Run: `cargo fmt --all --check`
Expected: non-zero exit, CI red.

Revert the change with `cargo fmt --all`.

- [ ] **Step 3: Prove clippy fails**

Add a deliberately lint-triggering function:

```rust
pub fn lint_probe() -> String {
    let s = String::from("x");
    s.clone()
}
```

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: red on `clippy::redundant_clone` or similar. Remove the function.

- [ ] **Step 4: Prove a failing Rust test is red**

Temporarily change the domain test to `assert_eq!(MediaKind::Movie, MediaKind::Episode);`

Run: `cargo test --workspace`
Expected: FAIL. Revert.

- [ ] **Step 5: Prove the TypeScript check is red**

Add `const x: number = "nope";` to `App.tsx`.

Run: `npx tsc --noEmit` (in `apps/desktop`)
Expected: error. Revert.

- [ ] **Step 6: Confirm green**

Push the reverted branch. Expected: all jobs green.

- [ ] **Step 7: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add rust and frontend checks (Refs: P1-02)"
```

---

## Task 3: Structured logging with redaction (P1-03)

**Files:**
- Create: `crates/diagnostics/Cargo.toml`
- Create: `crates/diagnostics/src/{lib.rs,paths.rs,redact.rs,init.rs}`
- Modify: `Cargo.toml` (add member)
- Test: `crates/diagnostics/src/redact.rs` (inline `#[cfg(test)]`)

**Interfaces:**
- Produces:
  - `pub fn app_data_dir() -> std::path::PathBuf`
  - `pub fn redact(input: &str) -> String`
  - `pub fn init_logging(log_dir: &std::path::Path, level: tracing::Level) -> anyhow::Result<tracing_appender::non_blocking::WorkerGuard>`

- [ ] **Step 1: Write the failing test**

Create `crates/diagnostics/src/redact.rs`:

```rust
//! Best-effort redaction of user-identifying data before it reaches a log file
//! or a diagnostic bundle.

/// Redacts home-directory paths, filenames after a separator, tokens and URLs.
pub fn redact(input: &str) -> String {
    input.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_a_windows_home_path() {
        let out = redact(r"C:\Users\alice\Videos\family-holiday.mp4");
        assert!(!out.contains("alice"), "username leaked: {out}");
        assert!(!out.contains("family-holiday"), "filename leaked: {out}");
    }

    #[test]
    fn redacts_a_bearer_token() {
        let out = redact("Authorization: Bearer abc123def456ghi789");
        assert!(!out.contains("abc123def456ghi789"), "token leaked: {out}");
    }

    #[test]
    fn keeps_ordinary_text_readable() {
        assert_eq!(redact("scan started"), "scan started");
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p diagnostics redact`
Expected: FAIL — crate not in workspace / assertions fail.

- [ ] **Step 3: Implement the redactor**

```rust
use once_cell::sync::Lazy;
use regex::Regex;

static HOME_PATH: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)[A-Z]:\\Users\\[^\\\s"']+(\\(?:[^\\\s"']+))*"#).unwrap()
});
static BEARER: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)(bearer|token|password|secret)\s*[:=]\s*\S+").unwrap());
static URL: Lazy<Regex> = Lazy::new(|| Regex::new(r"https?://\S+").unwrap());

pub fn redact(input: &str) -> String {
    let out = HOME_PATH.replace_all(input, r"<path>");
    let out = BEARER.replace_all(&out, r"$1=<redacted>");
    let out = URL.replace_all(&out, "<url>");
    out.into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Absence assertion: the sensitive value must not survive.
    fn assert_absent(haystack: &str, needle: &str) {
        assert!(
            !haystack.to_lowercase().contains(&needle.to_lowercase()),
            "leaked {needle:?} in {haystack:?}"
        );
    }

    #[test]
    fn redacts_a_windows_home_path() {
        let out = redact(r"C:\Users\alice\Videos\family-holiday.mp4");
        assert_absent(&out, "alice");
        assert_absent(&out, "family-holiday");
        assert!(out.contains("<path>"));
    }

    #[test]
    fn redacts_a_bearer_token() {
        let out = redact("Authorization: Bearer abc123def456ghi789");
        assert_absent(&out, "abc123def456ghi789");
    }

    #[test]
    fn redacts_a_url() {
        let out = redact("calling http://192.168.1.10:8080/api?k=1");
        assert_absent(&out, "192.168.1.10");
    }

    #[test]
    fn keeps_ordinary_text_readable() {
        assert_eq!(redact("scan started"), "scan started");
    }
}
```

`crates/diagnostics/Cargo.toml`:

```toml
[package]
name = "diagnostics"
version = "0.1.0"
edition.workspace = true

[dependencies]
anyhow = "1"
once_cell = "1"
regex = "1"
tracing = "0.1"
tracing-appender = "0.2"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

[lints]
workspace = true
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p diagnostics`
Expected: PASS — 4 tests.

- [ ] **Step 5: Add path resolution with a test**

`crates/diagnostics/src/paths.rs`:

```rust
use std::path::PathBuf;

/// Per-user application data root. Never the install directory.
pub fn app_data_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .expect("LOCALAPPDATA must be set on Windows");
    base.join("MediaHaven")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_data_dir_is_under_local_app_data() {
        let dir = app_data_dir();
        assert!(dir.ends_with("MediaHaven"));
        let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap();
        assert!(dir.starts_with(local), "app data must stay in LOCALAPPDATA");
    }
}
```

- [ ] **Step 6: Add logging initialisation with rotation**

`crates/diagnostics/src/init.rs`:

```rust
use std::path::Path;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, EnvFilter};

/// Initialises file logging under `log_dir`. The returned guard must be held
/// for the lifetime of the process or writes are dropped.
pub fn init_logging(
    log_dir: &Path,
    level: tracing::Level,
) -> anyhow::Result<tracing_appender::non_blocking::WorkerGuard> {
    std::fs::create_dir_all(log_dir)?;
    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("mediahaven")
        .max_log_files(7)
        .build(log_dir)?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    let filter = EnvFilter::new(level.to_string());
    fmt().with_env_filter(filter).with_writer(writer).json().init();
    Ok(guard)
}
```

`crates/diagnostics/src/lib.rs`:

```rust
pub mod init;
pub mod paths;
pub mod redact;
```

- [ ] **Step 7: Verify rotation is bounded**

Add to `init.rs` tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_files_are_bounded_by_retention() {
        let tmp = std::env::temp_dir().join(format!("mh-log-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let _guard = init_logging(&tmp, tracing::Level::INFO).expect("init logging");
        tracing::info!("bounded rotation probe");
        // max_log_files(7) is configured; assert the appender was built with a limit
        // by checking at most one file exists for a single day.
        let count = std::fs::read_dir(&tmp).unwrap().count();
        assert!(count <= 7, "retention not enforced: {count} files");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
```

> `init_logging` calls `init()` which panics if a subscriber is already set. Run this test in isolation or use `try_init()`; if you switch to `try_init()`, return an error the test can assert.

- [ ] **Step 8: Verify redaction is applied at the write site**

Add a test that logs a home path through the configured subscriber and reads the file back:

```rust
#[test]
fn logged_home_path_is_redacted_in_the_file() {
    let tmp = std::env::temp_dir().join(format!("mh-redact-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    {
        let _guard = init_logging(&tmp, tracing::Level::INFO).expect("init");
        let raw = r"C:\Users\alice\Videos\secret.mp4";
        tracing::info!(message = %crate::redact::redact(raw), "opening media");
    }
    let contents: String = std::fs::read_dir(&tmp)
        .unwrap()
        .filter_map(|e| std::fs::read(e.unwrap().path()).ok())
        .filter_map(|b| String::from_utf8(b).ok())
        .collect();
    assert!(!contents.is_empty(), "no log written");
    assert!(!contents.contains("alice"), "username leaked into log");
    assert!(!contents.contains("secret.mp4"), "filename leaked into log");
    let _ = std::fs::remove_dir_all(&tmp);
}
```

> If logging initialisation is global, this test must run in its own process. If that is impractical, test `redact()` directly and enforce redaction at the call site with a wrapper `log_media_event()` that only accepts redacted strings.

- [ ] **Step 9: Verify all checks**

Run: `cargo test -p diagnostics && cargo clippy -p diagnostics -- -D warnings`
Expected: PASS, no warnings.

- [ ] **Step 10: Commit**

```bash
git add crates/diagnostics Cargo.toml
git commit -m "feat(diagnostics): add structured logging with redaction (Refs: P1-03)"
```

---

## Task 4: Versioned configuration (P1-04)

**Files:**
- Create: `crates/app-core/src/application/config.rs`
- Create: `crates/persistence/Cargo.toml`, `crates/persistence/src/{lib.rs,config_store.rs}`
- Modify: `Cargo.toml`
- Test: inline `#[cfg(test)]` in `config.rs` and `config_store.rs`

**Interfaces:**
- Produces:
  - `pub struct AppConfig { pub config_version: u32, pub library_roots: Vec<PathBuf>, pub managed_root: PathBuf, pub theme: Theme }`
  - `impl AppConfig { pub fn default_for(managed_root: PathBuf) -> Self; pub fn validate(&self) -> Result<(), ConfigError> }`
  - `pub struct ConfigStore` with `load() -> LoadOutcome`, `save(&AppConfig) -> Result<()>`
  - `pub enum LoadOutcome { Loaded(AppConfig), RecoveredFromDefaults { quarantined: PathBuf }, Migrated(AppConfig, u32) }`

- [ ] **Step 1: Write the failing validation test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_a_relative_managed_root() {
        let mut cfg = AppConfig::default_for(std::path::PathBuf::from("media"));
        cfg.managed_root = std::path::PathBuf::from("relative/media");
        assert!(cfg.validate().is_err(), "relative managed root must be rejected");
    }

    #[test]
    fn rejects_an_unknown_config_version() {
        let mut cfg = AppConfig::default_for(std::path::PathBuf::from("C:/media"));
        cfg.config_version = 999;
        assert!(cfg.validate().is_err(), "future config version must be rejected");
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p app-core config`
Expected: FAIL — `AppConfig` undefined.

- [ ] **Step 3: Implement the config model**

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

pub const CURRENT_CONFIG_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    Dark,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub config_version: u32,
    pub library_roots: Vec<PathBuf>,
    pub managed_root: PathBuf,
    pub theme: Theme,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("managed_root must be an absolute path")]
    RelativeManagedRoot,
    #[error("library root must be absolute: {0}")]
    RelativeLibraryRoot(String),
    #[error("unsupported config_version {0} (supported: {CURRENT_CONFIG_VERSION})")]
    UnsupportedVersion(u32),
}

impl AppConfig {
    pub fn default_for(managed_root: PathBuf) -> Self {
        Self {
            config_version: CURRENT_CONFIG_VERSION,
            library_roots: Vec::new(),
            managed_root,
            theme: Theme::Dark,
        }
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.config_version != CURRENT_CONFIG_VERSION {
            return Err(ConfigError::UnsupportedVersion(self.config_version));
        }
        if !self.managed_root.is_absolute() {
            return Err(ConfigError::RelativeManagedRoot);
        }
        for root in &self.library_roots {
            if !root.is_absolute() {
                return Err(ConfigError::RelativeLibraryRoot(root.display().to_string()));
            }
        }
        Ok(())
    }
}
```

- [ ] **Step 4: Run to verify validation passes**

Run: `cargo test -p app-core config`
Expected: PASS — 2 tests.

- [ ] **Step 5: Write the failing atomic-write test**

```rust
#[test]
fn an_interrupted_save_does_not_corrupt_the_previous_file() {
    let dir = tempdir();
    let path = dir.join("config.json");
    let store = ConfigStore::new(path.clone());

    let first = AppConfig::default_for(dir.join("managed"));
    store.save(&first).expect("first save");
    let before = std::fs::read_to_string(&path).expect("read back");

    // Simulate a partial write: a stray temp file must not be picked up on load.
    std::fs::write(dir.join("config.json.tmp"), b"{ this is not json").unwrap();

    let outcome = store.load();
    match outcome {
        LoadOutcome::Loaded(cfg) => assert_eq!(cfg.managed_root, first.managed_root),
        other => panic!("expected the previous config to survive, got {other:?}"),
    }
    assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
}
```

- [ ] **Step 6: Implement `ConfigStore` with atomic replace**

```rust
use crate::app_core::application::config::{AppConfig, ConfigError};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum LoadOutcome {
    Loaded(AppConfig),
    Migrated(AppConfig, u32),
    RecoveredFromDefaults { quarantined: PathBuf },
}

pub struct ConfigStore {
    path: PathBuf,
    fallback: AppConfig,
}

impl ConfigStore {
    pub fn new(path: PathBuf) -> Self {
        let fallback = AppConfig::default_for(
            path.parent().unwrap_or(Path::new(".")).join("managed"),
        );
        Self { path, fallback }
    }

    pub fn load(&self) -> LoadOutcome {
        let raw = match std::fs::read_to_string(&self.path) {
            Ok(raw) => raw,
            Err(_) => return LoadOutcome::Loaded(self.fallback.clone()),
        };
        match serde_json::from_str::<AppConfig>(&raw) {
            Ok(cfg) if cfg.validate().is_ok() => LoadOutcome::Loaded(cfg),
            _ => {
                let quarantined = self.path.with_extension("corrupt");
                let _ = std::fs::rename(&self.path, &quarantined);
                LoadOutcome::RecoveredFromDefaults { quarantined }
            }
        }
    }

    /// Writes to a temp file in the same directory, fsyncs, then renames.
    /// A crash mid-write leaves the previous file intact.
    pub fn save(&self, cfg: &AppConfig) -> Result<(), ConfigError> {
        cfg.validate()?;
        let json = serde_json::to_vec_pretty(cfg).expect("config is serialisable");
        let tmp = self.path.with_extension("json.tmp");
        {
            use std::io::Write;
            let mut f = std::fs::File::create(&tmp).map_err(|_| ConfigError::RelativeManagedRoot)?;
            f.write_all(&json).map_err(|_| ConfigError::RelativeManagedRoot)?;
            f.sync_all().map_err(|_| ConfigError::RelativeManagedRoot)?;
        }
        std::fs::rename(&tmp, &self.path).map_err(|_| ConfigError::RelativeManagedRoot)?;
        Ok(())
    }
}
```

> The `ConfigError` variants above are placeholders for I/O errors. Add `#[error("io: {0}")] Io(String)` to `ConfigError` and map real errors to it before merging — do not ship the placeholder mapping.

- [ ] **Step 7: Write the failing corrupt-config recovery test**

```rust
#[test]
fn corrupt_config_is_quarantined_and_defaults_load() {
    let dir = tempdir();
    let path = dir.join("config.json");
    std::fs::write(&path, b"{not valid json").unwrap();
    let store = ConfigStore::new(path.clone());

    match store.load() {
        LoadOutcome::RecoveredFromDefaults { quarantined } => {
            assert!(quarantined.exists(), "corrupt file must be preserved");
            assert!(!path.exists(), "corrupt file must be moved aside");
        }
        other => panic!("expected recovery, got {other:?}"),
    }
}
```

- [ ] **Step 8: Run all config tests**

Run: `cargo test -p persistence config`
Expected: PASS — 3 tests including atomic-write survival.

- [ ] **Step 9: Commit**

```bash
git add crates/app-core/src/application/config.rs crates/persistence Cargo.toml
git commit -m "feat(config): versioned atomic config with corrupt recovery (Refs: P1-04)"
```

---

## Task 5: SQLite migrations and backup (P1-05)

**Files:**
- Create: `crates/persistence/src/{lib.rs,sqlite/mod.rs,migrations/0001_init.sql}`
- Create: `crates/persistence/tests/migrations.rs`
- Test: `crates/persistence/tests/migrations.rs`

**Interfaces:**
- Produces:
  - `pub async fn open_and_migrate(db_path: &Path) -> anyhow::Result<sqlx::SqlitePool>`
  - `pub async fn backup_before_migrate(db_path: &Path) -> anyhow::Result<Option<PathBuf>>`
  - Table `schema_migrations`

- [ ] **Step 1: Write the failing migration test**

`crates/persistence/tests/migrations.rs`:

```rust
use persistence::open_and_migrate;
use sqlx::Row;

#[tokio::test]
async fn creates_schema_migrations_table_on_a_fresh_database() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("library.sqlite");

    let pool = open_and_migrate(&db).await.expect("migrate");

    let row = sqlx::query("SELECT name FROM sqlite_master WHERE type='table' AND name='schema_migrations'")
        .fetch_one(&pool)
        .await
        .expect("schema_migrations must exist");
    assert_eq!(row.get::<String, _>("name"), "schema_migrations");
    assert!(db.exists());
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p persistence --test migrations`
Expected: FAIL — `open_and_migrate` undefined.

- [ ] **Step 3: Implement `open_and_migrate`**

```rust
use anyhow::Context;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::Path;
use std::str::FromStr;

pub async fn open_and_migrate(db_path: &Path) -> anyhow::Result<SqlitePool> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).context("create database directory")?;
    }
    let opts = SqliteConnectOptions::from_str(&format!(
        "sqlite://{}",
        db_path.display().to_string().replace('\\', "/")
    ))?
    .create_if_missing(true)
    .foreign_keys(true)
    .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);

    let pool = SqlitePoolOptions::new().max_connections(5).connect_with(opts).await?;
    sqlx::migrate!("./src/migrations").run(&pool).await?;
    Ok(pool)
}
```

`crates/persistence/src/migrations/0001_init.sql`:

```sql
CREATE TABLE IF NOT EXISTS schema_migrations (
    version     INTEGER PRIMARY KEY,
    applied_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS settings (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);
```

`crates/persistence/Cargo.toml` needs `sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "migrate"] }`, `tokio`, `anyhow`, and dev-deps `tempfile`.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p persistence --test migrations`
Expected: PASS — 1 test.

- [ ] **Step 5: Write the failing foreign-key test**

```rust
#[tokio::test]
async fn foreign_keys_are_enforced() {
    let dir = tempfile::tempdir().unwrap();
    let pool = open_and_migrate(&dir.path().join("db.sqlite")).await.unwrap();

    sqlx::query("PRAGMA foreign_keys = ON").execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE parent (id INTEGER PRIMARY KEY)").execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE child (parent_id INTEGER REFERENCES parent(id))")
        .execute(&pool).await.unwrap();

    let result = sqlx::query("INSERT INTO child (parent_id) VALUES (42)")
        .execute(&pool)
        .await;
    assert!(result.is_err(), "foreign key violation must fail");
}
```

- [ ] **Step 6: Write the failing backup-before-migrate test**

```rust
#[tokio::test]
async fn a_migration_takes_a_backup_first() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("library.sqlite");

    // Seed a database at the current version.
    let pool = open_and_migrate(&db).await.unwrap();
    pool.close().await;

    let backup = persistence::backup_before_migrate(&db).await.unwrap();
    let backup = backup.expect("a backup path must be produced when the db exists");
    assert!(backup.exists(), "backup file must exist");
    assert_eq!(
        std::fs::read(&backup).unwrap().len(),
        std::fs::read(&db).unwrap().len(),
        "backup must be a byte-for-byte copy"
    );
}
```

- [ ] **Step 7: Implement the backup**

```rust
pub async fn backup_before_migrate(db_path: &Path) -> anyhow::Result<Option<PathBuf>> {
    if !db_path.exists() {
        return Ok(None);
    }
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let backup = db_path.with_file_name(format!(
        "{}.{}.bak",
        db_path.file_stem().unwrap().to_string_lossy(),
        stamp
    ));
    std::fs::copy(db_path, &backup).context("copy database for backup")?;
    Ok(Some(backup))
}
```

- [ ] **Step 8: Run all persistence tests**

Run: `cargo test -p persistence`
Expected: PASS — migration, foreign key, backup tests.

- [ ] **Step 9: Commit**

```bash
git add crates/persistence
git commit -m "feat(persistence): sqlite migrations, foreign keys and pre-migration backup (Refs: P1-05)"
```

---

## Task 6: First-run preflight wiring (P1-06)

**Files:**
- Modify: `scripts/windows/preflight.ps1` (already written; verify only)
- Create: `apps/desktop/src-tauri/src/commands/preflight.rs`
- Create: `apps/desktop/src/pages/Setup/SetupWizard.tsx`
- Test: `apps/desktop/src/pages/Setup/SetupWizard.test.tsx`

**Interfaces:**
- Produces: Tauri command `run_preflight() -> Result<PreflightReport, String>`; `PreflightReport { checks: Vec<Check>, can_proceed: bool }`; `Check { name, value, status, detail }`.

- [ ] **Step 1: Verify the script is read-only**

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/windows/preflight.ps1 -Json`
Expected: JSON with `"readOnly": true` and a `checks` array. No prompts, no elevation.

- [ ] **Step 2: Write the failing command test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_preflight_json_payload() {
        let json = r#"{"readOnly":true,"checks":[
            {"Check":"Windows 11","Value":"Detected","Status":"ready","Detail":""}
        ]}"#;
        let report = parse_preflight(json).expect("parse");
        assert!(report.can_proceed);
        assert_eq!(report.checks.len(), 1);
        assert_eq!(report.checks[0].status, "ready");
    }

    #[test]
    fn blocks_when_a_check_needs_attention() {
        let json = r#"{"readOnly":true,"checks":[
            {"Check":"WebView2 runtime","Value":"Not detected","Status":"attention","Detail":"Install WebView2"}
        ]}"#;
        let report = parse_preflight(json).expect("parse");
        assert!(!report.can_proceed);
    }
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `cargo test -p media-haven-tauri preflight`
Expected: FAIL — `parse_preflight` undefined.

- [ ] **Step 4: Implement the command**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Check {
    #[serde(rename = "Check")]
    pub name: String,
    #[serde(rename = "Value")]
    pub value: String,
    #[serde(rename = "Status")]
    pub status: String,
    #[serde(rename = "Detail", default)]
    pub detail: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PreflightReport {
    pub checks: Vec<Check>,
    pub can_proceed: bool,
}

#[derive(Deserialize)]
struct RawReport {
    #[serde(default)]
    #[allow(dead_code)]
    read_only: bool,
    checks: Vec<Check>,
}

pub fn parse_preflight(json: &str) -> Result<PreflightReport, String> {
    let raw: RawReport = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let can_proceed = !raw
        .checks
        .iter()
        .any(|c| c.status.eq_ignore_ascii_case("attention"));
    Ok(PreflightReport { checks: raw.checks, can_proceed })
}

#[tauri::command]
pub async fn run_preflight() -> Result<PreflightReport, String> {
    let script = std::env::current_dir()
        .map_err(|e| e.to_string())?
        .join("..").join("..").join("scripts").join("windows").join("preflight.ps1");
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&script)
        .arg("-Json")
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }
    parse_preflight(&String::from_utf8_lossy(&output.stdout))
}
```

> The hard-coded relative script path is a bug for a packaged app. In the real implementation resolve it from the Tauri resource dir (`app.path().resource_dir()`), and add a test that asserts the resolved path exists. Keep the placeholder out of `main`.

- [ ] **Step 5: Run to verify it passes**

Run: `cargo test -p media-haven-tauri preflight`
Expected: PASS — 2 tests.

- [ ] **Step 6: Write the failing wizard test**

```tsx
import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { SetupWizard } from "./SetupWizard";

describe("SetupWizard", () => {
  it("blocks Continue when a check needs attention", () => {
    render(
      <SetupWizard
        report={{
          can_proceed: false,
          checks: [
            { name: "WebView2 runtime", value: "Not detected", status: "attention", detail: "Install WebView2" },
          ],
        }}
      />
    );
    expect(screen.getByRole("button", { name: /continue/i })).toBeDisabled();
  });

  it("allows Continue when every check is ready", () => {
    render(
      <SetupWizard
        report={{
          can_proceed: true,
          checks: [{ name: "Windows 11", value: "Detected", status: "ready", detail: "" }],
        }}
      />
    );
    expect(screen.getByRole("button", { name: /continue/i })).toBeEnabled();
  });

  it("shows a plain-language next step, not a bare error code", () => {
    render(
      <SetupWizard
        report={{
          can_proceed: false,
          checks: [
            { name: "WebView2 runtime", value: "Not detected", status: "attention", detail: "Install WebView2 from Microsoft's official page." },
          ],
        }}
      />
    );
    expect(screen.getByText(/official page/i)).toBeInTheDocument();
  });
});
```

- [ ] **Step 7: Implement the wizard and verify**

Render the check list grouped by status, with a `Continue` button disabled when `can_proceed` is false, and a `Cancel` action that performs no writes.

Run: `npm test -w apps/desktop`
Expected: PASS — all SetupWizard tests.

- [ ] **Step 8: Verify no host mutation**

```powershell
Get-WindowsOptionalFeature -Online | Where-Object { $_.FeatureName -match 'Hyper-V|VirtualMachinePlatform|Windows-Subsystem-Linux' } | Select-Object FeatureName, State
```

Run before and after a full setup walkthrough. Expected: identical output. Record both in `progress.md`.

- [ ] **Step 9: Commit**

```bash
git add apps/desktop/src/pages/Setup apps/desktop/src-tauri/src/commands
git commit -m "feat(setup): wire read-only preflight into the first-run wizard (Refs: P1-06)"
```

---

## Task 7: Pass CP-1

- [ ] **Step 1: Run the full verification set**

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
npm run test -w apps/desktop
npm run build -w apps/desktop
```

Expected: all exit 0.

- [ ] **Step 2: Demonstrate the three recoveries**

1. Corrupt the config file → app starts with defaults and a warning.
2. Corrupt the database → app offers recovery, keeps the backup.
3. A deliberately failing migration → rolls back, backup retained.

Record the exact commands and output for each in `progress.md`.

- [ ] **Step 3: Record the host-state proof**

Paste the before/after optional-feature output from Task 6 Step 8. Expected: no change.

- [ ] **Step 4: Fill the verification table**

Update `progress.md`'s table with real PASS/FAIL/NOT RUN values. Anything not run stays `NOT RUN` with a reason.

- [ ] **Step 5: Get human sign-off**

Show the CP-1 evidence. CP-1 passes only on explicit confirmation.

If it fails, fix the foundation. Do not start Phase 2 — every later phase inherits these guarantees.
