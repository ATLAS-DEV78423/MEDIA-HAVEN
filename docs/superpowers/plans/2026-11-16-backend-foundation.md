# Backend Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up a runnable `core` Rust backend that serves the frontend's
frozen command contract (`apps/desktop/src/types/media.ts`) from SQLite, so the
UI's data-dependent acceptance criteria can finally be exercised. The backend
runs on the host during development and is structured so it can be packaged into
the mandatory Docker `core` container without rework.

**Architecture:** A Cargo workspace with a domain crate (`app-core`), a
persistence crate (`persistence`), and a binary crate (`core`) that runs an
`axum` HTTP server exposing one named route per command in the contract. SQLite
via `sqlx` with embedded migrations. Bearer-token auth on every route except
`/healthz`. The frontend gains an HTTP transport used when
`VITE_BACKEND_URL` is set, so the dev mock remains the default.

**Tech Stack:** Rust 1.99 (pinned), Cargo workspace, `axum` 0.7, `tokio`,
`sqlx` (sqlite, runtime-tokio-rustls, macros, migrate), `serde`/`serde_json`,
`thiserror`, `tracing` + `tracing-subscriber`, `uuid`, `chrono`. Frontend:
Vite env var + fetch-based transport.

**Spec:** `Development.md` §3, §5, §6.2; `apps/desktop/src/types/media.ts`;
`roadmap.md` P1-01, P1-05, P1-08 (partial).

## Global Constraints

- No host-side backend fallback in production: the binary is built to run in the
  `core` container; running on the host is a dev-only convenience.
- Domain crates (`app-core`, `persistence`) must contain zero HTTP-transport,
  container-runtime or Tauri dependencies. Enforced by a dependency check.
- All input validated at the boundary. Errors normalised to the closed
  `AppErrorCode` set.
- `thiserror` for domain errors; `anyhow` only at application boundaries.
- No `unwrap()`/`expect()` on user-controlled input.
- Every path crossing a boundary is canonicalised and containment-checked.
- SQLite: foreign keys ON, WAL mode, UTC timestamps, migrations embedded.
- Tokens never in source, logs or plaintext config.
- Loopback-only binding in production; dev may bind 0.0.0.0 only when
  `MEDIAHAVEN_DEV=1`.

---

## Task 1: Rust workspace scaffold

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `rust-toolchain.toml`
- Create: `crates/app-core/Cargo.toml`, `crates/app-core/src/lib.rs`
- Create: `crates/persistence/Cargo.toml`, `crates/persistence/src/lib.rs`
- Create: `crates/core/Cargo.toml`, `crates/core/src/main.rs`
- Modify: `crates/README.md` (status line)

**Interfaces:**
- Produces: workspace members `app-core`, `persistence`, `core`; a `cargo build`
  that succeeds from a clean clone.

- [ ] **Step 1: Write the workspace manifest**

```toml
# Cargo.toml
[workspace]
resolver = "2"
members = [
  "crates/app-core",
  "crates/persistence",
  "crates/core",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
authors = ["MEDIA_HAVEN contributors"]

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
thiserror = "1"
anyhow = "1"
tracing = "1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4", "serde"] }
sqlx = { version = "0.8", features = ["runtime-tokio-rustls", "sqlite", "macros", "migrate", "chrono", "uuid"] }
axum = "0.7"
tower = "0.5"
tower-http = { version = "0.6", features = ["cors", "trace"] }
```

- [ ] **Step 2: Pin the toolchain**

```toml
# rust-toolchain.toml
[toolchain]
channel = "1.99.0"
components = ["rustfmt", "clippy"]
targets = ["x86_64-pc-windows-msvc"]
```

- [ ] **Step 3: Create crate manifests**

`crates/app-core/Cargo.toml`:
```toml
[package]
name = "app-core"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
chrono.workspace = true
uuid.workspace = true
```

`crates/persistence/Cargo.toml`:
```toml
[package]
name = "media-persistence"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
app-core = { path = "../app-core" }
sqlx.workspace = true
serde.workspace = true
thiserror.workspace = true
chrono.workspace = true
uuid.workspace = true
anyhow.workspace = true
```

`crates/core/Cargo.toml`:
```toml
[package]
name = "mediahaven-core"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
app-core = { path = "../app-core" }
media-persistence = { path = "../persistence" }
axum.workspace = true
tower.workspace = true
tower-http.workspace = true
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
anyhow.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
chrono.workspace = true
uuid.workspace = true
```

- [ ] **Step 4: Minimal lib/main files**

`crates/app-core/src/lib.rs`: `pub mod domain; pub mod ports; pub mod application;`
with empty submodules.
`crates/persistence/src/lib.rs`: `pub mod sqlite;`
`crates/core/src/main.rs`:
```rust
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_env_filter("info").init();
    tracing::info!("mediahaven-core starting");
}
```

- [ ] **Step 5: Build**

Run: `cargo build`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml rust-toolchain.toml crates/
git commit -m "feat: scaffold Rust workspace (app-core, persistence, core)"
```

---

## Task 2: app-core domain and ports

**Files:**
- Create: `crates/app-core/src/domain/mod.rs`, `media.rs`, `errors.rs`
- Create: `crates/app-core/src/ports/mod.rs`, `library.rs`, `persistence.rs`
- Create: `crates/app-core/src/application/mod.rs`, `library.rs`

**Interfaces:**
- Consumes: none.
- Produces: `MediaSummary`, `MediaKind`, `MediaAvailability`, `HomeScreen`,
  `MediaShelf`, `LibraryScreen`, `BrowseQuery`, `AppError`, `AppErrorCode`,
  `LibraryRepository` trait, `LibraryService` with `home()`, `movies()`,
  `search()`.

- [ ] **Step 1: Write failing tests for error code mapping and domain invariants**

Create `crates/app-core/tests/domain.rs` with tests asserting `AppErrorCode`
serialises to the exact frontend string, and that `MediaAvailability::MissingFile`
carries `lastSeenAt`.

- [ ] **Step 2: Run tests → FAIL**

Run: `cargo test -p app-core`
Expected: compile error (modules missing).

- [ ] **Step 3: Implement domain types mirroring `media.ts`**

Implement `MediaKind`, `MediaAvailability`, `MediaSummary`, `HomeScreen`,
`MediaShelf`, `ShelfId`, `LibraryScreen`, `BrowseQuery`, `SearchQuery`,
`SearchResults`, `AppErrorCode`, `AppError` with serde derives producing the
exact JSON shapes the frontend expects (camelCase fields).

- [ ] **Step 4: Implement ports**

```rust
// crates/app-core/src/ports/library.rs
#[async_trait::async_trait]
pub trait LibraryRepository: Send + Sync {
    async fn home(&self) -> Result<HomeScreen, AppError>;
    async fn movies(&self, query: &BrowseQuery) -> Result<Vec<MediaSummary>, AppError>;
    async fn search(&self, query: &SearchQuery) -> Result<SearchResults, AppError>;
    async fn roots(&self) -> Result<LibraryScreen, AppError>;
}
```

(Add `async-trait` to workspace deps.)

- [ ] **Step 5: Implement application service**

`LibraryService<R: LibraryRepository>` delegating to the repository.

- [ ] **Step 6: Run tests → PASS**

- [ ] **Step 7: Commit**

---

## Task 3: persistence crate — SQLite + migrations

**Files:**
- Create: `crates/persistence/migrations/0001_init.sql`
- Create: `crates/persistence/src/sqlite/mod.rs`, `pool.rs`, `library_repo.rs`
- Create: `crates/persistence/tests/migrations.rs`

**Interfaces:**
- Produces: `SqlitePool::connect`, `run_migrations`, `SqliteLibraryRepository`
  implementing `LibraryRepository`.

- [ ] **Step 1: Write migration test (failing)**

Test creates a temp db, runs migrations, asserts `media_items` table exists
and foreign keys are ON.

- [ ] **Step 2: Run → FAIL**

- [ ] **Step 3: Write migration SQL**

```sql
CREATE TABLE IF NOT EXISTS schema_migrations ( ... );
CREATE TABLE media_items (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL CHECK (kind IN ('movie','series','episode')),
  title TEXT NOT NULL,
  year INTEGER,
  synopsis TEXT,
  runtime_minutes INTEGER,
  genres TEXT NOT NULL DEFAULT '[]',
  availability_state TEXT NOT NULL DEFAULT 'available',
  availability_last_seen TEXT,
  progress_position REAL,
  progress_duration REAL,
  progress_updated_at TEXT,
  progress_completed INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE media_files (...);
CREATE TABLE library_roots (...);
CREATE TABLE watchlist_items (...);
CREATE TABLE queue_items (...);
CREATE INDEX idx_media_items_kind ON media_items(kind);
CREATE INDEX idx_media_items_title ON media_items(title);
```

- [ ] **Step 4: Implement pool + migrations + repository**

`SqliteLibraryRepository` implementing `LibraryRepository` with queries.

- [ ] **Step 5: Run tests → PASS**

- [ ] **Step 6: Commit**

---

## Task 4: core binary — HTTP server

**Files:**
- Create: `crates/core/src/server.rs`, `routes.rs`, `auth.rs`, `state.rs`
- Modify: `crates/core/src/main.rs`

**Interfaces:**
- Produces: binary listening on `0.0.0.0:8080` (dev) / `127.0.0.1:8080`,
  serving `/healthz`, `/readyz`, and one route per `Commands` key.

- [ ] **Step 1: Write auth-matrix test (failing)** — integration test in
  `crates/core/tests/auth.rs` asserting no-token / wrong-token / tampered-body
  return 401.

- [ ] **Step 2: Run → FAIL**

- [ ] **Step 3: Implement server**

Routes:
```
GET  /healthz  (unauth) -> {"status":"ok"}
GET  /readyz   (auth)   -> {"status":"ready"}
POST /library_home      -> HomeScreen
POST /library_movies    -> [MediaSummary]
...
```
Each route deserialises the request body into the command args, calls the
application service, serialises the result.

Auth middleware: extract `Authorization: Bearer <token>` and a signature
header `X-MediaHaven-Sig: HMAC-SHA256(token, body)`; compare in constant time.
In dev (`MEDIAHAVEN_DEV=1`), accept token from env `MEDIAHAVEN_API_TOKEN`
or a default dev token.

- [ ] **Step 4: Run auth test → PASS**

- [ ] **Step 5: Commit**

---

## Task 5: Seed data + frontend HTTP transport

**Files:**
- Create: `crates/core/src/seed.rs` (inserts a small fixture library on first
  run when `MEDIAHAVEN_SEED=1`)
- Modify: `apps/desktop/src/services/transport.ts` (add HTTP transport when
  `VITE_BACKEND_URL` set)
- Create: `apps/desktop/src/services/httpTransport.ts`

**Interfaces:**
- Frontend: `createTransport()` returns HTTP transport if
  `import.meta.env.VITE_BACKEND_URL` is set, else mock in dev / Tauri in prod.

- [ ] **Step 1: Implement seed**

- [ ] **Step 2: Implement HTTP transport mirroring the mock's invoke signature**

- [ ] **Step 3: Typecheck + build frontend**

Run: `cd apps/desktop && npm run typecheck && npm run build`
Expected: PASS.

- [ ] **Step 4: End-to-end smoke**

Run core with seed, run frontend with `VITE_BACKEND_URL=http://127.0.0.1:8080`,
open browser, verify Home renders seeded data.

- [ ] **Step 5: Commit**

---

## Task 6: Dockerfile for core

**Files:**
- Create: `infrastructure/containers/Dockerfile.core`
- Create: `.dockerignore`

**Interfaces:**
- Produces: an image that runs `mediahaven-core` as UID 10001, exposing 8080,
  with a healthcheck hitting `/healthz`.

- [ ] **Step 1: Multi-stage Dockerfile** (rust:1.x builder → debian:bookworm-slim
  runtime, non-root user 10001).

- [ ] **Step 2: Build image locally**

Run: `docker build -f infrastructure/containers/Dockerfile.core -t mediahaven-core:dev .`
Expected: PASS (image builds).

- [ ] **Step 3: Run container and hit /healthz**

Run: `docker run --rm -p 127.0.0.1:18081:8080 -e MEDIAHAVEN_API_TOKEN=dev mediahaven-core:dev`
then `curl http://127.0.0.1:18081/healthz`
Expected: `{"status":"ok"}`.

- [ ] **Step 4: Commit**

---

## Execution notes

- Run `cargo test` after every task.
- Run `cargo clippy -- -D warnings` before committing.
- Do not commit secrets. The dev token default is `dev-only-not-for-production`
  and is gated behind `MEDIAHAVEN_DEV=1`.
- Frontend changes must keep `npm run check:tokens` green.