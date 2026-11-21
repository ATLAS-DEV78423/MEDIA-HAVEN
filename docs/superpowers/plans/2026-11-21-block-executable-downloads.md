# Block Executable Downloads Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Guarantee that the authorized-media acquisition path never downloads an executable file: add each torrent **paused**, inspect its declared file names, reject and delete the task before any file bytes are written if it contains an executable, and resume only an all-media torrent.

**Architecture:** `TorrentApi::add` gains a `resume` counterpart; `QbClient::add` posts `paused=true` to qBittorrent's `/api/v2/torrents/add`, so a newly added torrent holds metadata but writes no file data. `Orchestrator::acquire` then calls `files()` on the paused task, runs a pure, case-insensitive executable-name check, and either `remove(hash, false)` + returns a new `AcquireError::Rejected`, or `resume(hash)` and continues through the existing status/store/inspection path. A bounded 5×200 ms poll covers a magnet whose metadata is not yet resolved; an empty or failed file list fails closed. This is a **pre-download** control layered in front of the existing **post-download** ffprobe/Defender gates — it does not replace them.

**Tech Stack:** Rust (std only; no new dependency). qBittorrent WebUI v2 API: `add` with `paused=true`, `torrents/files`, `torrents/resume`, `torrents/delete`. React UI unchanged except an honest error message already carried by the command response.

**Spec:** Human instruction 2026-11-21 ("stop and make sure no .exe files get downloaded when torrenting and stuff"); ADR-0018 (qBittorrent boundary, no bundled sources), ADR-0019 (app-managed cleanup), `Development.md` §8.4/§8.7 (filesystem safety, destructive ops), `AGENTS.md` (validate at the trust boundary; regression test for every fix; no new dependency; no commit unless permitted).

## Global Constraints

- **Zero executable bytes on disk.** Add paused; never resume a torrent whose declared files include an executable. Failure/empty/unreadable file list ⇒ reject and delete the task (`delete_files=false`).
- **Pre-download control only.** Do not remove, weaken or bypass the existing ffprobe (`inspection.rs`) or Windows Defender (`defender.rs`) gates; they still run on every completed download.
- qBittorrent control stays inside the `acquire` network from `core`; no path, hash-derived path or executable name reaches a renderer DTO. The rejection message is a fixed, path-free string.
- No new dependency. No scraper, indexer or content catalogue. Lawful authorized media only.
- Every fake `TorrentApi` in tests must implement `resume`; tests drive the reject path without qB or Docker.
- `PASS` / `FAIL` / `NOT RUN` reflect observed runs. A live qB check is NOT RUN (no lawful source).
- Commits are permitted on this worktree by the human's current instruction; push/merge/install/Docker/host changes are not.

---

## File Structure

| File | Responsibility |
|---|---|
| `crates/acquisition/src/executables.rs` (new) | Pure `has_executable_file(&[String]) -> bool` and the case-insensitive extension blocklist. |
| `crates/acquisition/src/qbittorrent.rs` | `TorrentApi::resume`; `QbClient::add` posts `paused=true`; expose `add_form` for a test assertion. |
| `crates/acquisition/src/orchestrator.rs` | `AcquireError::Rejected`; paused-add → file check → resume-or-reject flow; 5×200 ms metadata poll. |
| `crates/acquisition/src/lib.rs` | Re-export `has_executable_file`, `EXECUTABLE_EXTENSIONS`. |
| `crates/acquisition/tests/executables.rs` (new) | Blocklist, double-extension, path-separator and benign-media cases. |
| `crates/acquisition/src/orchestrator.rs` (tests) | Fake `resume` recording; reject / clean / empty-list cases. |
| `docs/decisions/0024-no-executable-downloads.md` (new) | Record the pre-download control, its limits, and that it complements the existing gates. |
| `roadmap.md`, `progress.md` | Status and evidence. |

---

## Task 1: Reject executable-bearing torrents before download

**Files:**
- Create: `docs/decisions/0024-no-executable-downloads.md`, `crates/acquisition/src/executables.rs`, `crates/acquisition/tests/executables.rs`
- Modify: `crates/acquisition/src/qbittorrent.rs`, `crates/acquisition/src/orchestrator.rs`, `crates/acquisition/src/lib.rs`, `roadmap.md`, `progress.md`

**Interfaces:**
- Consumes: `TorrentApi { add, status, files, remove }`, `Orchestrator::acquire`, `AcquireError`, `DownloadStore`, `SAVE_PATH`, `validate_source`.
- Produces:
  - `pub const EXECUTABLE_EXTENSIONS: &[&str]`
  - `pub fn has_executable_file(names: &[String]) -> bool`
  - `TorrentApi::resume(&self, hash: &str) -> Result<(), QbError>` (all fakes updated)
  - `QbClient::add_form<'a>(source: &'a str, save_path: &'a str) -> Vec<(&'a str, &'a str)>` — includes `("paused", "true")`
  - `AcquireError::Rejected` with message `"torrent contains an executable file or has no readable file list; nothing was downloaded"`

- [ ] **Step 1: Write ADR-0024 (status Accepted — human instruction 2026-11-21).**

```markdown
# ADR-0024: Reject executable-bearing torrents before download

- **Status:** Accepted (2026-11-21, human instruction)
- **Related:** ADR-0018, ADR-0019, Development.md §8.4/§8.7

## Context
The existing probe/Defender gates inspect a download only after qBittorrent has
written its bytes. A torrent may declare an `.exe` alongside media, so the app
could place an executable on disk before any gate refused to play it.

## Decision
Acquire every torrent paused. Read its declared file names through qB's files
endpoint, and refuse to resume if any name has a known executable extension
(exe, com, scr, pif, bat, cmd, msi, msp, jar, js, jse, vbs, vbe, wsf, wsh, hta,
ps1, ps1xml, lnk, reg, dll, ocx, sys, drv, cpl, msc, app, appx, msix) in its
final or any dotted segment. Case-insensitive. Empty/failed metadata fails
closed. A rejected task is removed with `deleteFiles=false` and no file bytes
are downloaded. This is a name-based pre-download control: it cannot inspect
bytes the client has not fetched, so the existing post-download media-parse and
Defender gates remain mandatory.

## Consequences
Positive: executables are never written to the download root by this app.
Negative: a file deliberately named with an executable-looking segment is
rejected even if it is media (the operator can rename the source); a magnet
whose metadata is not ready within ~1 s is rejected and can be retried. Both
fail closed by design.
```

- [ ] **Step 2: Write failing tests** in `crates/acquisition/tests/executables.rs`:

```rust
use media_acquisition::{has_executable_file, EXECUTABLE_EXTENSIONS};

#[test]
fn rejects_windows_executable_extensions_case_insensitively() {
    for name in ["movie.EXE", "tool.bat", "install.MSI", "payload.scr", "x.ps1", "a.dll"] {
        assert!(has_executable_file(&[name.to_string()]), "{name}");
    }
    assert!(!EXECUTABLE_EXTENSIONS.is_empty());
}

#[test]
fn rejects_double_extension_and_windows_path_separators() {
    assert!(has_executable_file(&[r"show\S01E01.mp4.exe".to_string()]));
    assert!(has_executable_file(&["show/S01E01.exe.mkv".to_string()]));
}

#[test]
fn allows_ordinary_media_names() {
    let names = vec![
        "show/S01E01.mkv".to_string(),
        "movie.2024.1080p.mp4".to_string(),
        "subtitle.en.srt".to_string(),
        "cover.jpg".to_string(),
    ];
    assert!(!has_executable_file(&names));
    assert!(!has_executable_file(&[]));
}
```

- [ ] **Step 3: Run them to verify they fail.**

Run: `cargo test -p media-acquisition --test executables`
Expected: compile error — `has_executable_file` / `EXECUTABLE_EXTENSIONS` missing.

- [ ] **Step 4: Implement `crates/acquisition/src/executables.rs`.**

```rust
//! Pure, name-only pre-download guard. qB writes no file bytes while a task is
//! paused, so rejecting a task here keeps executables off disk entirely.

pub const EXECUTABLE_EXTENSIONS: &[&str] = &[
    "exe", "com", "scr", "pif", "bat", "cmd", "msi", "msp", "jar", "js", "jse",
    "vbs", "vbe", "wsf", "wsh", "hta", "ps1", "ps1xml", "lnk", "reg", "dll",
    "ocx", "sys", "drv", "cpl", "msc", "app", "appx", "msix",
];

/// True when any declared file name carries a known executable extension,
/// either as its final extension or as an inner dotted segment. Both `/` and
/// `\` separators are accepted; matching is ASCII case-insensitive.
pub fn has_executable_file(names: &[String]) -> bool {
    names.iter().any(|name| {
        let file = name.rsplit(|ch| ch == '/' || ch == '\\').next().unwrap_or(name);
        file.split('.').skip(1).any(|part| {
            let part = part.trim().to_ascii_lowercase();
            EXECUTABLE_EXTENSIONS.iter().any(|ext| *ext == part)
        })
    })
}
```

`shortcut: name-based only — a deliberately renamed executable is caught by the existing post-download probe/Defender gates, not here; upgrade only if qB exposes byte inspection before download.`

- [ ] **Step 5: Re-export in `crates/acquisition/src/lib.rs`.**

```rust
pub use executables::{has_executable_file, EXECUTABLE_EXTENSIONS};
```

- [ ] **Step 6: Run the pure tests to verify they pass.**

Run: `cargo test -p media-acquisition --test executables`
Expected: PASS, 3/3.

- [ ] **Step 7: Write failing orchestrator tests** in `crates/acquisition/src/orchestrator.rs` (inside its `#[cfg(test)]` module). Extend the existing fake `TorrentApi` with a `resume` recorder, then add:

```rust
#[test]
fn executable_torrent_is_removed_without_resuming_or_storing() {
    let api = GuardApi {
        files: vec!["show/S01E01.mkv".into(), "installer.exe".into()],
        ..GuardApi::default()
    };
    let store = InMemoryDownloadStore::default();
    let err = Orchestrator::new(api, store).acquire(MAGNET).unwrap_err();
    assert!(matches!(err, AcquireError::Rejected));
    // GuardApi asserts on Drop that resume was never called and remove(hash,false) was.
}

#[test]
fn clean_torrent_is_resumed_and_stored() {
    let api = GuardApi { files: vec!["show/S01E01.mkv".into()], ..GuardApi::default() };
    let item = Orchestrator::new(api, InMemoryDownloadStore::default()).acquire(MAGNET).unwrap();
    assert!(!item.id.is_empty());
}

#[test]
fn empty_or_unreadable_file_list_is_rejected() {
    for files in [vec![], vec!["nope".into() /* no extension is fine, but empty must reject */]] {
        let api = GuardApi { files, files_error: files.is_empty(), ..GuardApi::default() };
        assert!(matches!(Orchestrator::new(api, InMemoryDownloadStore::default()).acquire(MAGNET), Err(AcquireError::Rejected)));
    }
}
```

Adapt `GuardApi` to the existing fake shape already in the file; the assertions listed above are the contract — resume count 0 and `remove(hash, false)` on rejection, resume count 1 and store insert on acceptance.

- [ ] **Step 8: Add `resume` to the trait and client in `crates/acquisition/src/qbittorrent.rs`.**

```rust
pub trait TorrentApi {
    fn add(&self, source: &str, save_path: &str) -> Result<String, QbError>;
    fn resume(&self, hash: &str) -> Result<(), QbError>;
    fn status(&self, hash: &str) -> Result<TorrentStatus, QbError>;
    fn files(&self, hash: &str) -> Result<Vec<String>, QbError>;
    fn remove(&self, hash: &str, delete_files: bool) -> Result<(), QbError>;
}

/// Exposed so a test can assert the paused flag without a live qB server.
pub fn add_form<'a>(source: &'a str, save_path: &'a str) -> Vec<(&'a str, &'a str)> {
    vec![("urls", source), ("savepath", save_path), ("paused", "true")]
}
```

In `impl TorrentApi for QbClient`, change `add` to use `Self::add_form(source, save_path)` and add:

```rust
fn resume(&self, hash: &str) -> Result<(), QbError> {
    let body = form(&[("hashes", hash)]);
    self.api("POST", "/api/v2/torrents/resume", Some(&body))?;
    Ok(())
}
```

Add a client test asserting `add_form(...).contains(&("paused", "true"))`.

- [ ] **Step 9: Guard `Orchestrator::acquire`.**

Add the error variant:

```rust
#[error("torrent contains an executable file or has no readable file list; nothing was downloaded")]
Rejected,
```

Replace the body after `validate_source` / duplicate check with:

```rust
let hash = self.api.add(&source, SAVE_PATH).map_err(|_| AcquireError::Backend)?;

// Metadata for a freshly added magnet may lag; poll briefly, then fail closed.
let mut declared: Vec<String> = Vec::new();
for _ in 0..5 {
    match self.api.files(&hash) {
        Ok(files) if !files.is_empty() => { declared = files; break; }
        _ => std::thread::sleep(std::time::Duration::from_millis(200)),
    }
}
if declared.is_empty() || has_executable_file(&declared) {
    let _ = self.api.remove(&hash, false); // task-only; no file bytes were written
    return Err(AcquireError::Rejected);
}
self.api.resume(&hash).map_err(|_| {
    let _ = self.api.remove(&hash, false);
    AcquireError::Backend
})?;

let status = self.api.status(&hash).map_err(|_| AcquireError::Backend)?;
let item = DownloadItem {
    id: hash.clone(), torrent_hash: hash, media_id: None,
    state: map_state(&status.state), progress: status.progress,
};
self.store.download_insert(&item, &source);
self.inspect_if_ready(&item);
Ok(item)
```

Update every other `impl TorrentApi for …` fake in the crate to provide `resume`.

- [ ] **Step 10: Run the acquisition gate.**

Run: `cargo test -p media-acquisition && cargo fmt --all -- --check && cargo clippy -p media-acquisition --all-targets -- -D warnings`
Expected: PASS, exit 0, no new warnings.

- [ ] **Step 11: Update `roadmap.md` and `progress.md`.**

Add a security task row/status noting the pre-download guard, the ADR, and that a live qB rejection test is **NOT RUN** (no lawful source). State plainly that this is a name-based guard and the post-download gates remain in force.

- [ ] **Step 12: Do not claim a live result.**

Record in `progress.md`: all acquisition tests used fakes; no qB instance, torrent, or executable was used. Live rejection remains NOT RUN pending a lawful source and human review.

---

## Self-review (completed by planner)

- **Spec coverage:** no `.exe` downloaded ⇒ paused add + pre-flight check + reject before resume. Defence in depth ⇒ existing gates untouched. No bundling, no new dependency, path-free errors, regression tests. Covered.
- **Placeholder scan:** no TBD/TODO; every code step contains concrete code. The one adaptation ("GuardApi to the existing fake shape") is a named local test helper, not new behaviour.
- **Type consistency:** `has_executable_file`, `EXECUTABLE_EXTENSIONS`, `add_form`, `TorrentApi::resume`, `AcquireError::Rejected` keep one spelling across all steps; every fake is updated in Step 9.
