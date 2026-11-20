# Interrupted Scan Recovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A process killed during a library scan leaves the root in an honest `error` state with a legible message instead of a stale `scanning` state; no media rows are changed.

**Architecture:** One repository method updates only `library_roots.status` and `error_message` for rows still marked `scanning`. The service exposes it; `core` calls it once at startup before serving. No schema change, no scanner change, no new dependency.

**Tech Stack:** Rust, `sqlx` (SQLite), existing `LibraryRepository` port, existing tests.

**Spec:** `roadmap.md` F1/P5-03 and `Development.md` §4.7 (no data loss, honest state).

## Global Constraints

- No new dependency, no migration, no host change, no Docker engine, no commit.
- Rust owns policy; the renderer is untouched. No renderer-facing DTO changes.
- Recovery never deletes rows or files. It changes only the status/message of roots stuck in `scanning`.
- Tests use in-memory/temp SQLite fixtures only; no personal data.
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` must pass.

---

### Task 1: Recover interrupted scans at startup

**Files:**
- Test: `crates/persistence/tests/repository.rs`
- Modify: `crates/app-core/src/ports/library.rs`
- Modify: `crates/app-core/src/application/library.rs`
- Modify: `crates/persistence/src/sqlite/library_repo.rs`
- Modify: `crates/core/src/server.rs`

**Interfaces:**
- Consumes: `LibraryRepository` trait, `LibraryRootStatus::{Scanning, Error}`, `LibraryRoot`, `AppError`.
- Produces: `LibraryRepository::recover_interrupted_scans() -> Result<usize, AppError>`; `LibraryService::recover_interrupted_scans() -> Result<usize, AppError>`.

- [ ] **Step 1: Write the failing test** in `crates/persistence/tests/repository.rs`:

```rust
#[tokio::test]
async fn interrupted_scan_is_marked_error_without_touching_media() {
    let ctx = TestContext::new().await;
    let root = ctx.repo.root_add("Recover", "/tmp/recover").await.unwrap();
    ctx.repo
        .root_update(
            &root.id,
            3,
            app_core::domain::LibraryRootStatus::Scanning,
            None,
            None,
        )
        .await
        .unwrap();

    let recovered = ctx.repo.recover_interrupted_scans().await.unwrap();
    assert_eq!(recovered, 1);

    let roots = ctx.repo.roots().await.unwrap().roots;
    let updated = roots.iter().find(|r| r.id == root.id).unwrap();
    assert_eq!(updated.status, app_core::domain::LibraryRootStatus::Error);
    assert_eq!(
        updated.error_message.as_deref(),
        Some("Scan interrupted by restart.")
    );

    // A second recovery is a no-op: the root is no longer in `scanning`.
    assert_eq!(ctx.repo.recover_interrupted_scans().await.unwrap(), 0);
}
```

- [ ] **Step 2: Run `cargo test -p media-persistence interrupted_scan_is_marked_error` and confirm it fails to compile** with "no method named `recover_interrupted_scans`".

- [ ] **Step 3: Add the trait method** to `LibraryRepository` in `crates/app-core/src/ports/library.rs`:

```rust
    /// Mark roots left in `scanning` by a killed process as `error`.
    /// Returns the number of roots recovered. Never deletes rows or media.
    async fn recover_interrupted_scans(&self) -> Result<usize, AppError>;
```

- [ ] **Step 4: Implement it** in `impl LibraryRepository for SqliteLibraryRepository` in `crates/persistence/src/sqlite/library_repo.rs`, next to `root_update`:

```rust
    async fn recover_interrupted_scans(&self) -> Result<usize, AppError> {
        let result = sqlx::query(
            "UPDATE library_roots SET status = 'error', \
             error_message = 'Scan interrupted by restart.' \
             WHERE status = 'scanning'",
        )
        .execute(&self.pool)
        .await
        .map_err(Self::internal)?;
        Ok(result.rows_affected() as usize)
    }
```

- [ ] **Step 5: Expose it on the service** in `crates/app-core/src/application/library.rs`:

```rust
    pub async fn recover_interrupted_scans(&self) -> Result<usize, AppError> {
        self.repository.recover_interrupted_scans().await
    }
```

- [ ] **Step 6: Call it at startup** in `crates/core/src/server.rs::run`, immediately after the service is built:

```rust
    let service = LibraryService::new(SqliteLibraryRepository::new(pool));
    let recovered = service.recover_interrupted_scans().await?;
    if recovered > 0 {
        tracing::warn!(recovered, "marked interrupted library scans as errors");
    }
```

- [ ] **Step 7: Run the targeted test**, then `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`. All must exit 0. Record exact output in `progress.md`.

- [ ] **Step 8: Commit** — NOT DONE. `AGENTS.md` requires explicit permission to commit; the current instruction does not grant it. Leave the changes uncommitted and report them.
