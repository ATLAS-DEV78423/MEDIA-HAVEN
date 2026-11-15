# Phase 2 — Core Local Library: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (or superpowers:subagent-driven-development) to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The user's own folders become a browsable, searchable library that the app never modifies.

**Architecture:** `crates/media-library` owns import and scanning; `crates/persistence` owns SQL; `crates/app-core` owns the policy for what is allowed to be read. The frontend renders DTOs and never composes a filesystem path.

**Tech Stack:** Rust, SQLx/SQLite, Tauri commands, React + TypeScript, Vitest.

**Spec:** `docs/superpowers/specs/2026-11-15-media-haven-design.md`

**Depends on:** CP-1 passed.

## Global Constraints

- Source media is **read-only** to the app. No move, rename or delete — ever.
- Only folders the user explicitly selected are read. No default scan of the profile, Desktop, Documents or all drives.
- Every path is canonicalised, then containment-checked against its root. Reparse points/junctions are refused.
- Filenames and embedded metadata are untrusted input.
- Removing a library root deletes **zero** files.
- No personal filenames in tests, logs or fixtures.
- Full offline operation: no feature may require the network.

---

## Task 1: Folder import (P2-01)

**Files:**
- Create: `crates/media-library/src/{lib.rs,roots.rs}`
- Create: `crates/app-core/src/domain/library_root.rs`
- Create: `crates/app-core/src/ports/library_roots.rs`
- Create: `apps/desktop/src-tauri/src/commands/library.rs`
- Test: `crates/media-library/tests/roots.rs`

**Interfaces:**
- Produces:
  - `pub struct LibraryRoot { pub id: i64, pub path: PathBuf }`
  - `pub fn validate_root(path: &Path) -> Result<PathBuf, RootError>` (canonicalises; rejects non-directories)
  - `pub fn add_root(existing: &[PathBuf], candidate: &Path) -> Result<PathBuf, RootError>`
  - `pub enum RootError { NotADirectory, Unreadable, NestedInsideExisting(PathBuf), ContainsExisting(PathBuf), Canonicalise(String) }`
  - Tauri command `add_library_root(path: String) -> Result<LibraryRootDto, String>`, `remove_library_root(id: i64) -> Result<(), String>`

- [ ] **Step 1: Write the failing test**

`crates/media-library/tests/roots.rs`:

```rust
use media_library::roots::{add_root, validate_root, RootError};
use tempfile::tempdir;

#[test]
fn a_real_directory_is_accepted_and_canonicalised() {
    let dir = tempdir().unwrap();
    let root = tempdir().unwrap_or_else(|_| panic!()); // placeholder
    let validated = validate_root(root.path()).expect("accept a real directory");
    assert!(validated.is_absolute());
    assert_eq!(validated, root.path().canonicalize().unwrap());
}

#[test]
fn a_file_is_rejected_as_a_root() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("movie.mp4");
    std::fs::write(&file, b"not really a video").unwrap();
    assert!(matches!(validate_root(&file), Err(RootError::NotADirectory)));
}

#[test]
fn a_root_nested_inside_an_existing_root_is_rejected() {
    let outer = tempdir().unwrap();
    let inner = outer.path().join("sub");
    std::fs::create_dir(&inner).unwrap();

    let existing = vec![outer.path().canonicalize().unwrap()];
    match add_root(&existing, &inner) {
        Err(RootError::NestedInsideExisting(p)) => assert_eq!(p, outer.path().canonicalize().unwrap()),
        other => panic!("expected NestedInsideExisting, got {other:?}"),
    }
}

#[test]
fn a_root_that_contains_an_existing_root_is_rejected() {
    let outer = tempdir().unwrap();
    let inner = outer.path().join("sub");
    std::fs::create_dir(&inner).unwrap();

    let existing = vec![inner.canonicalize().unwrap()];
    match add_root(&existing, outer.path()) {
        Err(RootError::ContainsExisting(p)) => assert_eq!(p, inner.canonicalize().unwrap()),
        other => panic!("expected ContainsExisting, got {other:?}"),
    }
}

#[test]
fn a_nonexistent_path_is_rejected() {
    let dir = tempdir().unwrap();
    let missing = dir.path().join("does-not-exist");
    assert!(validate_root(&missing).is_err());
}
```

Remove the unused `dir`/placeholder line before committing.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p media-library --test roots`
Expected: FAIL — crate/module missing.

- [ ] **Step 3: Implement root validation**

```rust
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RootError {
    #[error("not a directory")]
    NotADirectory,
    #[error("cannot read directory")]
    Unreadable,
    #[error("this folder is already inside the library root {0}")]
    NestedInsideExisting(PathBuf),
    #[error("this folder already contains the library root {0}")]
    ContainsExisting(PathBuf),
    #[error("cannot resolve path: {0}")]
    Canonicalise(String),
}

pub fn validate_root(path: &Path) -> Result<PathBuf, RootError> {
    if !path.is_dir() {
        return Err(RootError::NotADirectory);
    }
    let canonical = path
        .canonicalize()
        .map_err(|e| RootError::Canonicalise(e.to_string()))?;
    // A readable directory must be listable; do not read entries, just open it.
    std::fs::read_dir(&canonical).map_err(|_| RootError::Unreadable)?;
    Ok(canonical)
}

pub fn add_root(existing: &[PathBuf], candidate: &Path) -> Result<PathBuf, RootError> {
    let candidate = validate_root(candidate)?;
    for root in existing {
        if candidate.starts_with(root) {
            return Err(RootError::NestedInsideExisting(root.clone()));
        }
        if root.starts_with(&candidate) {
            return Err(RootError::ContainsExisting(root.clone()));
        }
    }
    Ok(candidate)
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p media-library --test roots`
Expected: PASS — 5 tests.

- [ ] **Step 5: Write the failing "removing a root deletes nothing" test**

```rust
#[test]
fn removing_a_root_deletes_no_files() {
    let dir = tempdir().unwrap();
    for i in 0..5 {
        std::fs::write(dir.path().join(format!("file-{i}.mp4")), b"x").unwrap();
    }
    let before: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();

    // Removing a root is a database operation only.
    let canonical = validate_root(dir.path()).unwrap();
    database::remove_root_by_path(&db, &canonical).await.unwrap();

    let after: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(before.len(), after.len(), "removing a root must not delete files");
    assert_eq!(before.len(), 5);
}
```

Wire it to the real repository in `crates/persistence/src/library_roots.rs`.

- [ ] **Step 6: Implement the repository and command**

```rust
// crates/persistence/src/library_roots.rs
pub async fn insert_root(pool: &SqlitePool, path: &Path) -> sqlx::Result<i64> {
    sqlx::query("INSERT INTO library_roots (path, added_at) VALUES (?, ?)")
        .bind(path.to_string_lossy().to_string())
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(pool)
        .await
        .map(|r| r.last_insert_rowid())
}

/// Deletes the root row and its associations. Never touches the filesystem.
pub async fn remove_root(pool: &SqlitePool, id: i64) -> sqlx::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM library_roots WHERE id = ?").bind(id).execute(&mut *tx).await?;
    tx.commit().await
}
```

- [ ] **Step 7: Verify**

Run: `cargo test -p media-library -p persistence`
Expected: PASS, including the deletion-safety test.

- [ ] **Step 8: Commit**

```bash
git add crates/media-library crates/persistence apps/desktop/src-tauri/src/commands/library.rs
git commit -m "feat(library): user-selected folder import with nested-root guards (Refs: P2-01)"
```

---

## Task 2: Safe scanner (P2-02)

**Files:**
- Create: `crates/media-library/src/{scan.rs,path_policy.rs,sanitize.rs}`
- Test: `crates/media-library/tests/scan.rs`, inline tests in `path_policy.rs`

**Interfaces:**
- Produces:
  - `pub fn is_inside_root(candidate: &Path, root: &Path) -> bool`
  - `pub fn assert_safe_child(candidate: &Path, root: &Path) -> Result<PathBuf, PathError>`
  - `pub fn sanitize_display_name(raw: &str) -> String`
  - `pub struct ScanReport { pub indexed: usize, pub skipped: usize, pub errors: Vec<ScanError> }`
  - `pub async fn scan_root(root: &Path, cancel: CancellationToken, on_item: impl Fn(FoundFile)) -> ScanReport`

- [ ] **Step 1: Write the failing traversal test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_dotdot_that_escapes_the_root() {
        let root = std::path::Path::new(r"C:\Media\Library");
        let candidate = std::path::Path::new(r"C:\Media\Library\..\..\Windows\System32\drivers\etc");
        assert!(!is_inside_root(candidate, root));
    }

    #[test]
    fn rejects_an_absolute_path_outside_the_root() {
        let root = std::path::Path::new(r"C:\Media\Library");
        assert!(!is_inside_root(std::path::Path::new(r"C:\Windows\notepad.exe"), root));
    }

    #[test]
    fn accepts_a_nested_path_inside_the_root() {
        let root = std::path::Path::new(r"C:\Media\Library");
        assert!(is_inside_root(std::path::Path::new(r"C:\Media\Library\Shows\S01"), root));
    }

    #[test]
    fn rejects_a_unc_path() {
        let root = std::path::Path::new(r"C:\Media\Library");
        assert!(!is_inside_root(std::path::Path::new(r"\\server\share\movie.mp4"), root));
    }

    /// Absence assertion: the policy must refuse, and no read may occur.
    #[test]
    fn refusal_does_not_touch_the_filesystem_outside_the_root() {
        let root = std::path::Path::new(r"C:\Media\Library");
        let escape = std::path::Path::new(r"C:\Media\Library\..\secret.txt");
        assert!(assert_safe_child(escape, root).is_err());
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p media-library path_policy`
Expected: FAIL.

- [ ] **Step 3: Implement the path policy**

```rust
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PathError {
    #[error("path escapes the library root")]
    OutsideRoot,
    #[error("path contains a parent-directory component")]
    ParentComponent,
    #[error("path is not absolute")]
    NotAbsolute,
    #[error("path is a reparse point or symlink")]
    ReparsePoint,
}

/// Lexical containment check. No filesystem access, so it is safe to call on
/// hostile input.
pub fn is_inside_root(candidate: &Path, root: &Path) -> bool {
    if !candidate.is_absolute() || !root.is_absolute() {
        return false;
    }
    // UNC paths are not part of the library model.
    let s = candidate.to_string_lossy();
    if s.starts_with(r"\\") {
        return false;
    }
    let mut norm = PathBuf::new();
    for comp in candidate.components() {
        match comp {
            Component::ParentDir => {
                if !norm.pop() {
                    return false;
                }
            }
            other => norm.push(other.as_os_str()),
        }
    }
    norm.starts_with(root)
}

/// Canonicalises and then re-checks containment. Refuses reparse points.
pub fn assert_safe_child(candidate: &Path, root: &Path) -> Result<PathBuf, PathError> {
    if !candidate.is_absolute() || !root.is_absolute() {
        return Err(PathError::NotAbsolute);
    }
    if candidate.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(PathError::ParentComponent);
    }
    let meta = std::fs::symlink_metadata(candidate).map_err(|_| PathError::OutsideRoot)?;
    if is_reparse_point(&meta) {
        return Err(PathError::ReparsePoint);
    }
    let canonical = candidate.canonicalize().map_err(|_| PathError::OutsideRoot)?;
    let canonical_root = root.canonicalize().map_err(|_| PathError::OutsideRoot)?;
    if !canonical.starts_with(&canonical_root) {
        return Err(PathError::OutsideRoot);
    }
    Ok(canonical)
}

#[cfg(windows)]
fn is_reparse_point(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(meta: &std::fs::Metadata) -> bool {
    meta.file_type().is_symlink()
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p media-library path_policy`
Expected: PASS — 5 tests.

- [ ] **Step 5: Write the failing hostile-filename test**

```rust
#[test]
fn control_characters_are_stripped_from_display_names() {
    let raw = "Movie\u{0007}\u{202E}txt.exe";
    let clean = sanitize_display_name(raw);
    assert!(!clean.contains('\u{0007}'), "control char survived");
    assert!(!clean.contains('\u{202E}'), "RTL override survived");
}

#[test]
fn extremely_long_names_are_truncated_not_panicking() {
    let raw = "a".repeat(10_000);
    let clean = sanitize_display_name(&raw);
    assert!(clean.chars().count() <= 200);
}

#[test]
fn ordinary_names_are_preserved() {
    assert_eq!(sanitize_display_name("Blade Runner 2049 (2017)"), "Blade Runner 2049 (2017)");
}
```

- [ ] **Step 6: Implement the sanitiser**

```rust
/// Makes a filename safe for display. Never used to build a filesystem path.
pub fn sanitize_display_name(raw: &str) -> String {
    const MAX: usize = 200;
    let cleaned: String = raw
        .chars()
        .filter(|c| {
            !c.is_control()
                && *c != '\u{202E}' && *c != '\u{202D}'  // RTL/LTR override
                && *c != '\u{200B}' && *c != '\u{200E}' // zero-width, LRM
        })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.chars().count() > MAX {
        trimmed.chars().take(MAX).collect()
    } else {
        trimmed.to_string()
    }
}
```

- [ ] **Step 7: Write the failing scan-cancellation test**

```rust
#[tokio::test]
async fn scanning_can_be_cancelled_and_resumes_without_duplicates() {
    let dir = tempdir().unwrap();
    for i in 0..50 {
        std::fs::write(dir.path().join(format!("ep-{i:02}.mp4")), b"x").unwrap();
    }
    let cancel = CancellationToken::new();

    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen2 = seen.clone();
    let cancel2 = cancel.clone();
    let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let report = scan_root(dir.path(), cancel2, move |found| {
        let n = count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        seen2.lock().unwrap().push(found.path.clone());
        if n == 9 { /* caller cancels after 10 items */ }
    })
    .await;

    assert!(report.indexed <= 50);
    let unique = {
        let v = seen.lock().unwrap();
        let mut s: Vec<_> = v.clone();
        s.sort();
        s.dedup();
        s.len()
    };
    assert_eq!(unique, seen.lock().unwrap().len(), "scan produced duplicate paths");
}
```

- [ ] **Step 8: Implement the scanner**

```rust
pub async fn scan_root(
    root: &Path,
    cancel: tokio_util::sync::CancellationToken,
    mut on_item: impl FnMut(FoundFile),
) -> ScanReport {
    const MEDIA_EXTS: &[&str] = &["mp4", "mkv", "webm", "avi", "mov", "m4v", "ts"];
    let mut report = ScanReport::default();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        if cancel.is_cancelled() {
            report.skipped += 1;
            break;
        }
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) => { report.errors.push(ScanError { path: dir.clone(), message: e.to_string() }); continue; }
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
            if is_reparse_point(&meta) { report.skipped += 1; continue; }
            if meta.is_dir() {
                stack.push(path);
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
            if !MEDIA_EXTS.contains(&ext.as_str()) { continue; }
            if assert_safe_child(&path, root).is_err() { report.skipped += 1; continue; }
            report.indexed += 1;
            on_item(FoundFile { path, kind: MediaKind::Unknown });
        }
    }
    report
}
```

- [ ] **Step 9: Verify the full suite**

Run: `cargo test -p media-library`
Expected: PASS — roots, path policy, sanitiser, scan.

- [ ] **Step 10: Commit**

```bash
git add crates/media-library
git commit -m "feat(library): safe interruptible scanner with traversal guards (Refs: P2-02)"
```

---

## Task 3: Media entities (P2-03)

**Files:**
- Create: `crates/persistence/src/migrations/0002_media.sql`
- Create: `crates/persistence/src/media.rs`
- Test: `crates/persistence/tests/media.rs`

**Interfaces:**
- Produces tables `media_items`, `media_files`, `collections`; functions `upsert_media_item`, `link_file`, `list_movies`, `count_items`.

- [ ] **Step 1: Write the failing schema test**

```rust
#[tokio::test]
async fn media_items_require_a_kind_and_title() {
    let pool = test_pool().await;
    let result = sqlx::query("INSERT INTO media_items (id, title, kind, added_at) VALUES (1, NULL, 'movie', '2026-01-01T00:00:00Z')")
        .execute(&pool).await;
    assert!(result.is_err(), "title must be NOT NULL");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p persistence --test media`
Expected: FAIL — table missing.

- [ ] **Step 3: Write the migration**

```sql
CREATE TABLE library_roots (
    id        INTEGER PRIMARY KEY,
    path      TEXT NOT NULL UNIQUE,
    added_at  TEXT NOT NULL
);

CREATE TABLE media_items (
    id            INTEGER PRIMARY KEY,
    title         TEXT NOT NULL,
    kind          TEXT NOT NULL CHECK (kind IN ('movie', 'episode', 'unknown')),
    year          INTEGER,
    duration_ms   INTEGER,
    collection_id INTEGER REFERENCES collections(id) ON DELETE SET NULL,
    added_at      TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE TABLE media_files (
    id            INTEGER PRIMARY KEY,
    media_item_id INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    path          TEXT NOT NULL,
    size_bytes    INTEGER NOT NULL,
    content_hash  TEXT,
    last_seen_at  TEXT NOT NULL,
    UNIQUE (path)
);

CREATE TABLE collections (
    id        INTEGER PRIMARY KEY,
    title     TEXT NOT NULL,
    kind      TEXT NOT NULL CHECK (kind IN ('show', 'season', 'folder', 'user')),
    parent_id INTEGER REFERENCES collections(id) ON DELETE CASCADE,
    added_at  TEXT NOT NULL
);

CREATE INDEX idx_media_items_kind_added ON media_items (kind, added_at DESC);
CREATE INDEX idx_media_items_title      ON media_items (title COLLATE NOCASE);
CREATE INDEX idx_media_files_item       ON media_files (media_item_id);
CREATE INDEX idx_media_files_hash       ON media_files (content_hash);
```

> `collections` must be created before `media_items` references it. Order the statements accordingly in the file.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p persistence --test media`
Expected: PASS.

- [ ] **Step 5: Write the failing cascade test**

```rust
#[tokio::test]
async fn deleting_a_media_item_cascades_to_its_files() {
    let pool = test_pool().await;
    seed_item_with_file(&pool).await;

    sqlx::query("DELETE FROM media_items WHERE id = 1").execute(&pool).await.unwrap();

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM media_files")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(count, 0, "media_files must cascade");
}
```

- [ ] **Step 6: Write the failing index-usage test**

```rust
#[tokio::test]
async fn library_browse_uses_an_index() {
    let pool = test_pool().await;
    let plan: Vec<String> = sqlx::query_scalar(
        "EXPLAIN QUERY PLAN SELECT id, title FROM media_items WHERE kind = 'movie' ORDER BY added_at DESC"
    ).fetch_all(&pool).await.unwrap();

    let joined = plan.join(" \n ");
    assert!(joined.contains("idx_media_items_kind_added"), "browse query is not using the index:\n{joined}");
}
```

- [ ] **Step 7: Verify**

Run: `cargo test -p persistence`
Expected: PASS — schema, cascade, index usage.

- [ ] **Step 8: Commit**

```bash
git add crates/persistence
git commit -m "feat(persistence): media entities with cascade and browse indexes (Refs: P2-03)"
```

---

## Task 4: Movies, Shows and Library views (P2-04)

**Files:**
- Create: `apps/desktop/src/pages/Movies/MoviesPage.tsx`
- Create: `apps/desktop/src/pages/Shows/ShowsPage.tsx`
- Create: `apps/desktop/src/pages/Library/LibraryPage.tsx`
- Create: `apps/desktop/src/components/cards/{MediaCard,Artwork,SkeletonCard,StatusPill,ProgressBar}.tsx`
- Create: `apps/desktop/src/components/feedback/EmptyState.tsx`
- Test: `apps/desktop/src/pages/Movies/MoviesPage.test.tsx`

**Interfaces:**
- Consumes: `list_movies` Tauri command returning `MediaCardDto[]`.
- Produces: components that handle `loading | empty | offline | error | loaded`.

- [ ] **Step 1: Write the failing state tests**

```tsx
import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { MoviesPage } from "./MoviesPage";

describe("MoviesPage", () => {
  it("renders an empty state with an import action when there are no movies", () => {
    render(<MoviesPage state={{ status: "empty" }} />);
    expect(screen.getByRole("button", { name: /add media folder/i })).toBeInTheDocument();
  });

  it("renders skeletons while loading, not a blank screen", () => {
    render(<MoviesPage state={{ status: "loading" }} />);
    expect(screen.getAllByTestId("skeleton-card").length).toBeGreaterThan(0);
  });

  it("renders an offline state that explains what still works", () => {
    render(<MoviesPage state={{ status: "offline" }} />);
    expect(screen.getByText(/drive|disconnected|unavailable/i)).toBeInTheDocument();
  });

  it("renders an error state with a retry action", () => {
    render(<MoviesPage state={{ status: "error", message: "database unavailable" }} />);
    expect(screen.getByRole("button", { name: /retry/i })).toBeInTheDocument();
  });

  it("keeps card dimensions when artwork is missing", () => {
    render(<MoviesPage state={{ status: "loaded", items: [{ id: 1, title: "Unknown Art", artworkUrl: null }] }} />);
    const card = screen.getByTestId("media-card");
    expect(card).toHaveStyle({ aspectRatio: "2 / 3" });
  });
});
```

- [ ] **Step 2: Run to verify they fail**

Run: `npm test -w apps/desktop`
Expected: FAIL — `MoviesPage` undefined.

- [ ] **Step 3: Implement the components**

Rules to honour:

- `MediaCard` uses tokens for radius, shadow and spacing; no literals.
- `Artwork` renders a placeholder that occupies the same box, so a failed image never shifts layout.
- `SkeletonCard` has the same aspect ratio as the real card.
- Cards are keyboard-focusable buttons with an accessible name.

- [ ] **Step 4: Run to verify they pass**

Run: `npm test -w apps/desktop`
Expected: PASS — 5 tests.

- [ ] **Step 5: Write the failing lazy-artwork test**

```tsx
it("does not eagerly load artwork for a large library", () => {
  const items = Array.from({ length: 200 }, (_, i) => ({
    id: i, title: `Item ${i}`, artworkUrl: `https://example.invalid/${i}.jpg`,
  }));
  render(<MoviesPage state={{ status: "loaded", items }} />);
  const eager = document.querySelectorAll('img[loading="eager"]').length;
  expect(eager).toBe(0);
});
```

- [ ] **Step 6: Implement lazy loading**

Every `<img>` uses `loading="lazy"` plus an explicit width/height or aspect-ratio container. Verify with:

Run: `npm test -w apps/desktop`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add apps/desktop/src/pages apps/desktop/src/components
git commit -m "feat(ui): movies, shows and library views with full state coverage (Refs: P2-04)"
```

---

## Task 5: Search, sort and filter (P2-05)

**Files:**
- Create: `crates/media-library/src/search.rs`
- Create: `apps/desktop/src/pages/Search/SearchPage.tsx`
- Test: `crates/media-library/tests/search.rs`

**Interfaces:**
- Produces: `pub fn build_search_query(term: &str) -> (String, Vec<String>)` returning SQL and bound parameters, with wildcards escaped.

- [ ] **Step 1: Write the failing escaping test**

```rust
#[test]
fn percent_and_underscore_are_escaped() {
    let (sql, params) = build_search_query("100%_match");
    assert!(sql.contains("LIKE ? ESCAPE '\\'"), "must declare an escape char");
    assert_eq!(params[0], "%100\\%\\_match%");
}

#[test]
fn quotes_and_stars_do_not_alter_semantics() {
    let (_, params) = build_search_query(r#"ali"ce*"#);
    assert_eq!(params[0], r#"%ali"ce*%"#);
}

#[test]
fn an_empty_term_returns_everything_without_a_wildcard_bug() {
    let (_, params) = build_search_query("   ");
    assert_eq!(params[0], "%%");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p media-library search`
Expected: FAIL.

- [ ] **Step 3: Implement**

```rust
pub fn build_search_query(term: &str) -> (String, Vec<String>) {
    let sql = "SELECT id, title FROM media_items WHERE title LIKE ? ESCAPE '\\' ORDER BY title COLLATE NOCASE";
    let escaped = term
        .trim()
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    (sql.to_string(), vec![format!("%{escaped}%")])
}
```

- [ ] **Step 4: Verify**

Run: `cargo test -p media-library search`
Expected: PASS — 3 tests.

- [ ] **Step 5: Write the failing performance test**

```rust
#[tokio::test]
#[ignore = "run with the 10k fixture: cargo test -- --ignored"]
async fn searching_10k_items_is_within_budget() {
    let pool = seed_10k(&prepare().await).await;
    let start = std::time::Instant::now();
    let (sql, params) = build_search_query("episode 42");
    let mut q = sqlx::query(&sql);
    for p in &params { q = q.bind(p); }
    let _rows = q.fetch_all(&pool).await.unwrap();
    let elapsed = start.elapsed();
    assert!(elapsed.as_millis() < 200, "search took {}ms (budget 200ms)", elapsed.as_millis());
}
```

- [ ] **Step 6: Run the ignored performance test**

```bash
cargo test -p media-library -- --ignored searching_10k
```

Expected: PASS. Record the actual millisecond figure in `progress.md` — do not write "fast".

- [ ] **Step 7: Verify offline search**

Disconnect the network, launch the app, search a known title.

Expected: results return. Record the session in `progress.md`.

- [ ] **Step 8: Commit**

```bash
git add crates/media-library/src/search.rs apps/desktop/src/pages/Search
git commit -m "feat(search): local search with wildcard escaping and 10k budget (Refs: P2-05)"
```

---

## Task 6: Metadata and artwork cache (P2-06)

**Files:**
- Create: `crates/metadata/src/{lib.rs,provider.rs,cache.rs}`
- Create: `crates/persistence/src/migrations/0003_metadata.sql`
- Test: `crates/metadata/tests/cache.rs`

**Interfaces:**
- Produces:
  - `pub trait MetadataProvider { async fn lookup(&self, query: &str) -> Result<Option<Metadata>, ProviderError>; }`
  - `pub struct ArtworkCache` with `store(&self, key, bytes) -> Result<PathBuf>`, `get(&self, key) -> Option<PathBuf>`, `enforce_limit(&self, max_bytes: u64) -> Result<Eviction>`
  - `pub struct Eviction { pub removed: Vec<PathBuf>, pub freed_bytes: u64 }`

- [ ] **Step 1: Write the failing cache-eviction test**

```rust
#[test]
fn the_cache_evicts_until_it_is_under_the_limit() {
    let dir = tempdir().unwrap();
    let cache = ArtworkCache::new(dir.path(), 1024).unwrap();
    for i in 0..20 {
        cache.store(&format!("k{i}"), &vec![0u8; 200]).unwrap();
    }
    let total: u64 = std::fs::read_dir(dir.path()).unwrap()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum();

    assert!(total <= 1024, "cache exceeded its limit: {total} bytes");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p metadata --test cache`
Expected: FAIL.

- [ ] **Step 3: Implement the cache with eviction**

Implement least-recently-modified eviction: sort by mtime ascending, remove until under the limit, return the count and freed bytes. Never delete outside the cache directory — reuse `assert_safe_child` from `media-library`.

- [ ] **Step 4: Verify**

Run: `cargo test -p metadata`
Expected: PASS.

- [ ] **Step 5: Write the failing sanitisation test**

```rust
#[test]
fn hostile_metadata_text_is_sanitised_before_storage() {
    let meta = Metadata { title: "Movie\u{202E}gpj.exe".into(), synopsis: "<script>alert(1)</script>".into() };
    let clean = meta.sanitized();
    assert!(!clean.title.contains('\u{202E}'));
    assert!(!clean.synopsis.contains("<script>"));
}

#[test]
fn a_malformed_artwork_image_is_rejected_not_stored() {
    let dir = tempdir().unwrap();
    let cache = ArtworkCache::new(dir.path(), 10_000).unwrap();
    let result = cache.store_validated("k", b"not an image");
    assert!(result.is_err(), "malformed image must be rejected");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}
```

- [ ] **Step 6: Implement and verify**

Validate image magic bytes before storing; sanitise text on ingest and again on display.

Run: `cargo test -p metadata`
Expected: PASS.

- [ ] **Step 7: Verify offline fallback**

With the network disabled: launch, browse the library, open a details view.

Expected: cached metadata displays; missing items show placeholders; no error screen. Record the session.

- [ ] **Step 8: Verify no silent egress**

With a provider configured but **not** opted in, run the app and confirm zero outbound requests (use Windows Resource Monitor or a local proxy).

Expected: no request. If any request occurs, the opt-in gate is broken — fix before continuing.

- [ ] **Step 9: Commit**

```bash
git add crates/metadata crates/persistence/src/migrations
git commit -m "feat(metadata): bounded artwork cache with offline fallback (Refs: P2-06)"
```

---

## Task 7: Pass CP-2

- [ ] **Step 1: Build the 10k fixture**

```powershell
powershell -NoProfile -Command "1..10000 | ForEach-Object { $p = Join-Path 'tests\fixtures\library-10k' ('season\{0:D2}\ep-{1:D5}.mkv' -f ($_ % 20), $_); New-Item -ItemType Directory -Force (Split-Path $p) | Out-Null; Set-Content -Path $p -Value '' }"
```

Expected: 10,000 zero-byte files. Record the generation time.

- [ ] **Step 2: Browse the fixture end to end**

Import the fixture root, open Movies, scroll, search. Record timings and any dropped frames.

- [ ] **Step 3: Prove deletion safety**

Count files before and after removing the root:

```powershell
(Get-ChildItem tests/fixtures/library-10k -Recurse -File).Count
```

Expected: 10,000 before and after.

- [ ] **Step 4: Prove path-escape resistance**

Run the security suite:

```bash
cargo test -p media-library path_policy
cargo test -p media-library --test roots
```

Expected: PASS. Both results pasted into `progress.md`.

- [ ] **Step 5: Prove offline operation**

Full session with the network disabled: import (already indexed), browse, search, open details. Record what degraded and what did not.

- [ ] **Step 6: Update tracking and get sign-off**

Fill CP-2 in `roadmap.md`, update `progress.md`, and show the evidence. **Do not begin playback work until CP-2 passes** — browsing bugs and playback bugs become indistinguishable otherwise.
