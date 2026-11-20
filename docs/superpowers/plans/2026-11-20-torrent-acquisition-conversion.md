# MEDIA_HAVEN → Torrent Acquisition Architecture Conversion Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert the existing local-first MEDIA_HAVEN player into the system described in `media_haven_consolidated_specification.md`: an app that acquires authorized media via qBittorrent in a Docker sandbox, maintains a rolling two-episode buffer, and coordinates cleanup of watched, app-managed media — while preserving the already-built frontend, backend, and container boundary.

**Architecture:** Reuse the current shape (React TV UI → named Tauri commands → hardened Rust `core` + SQLite, mpv on the host) and add three new subsystems: (1) a qBittorrent container with a narrow egress network and a dedicated download volume, (2) an orchestrator crate that drives qBittorrent's loopback WebUI API and tracks download/file/episode state in SQLite, and (3) a prefetch+cleanup scheduler driven by playback completion. The user supplies torrent links/files through the UI; the app bundles no trackers, indexers, catalogues, or source-discovery code.

**Tech Stack:** Rust 1.99 / axum / sqlx(SQLite) / Tauri 2, React 19 + Vite 7, Docker Engine + Compose, qBittorrent (container), mpv via JSON IPC (host). No new crate without an ADR; the qBittorrent WebUI client is a hand-written loopback HTTP client using `std::net::TcpStream` (same pattern as the existing signed forwarder) unless an ADR approves an HTTP library.

**Spec:** `media_haven_consolidated_specification.md` (the target) and `Development.md` §1.4/§1.5/§20 (the current contract this plan must supersede via ADRs).

## Global Constraints

- **Lawful use only.** The target spec states: *"Use is intended for media the operator is authorized to download and distribute."* No trackers, indexers, catalogues, search services, or "find a torrent" features. The app accepts user-supplied magnet links / `.torrent` files only. This preserves the spirit of `AGENTS.md`'s "no content-source bundling" rule even after conversion.
- **Supersede, don't silently break.** This conversion reverses three settled decisions: ADR-0004 (local HDMI playback first / remote deferred), O-05 (downloads deferred out of v1), and AGENTS.md's "automatic permanent deletion is OFF" / "no acquisition". Each reversal needs a human-accepted ADR before code lands (Task 0). An agent may draft and recommend; only the human accepts.
- **Cleanup is scoped to app-managed downloads only.** `Development.md` §20.3 already permits automated retention *"only for app-managed downloads"* — never user-imported folders. Source media under user-selected library roots must never be deleted. This is the line between the target spec's auto-cleanup and the project's non-negotiable "never delete source media."
- **Local-first, offline core.** Browsing and playback of already-acquired media still work with the network off (O-03 accepted). Only acquisition needs internet.
- **Frontend owns presentation; Rust owns policy.** All completion/cleanup/prefetch/quota decisions live in Rust. No security rule in React.
- **Loopback-only, no credential leak.** qBittorrent WebUI is never exposed to the browser or LAN; its credentials live in the host secret store (DPAPI, ADR-0014), injected into the container at start — never in source, images, logs, or renderer DTOs.
- **No new dependency without an ADR.** Prefer std-only implementations; record every new crate in an ADR.
- No commit/push/install/host change without the current instruction granting it. Playwright config/plugin and `vite.config.ts` untouched. Tests are evidence: `PASS`/`FAIL`/`NOT RUN` describe observed runs; checkpoints close on evidence, not compilation.

## Decisions required before implementation (gates — the target spec §15 leaves these open)

The target spec explicitly lists unresolved decisions. The plan below marks tasks that depend on them. **Do not implement a gated task until the human chooses.**

| ID | Open decision (from spec §15) | Recommended default (agent may implement once human accepts) | Blocks |
|---|---|---|---|
| G-1 | Storage option A/B/C/D (spec §7) | **Option B** — dedicated host dir bind-mounted read-write into qBittorrent, read-only into `core`, added to the host map so mpv opens files directly | Tasks 2, 3, 6 |
| G-2 | Source mechanism (spec R-01) | User-supplied magnet/`.torrent` only; no discovery | Task 4 |
| G-3 | Prefetch trigger: immediately on playback or after threshold (spec §9) | After resume/completion policy confirms "Playing" + 10 s watched | Task 5 |
| G-4 | Watched completion definition (spec §8) | Reuse built B5 policy (watched ≥90% + near-end + EOF event); seeking/close never complete | Task 6 |
| G-5 | Cleanup timing / replay grace (spec §10) | Eligible only after confirmed completion AND outside a 24 h replay grace AND not active/prefetched | Task 6 |
| G-6 | Multi-user / concurrent sessions (spec §13) | Out of scope for v1 (profiles are local convenience only, ADR-0012) | none (defer) |
| G-7 | Transcoding / unplayable media (spec §13) | Not supported in v1; surface "unavailable" honestly | none (defer) |
| G-8 | Storage quota (spec §13) | Configurable cap (default 20 GB) with pause-at-90% like P5-01 | Task 6 |
| G-9 | Container network for qBittorrent (needs egress, unlike networkless probe) | Third network `acquire` with egress, no published port to LAN | Task 2 |

## Existing → target mapping (what changes, what is reused)

| Target requirement | Current state | Action |
|---|---|---|
| R-06 Playback, frontend, state store, Docker isolation, controlled frontend integration | Built and verified (React UI, named commands, `core`+`probe`, mpv, SQLite, auth matrix) | **Reuse unchanged** |
| R-02 qBittorrent engine, R-03 sandbox, R-04 dedicated storage | No downloader; `core`/`probe` only; read-only media mounts | **Add** qBittorrent container + download volume + network (Task 2) |
| R-01 source integration | None | **Add** user-supplied magnet/torrent command + UI (Task 4) |
| R-05 controlled media access from downloads | A9 mount map resolves user folders | **Extend** map to include the download root (Task 2/6) |
| R-08/R-09/R-10 rolling buffer + background prefetch | Queue exists for playback; no download queue | **Add** orchestrator prefetch scheduler (Task 5) |
| R-07/R-13 coordinated cleanup | Completion policy exists (B5); no deletion | **Add** cleanup worker scoped to app-managed downloads (Task 6) |
| R-11 duplicate prevention | Queue duplicate suppression exists | **Reuse/extend** to download tasks (Task 4) |
| R-12 resume protection | Resume policy built (B4) + playback_start fall-back fixed | **Reuse unchanged** |
| R-14/R-15 (spec §8) explicit completion | B5 completion policy | **Reuse** (G-4) |

## File structure (new files grouped by responsibility)

```
crates/acquisition/            NEW — orchestrator + qBittorrent client + state
  Cargo.toml
  src/lib.rs                   ports + types (Download, TorrentTask, FileMapping)
  src/qbittorrent.rs           loopback WebUI HTTP client (std TcpStream)
  src/orchestrator.rs          add/check/status/remove flows
  src/prefetch.rs              rolling two-episode buffer
  src/cleanup.rs               eligibility + coordinated deletion
  tests/*.rs                   fake WebUI server; state transitions
crates/persistence/migrations/
  0004_acquisition.sql         torrent_tasks, download_items, file_mappings, prefetch, cleanup
apps/desktop/src-tauri/src/
  downloads.rs                 host commands for acquisition (named only)
apps/desktop/src/pages/Downloads/   make read-only Downloads screen real
  DownloadsPage.tsx (+ test), AddSourceDialog.tsx
docs/decisions/0017-*           conversion ADRs (local-first reversal, qBittorrent boundary, auto-cleanup)
infrastructure/containers/
  Dockerfile.qbittorrent (or pinned image by digest)
  compose.yaml (+ acquire network, qbittorrent service, download volume)
```

---

## Task 0: Governance — supersede the conflicting decisions

**Files:** create `docs/decisions/0017-acquisition-conversion.md`, `0018-qbittorrent-boundary.md`, `0019-app-managed-cleanup.md`; update `docs/decisions/README.md`, `Development.md` §1.5/§18/§20, `roadmap.md` "Deferred / future" (remove downloads+auto-delete deferral), `AGENTS.md` non-negotiables (scope "no acquisition" to "no bundled sources"; scope "no deletion" to "source media").

**Interfaces:**
- Consumes: ADR-0004, O-05, ADR-0005 §3, `AGENTS.md` non-negotiables.
- Produces: human-accepted authority for Tasks 1–6. **No code may land until these are accepted.**

- [ ] **Step 1:** Draft ADR-0017 recording the conversion: the target spec is adopted; ADR-0004 (local-playback-first) and O-05 (downloads deferred) are superseded; local playback remains. Mark **Proposed**, present to human.
- [ ] **Step 2:** Draft ADR-0018 defining the qBittorrent boundary: loopback WebUI only, no LAN exposure, credentials in secret store, egress network `acquire`, untrusted media treated as hostile, no bundled sources.
- [ ] **Step 3:** Draft ADR-0019 defining cleanup: only app-managed downloads eligible; source media under user roots never deleted; replay grace + active/prefetch exclusions; coordinated with qBittorrent task removal.
- [ ] **Step 4:** Update indexes/roadmap/Development.md to reference the three ADRs and flip the deferral entries — **after** human acceptance. Record acceptances by name (do not self-accept).
- [ ] **Step 5:** Verify `git diff --check`; no ADR marked Accepted by an agent.

---

## Task 1: Data model — acquisition records

**Files:**
- Create: `crates/persistence/migrations/0004_acquisition.sql`
- Modify: `crates/app-core/src/domain/media.rs`, `crates/app-core/src/ports/library.rs`, `crates/app-core/src/application/library.rs`, `crates/persistence/src/sqlite/library_repo.rs`
- Test: `crates/persistence/tests/acquisition.rs`

**Interfaces:**
- Consumes: existing `MediaId`, `LibraryRepository`, migration harness.
- Produces: types `DownloadItem`, `TorrentTask`, `FileMapping`, `PrefetchEntry`, `CleanupEntry`; port methods `download_add`, `download_status`, `download_remove`, `prefetch_upsert`, `cleanup_eligible`.

- [ ] **Step 1 — migration SQL** (`0004_acquisition.sql`):

```sql
CREATE TABLE torrent_tasks (
  hash TEXT PRIMARY KEY,
  magnet_or_source TEXT NOT NULL,
  state TEXT NOT NULL,          -- queued|downloading|ready|error|removing
  progress REAL NOT NULL DEFAULT 0,
  save_path TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE download_items (
  id TEXT PRIMARY KEY,
  torrent_hash TEXT NOT NULL REFERENCES torrent_tasks(hash),
  media_id TEXT,                -- resolved once identified; NULL while downloading
  file_path TEXT NOT NULL,      -- container path, /downloads/...
  state TEXT NOT NULL,          -- downloading|ready|playing|watched|cleanup_pending
  created_at TEXT NOT NULL
);
CREATE TABLE file_mappings (
  file_path TEXT PRIMARY KEY,
  torrent_hash TEXT NOT NULL REFERENCES torrent_tasks(hash),
  media_id TEXT,                -- supports multi-episode torrents (spec §10)
  episode_no INTEGER
);
CREATE TABLE prefetch_entries (
  media_id TEXT PRIMARY KEY,
  requested_for TEXT NOT NULL,  -- the currently playing media id
  priority INTEGER NOT NULL,
  state TEXT NOT NULL
);
CREATE TABLE cleanup_entries (
  media_id TEXT PRIMARY KEY,
  eligible_at TEXT NOT NULL,
  state TEXT NOT NULL,          -- pending|confirmed|removed|failed
  outcome TEXT
);
CREATE INDEX idx_file_mappings_media ON file_mappings(media_id);
```

- [ ] **Step 2 — domain types** in `crates/app-core/src/domain/media.rs` (camelCase serde, no path in any renderer-facing DTO — `file_path` stays host-internal):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadItem {
    pub id: String,
    pub torrent_hash: String,
    pub media_id: Option<String>,
    pub state: DownloadState,
    pub progress: f64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DownloadState { Downloading, Ready, Playing, Watched, CleanupPending, Error }
```

- [ ] **Step 3 — failing tests** in `crates/persistence/tests/acquisition.rs`: insert a torrent task + two files mapped to two episodes of one torrent (spec §10 multi-episode case); assert both `file_mappings` rows persist and deleting one does not remove the other's media; assert migration is idempotent. Run `cargo test -p media-persistence --test acquisition` → FAIL (table missing), then run after Step 4 → PASS.
- [ ] **Step 4 — implement repository methods** mirroring `media_replace_for_root` (transactional, bound parameters). Run the full crate tests.
- [ ] **Step 5 — verify** `cargo fmt --all`, `cargo clippy -p media-persistence --all-targets -- -D warnings`, `cargo test -p media-persistence`. **No commit** unless current instruction grants it.

---

## Task 2: qBittorrent sandbox + download volume (gated by G-1, G-9)

**Files:** `infrastructure/containers/compose.yaml`, new `infrastructure/containers/Dockerfile.qbittorrent` (or pinned digest), `apps/desktop/src-tauri/src/mounts.rs`, `docs/security/boundary-evidence.md`.

**Interfaces:**
- Consumes: compose security checklist; `render_mounts` / `strip_verbatim_prefix` from A9.
- Produces: a `qbittorrent` service, an `acquire` network with egress, a `downloads` bind mount, and an extended host map key `/downloads` → host dir.

- [ ] **Step 1:** Add to `compose.yaml`:

```yaml
  qbittorrent:
    image: "${MEDIAHAVEN_QB_IMAGE:?must be <image>@sha256:<digest>}"
    container_name: mediahaven-qbittorrent
    restart: "on-failure:3"
    user: "10003:10003"
    read_only: false                 # qBittorrent must write resume data
    cap_drop: [ALL]
    security_opt: ["no-new-privileges:true"]
    environment:
      QB_WEBUI_TOKEN: "${MEDIAHAVEN_QB_TOKEN:?host-generated}"
    volumes:
      - downloads:/downloads:rw
      - qb-config:/config:rw
    networks: [acquire]
    # NO published port — WebUI reachable only from core on `acquire`.
networks:
  acquire:
    driver: bridge                   # egress allowed; no published ports to LAN
volumes:
  downloads:
  qb-config:
```

- [ ] **Step 2:** Extend `RootSpec`/`render_mounts` in `mounts.rs` with a `download_root: Option<PathBuf>`; render a read-only `/downloads` bind for `core` using `strip_verbatim_prefix`, and add `("/downloads", host_dir)` to `host_path_map()` so mpv can resolve acquired files. Add a test asserting exactly one `/downloads:ro` mount appears and that the canonical `\\?\` form is stripped (reuse `verbatim_prefix_is_stripped_from_the_bind_source`).
- [ ] **Step 3:** Live verification with consent: `docker compose config --quiet` with all required vars → 0; without them → fail-closed (exit 1). `docker inspect` asserts: no published port on qBittorrent, no docker.sock, cap_drop ALL, `acquire` network has a gateway (egress) while `probe` stays on gateway-free `internal`.
- [ ] **Step 4:** Record commands + outputs in `docs/security/boundary-evidence.md`. Do not claim a checkpoint passed.

---

## Task 3: qBittorrent WebUI client (loopback, std-only)

**Files:** create `crates/acquisition/src/qbittorrent.rs`, `crates/acquisition/tests/qb_fake.rs`, `crates/acquisition/Cargo.toml`; add member to root `Cargo.toml`.

**Interfaces:**
- Consumes: `MEDIAHAVEN_QB_TOKEN`, base URL `http://qbittorrent:8080` (container network).
- Produces: `struct QbClient`, `trait TorrentApi { add, status, files, remove }` so tests use a fake.

- [ ] **Step 1 — client skeleton** (no dependency):

```rust
pub trait TorrentApi {
    fn add(&self, source: &str, save_path: &str) -> Result<String, QbError>; // -> hash
    fn status(&self, hash: &str) -> Result<TorrentStatus, QbError>;
    fn files(&self, hash: &str) -> Result<Vec<String>, QbError>;
    fn remove(&self, hash: &str, delete_files: bool) -> Result<(), QbError>;
}
pub struct QbClient { base: String, token: String }
```

Implement with `std::net::TcpStream`: write `POST /api/v2/torrents/add` with `Cookie: SID=<token>` and form body, read the HTTP response, parse the JSON status with `serde_json` (already a workspace dep). Reject any non-2xx into `QbError::Api`.
- [ ] **Step 2 — fake server test**: a loopback `TcpListener` that records the request and returns canned JSON; assert `add` sends the magnet and save path, `remove` passes `deleteFiles=true` only when asked, and a 401 becomes `QbError::Api`. Run `cargo test -p media-acquisition` → PASS.
- [ ] **Step 3 — verify** fmt/clippy/tests 0. Record no live qBittorrent call yet (needs the running container; Task 2).

---

## Task 4: Acquisition flow + duplicate prevention (gated by G-2)

**Files:** `crates/acquisition/src/orchestrator.rs`, named command `apps/desktop/src-tauri/src/commands/renderer.rs::download_add`, frontend `apps/desktop/src/pages/Downloads/AddSourceDialog.tsx`, `DownloadsPage.tsx`.

**Interfaces:**
- Consumes: `TorrentApi` (Task 3), repository methods (Task 1).
- Produces: `Orchestrator::acquire(source) -> DownloadItem`, `download_status()`, `download_remove()`.

- [ ] **Step 1 — orchestrator logic**: `acquire` first calls repository `download_exists(source)`; if found, return the existing `DownloadItem` (spec R-11 duplicate prevention). Otherwise `TorrentApi::add(source, "/downloads")`, persist `torrent_tasks` + `download_items`. Test with an in-memory/fake API: adding the same source twice calls `add` exactly once and returns the same id.
- [ ] **Step 2 — validation at trust boundary**: reject empty/non-magnet/non-`.torrent` sources with a closed error code; cap input length; never accept a path. Test each rejection.
- [ ] **Step 3 — named commands**: add `download_add(source)`, `download_status()`, `download_remove(id)` as named Tauri commands forwarding to host-side orchestrator — **not** renderer-invocable WebUI calls. Add to `crates/contract` COMMANDS only if they must be renderer-visible; re-run the 24-command parity test (which counts POST routes) and confirm it still passes or is updated deliberately.
- [ ] **Step 4 — UI**: `AddSourceDialog` posts a magnet/torrent file path through the named command; `DownloadsPage` replaces its read-only state with real status rows. Vitest against a fake transport: add → row appears; duplicate add → no second row.
- [ ] **Step 5 — verify** fmt/clippy/tests, frontend typecheck/lint/Vitest/build/token/bundle.

---

## Task 5: Rolling two-episode buffer (gated by G-3)

**Files:** `crates/acquisition/src/prefetch.rs`, `crates/app-core/src/domain/playback.rs` (reuse `PlaybackConfig`), `crates/core/src/routes.rs` (call on `playback_start`), tests.

**Interfaces:**
- Consumes: episode ordering from `media_items` (series_id/season/episode), `Orchestrator::acquire`.
- Produces: `PrefetchScheduler::on_playback_start(media_id) -> Vec<DownloadItem>`.

- [ ] **Step 1 — pure ordering logic**: given a media id, compute episode N+1 and N+2 from SQLite ordered by (season, episode), stopping at season/final boundaries (spec §9 open questions → G-3 default). Test: middle-of-season returns exactly two; final episode returns none; season boundary handled.
- [ ] **Step 2 — skip existing**: for each next episode, check `file_mappings`/`download_items` state; reuse ready/downloading items, enqueue only missing (spec §9 steps 3–4). Test: one already ready → only one `acquire` call.
- [ ] **Step 3 — trigger**: call `on_playback_start` from the `playback_start` route **after** the existing policy checks; keep the backend authoritative (ADR-0002). Test that a failed prefetch never fails the playback session (degrade honestly).
- [ ] **Step 4 — storage cap**: before enqueueing, check quota (G-8); if at cap, skip and surface "buffer paused — storage full" in the Downloads state. Test the pause decision.
- [ ] **Step 5 — verify** fmt/clippy/tests 0.

---

## Task 6: Completion → coordinated cleanup (gated by G-4, G-5, G-8)

**Files:** `crates/acquisition/src/cleanup.rs`, `crates/core/src/routes.rs` (`playback_checkpoint`), `crates/persistence` cleanup methods, tests; frontend status.

**Interfaces:**
- Consumes: B5 completion signal (`is_complete`), `CleanupEntry`, `TorrentApi::remove`, G-5 grace.
- Produces: `CleanupWorker::evaluate(media_id) -> CleanupDecision`, `execute(media_id) -> Result<(), CleanupError>`.

- [ ] **Step 1 — eligibility (pure)**: eligible iff completion confirmed AND `now - completed_at >= replay_grace` AND not actively playing AND not in `prefetch_entries`. Test the exclusion matrix (spec §10, AGENTS.md "unfinished/currently playing/unverified never eligible").
- [ ] **Step 2 — never delete source media**: `execute` deletes only files whose `download_items.state` and `file_mappings.torrent_hash` exist — i.e. app-managed downloads. Assert a test that a media id with no `file_mappings` row deletes zero files (file-count proof). This is the release blocker.
- [ ] **Step 3 — multi-episode torrent**: when removing, check all `file_mappings` for the torrent; if any other episode is still needed, call `TorrentApi::remove(hash, delete_files=false)` — remove the task but keep files. Test both branches (spec §10).
- [ ] **Step 4 — reconciliation**: persist `cleanup_entries` state, verify the file is gone, update `download_items`; on failure keep state `failed` with visible outcome — never falsely mark deleted (spec §13). Test a simulated failure leaves `failed`, not `removed`.
- [ ] **Step 5 — trigger from playback**: `playback_checkpoint` calls `CleanupWorker::evaluate` only when completion is confirmed; cleanup executes after grace via a scheduled pass (not inline in the request). Test that seeking/close never triggers cleanup (reuse B5 adversarial cases).
- [ ] **Step 6 — verify** fmt/clippy/tests 0; frontend shows cleanup status honestly. **This task is a release blocker:** any failing exclusion test stops the line.

---

## Task 7: Frontend acceptance + documentation

**Files:** `apps/desktop/src/pages/Downloads/*` (done in Task 4), `docs/testing/acquisition-matrix.md`, `roadmap.md`, `progress.md`, `docs/decisions/README.md`.

- [ ] **Step 1:** Populate `docs/testing/acquisition-matrix.md` with the spec's §14 acceptance criteria mapped to observed test/command output: authorized torrent submitted; downloads confined to configured destination; media appears and plays; user never operates qBittorrent; next two episodes scheduled; no duplicate tasks; interrupted playback retains resume; watched episodes eligible; cleanup preserves needed files; records reflect deletion or recoverable failure; frontend gets no downloader credentials; unavailable episodes shown accurately.
- [ ] **Step 2:** Record CP evidence honestly; do not mark CPs passed (human reviews). Update `roadmap.md` statuses and `progress.md` with observed results.
- [ ] **Step 3:** Re-run the full gate: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, frontend typecheck/lint/Vitest/check:tokens/build/check:bundle, `git diff --check`. Record exact exit codes.

---

## Sequencing

```
Task 0 (human ADR acceptance) ── BLOCKING ──► everything else
Task 1 (data model) ──► Task 3 (client) ──┬──► Task 4 (acquire) ──► Task 5 (prefetch) ──► Task 6 (cleanup)
Task 2 (container/volume, needs G-1/G-9) ─┘
Task 7 (evidence/docs) after 1–6
```

Tasks 2 and 3 are independent after Task 1 and may run in parallel. Tasks 4–6 are sequential (shared interfaces). No task may start before Task 0's ADRs are accepted by the human.

## Self-review — spec coverage

| Spec section | Covered by |
|---|---|
| §1 purpose / lawful use | Global Constraints, Task 0 (ADR-0018) |
| §2 core UX (select → download → play → prefetch → cleanup) | Tasks 4, 5, 6, 7 |
| §3 control vs media flow | Architecture, Task 3 (no renderer WebUI access) |
| §4 components | Mapping table + Tasks 2–6 |
| §5 R-01…R-13 | Mapping table; each R has a task |
| §6 discovery workflow | Task 4 |
| §7 storage option B | G-1, Task 2 |
| §8 lifecycle + completion | Task 1 states, Task 6 (G-4) |
| §9 rolling buffer (all open questions) | G-3, Task 5 |
| §10 cleanup + multi-episode torrents | G-5, Task 6 |
| §11 security boundaries | Task 0 (ADR-0018), Task 2, Global Constraints |
| §12 records | Task 1 |
| §13 operational cases | Tasks 4–6 + `acquisition-matrix.md` |
| §14 acceptance criteria | Task 7 |
| §15 decisions required | Gates table G-1…G-9 |

## Risks

| ID | Risk | Mitigation |
|---|---|---|
| A-1 | Agent implements before ADRs accepted → breaches binding contract | Task 0 is a hard blocking gate; no code step references an unaccepted ADR |
| A-2 | Cleanup deletes user source media | Task 6 Step 2 file-count proof; file_mappings requirement; release blocker |
| A-3 | qBittorrent WebUI exposed to LAN/browser | Task 2 no published port; Task 3 loopback only; credentials in secret store |
| A-4 | Bundled source integration drifts into a tracker/indexer | Global Constraints forbid it; user-supplied links only; no "search" command |
| A-5 | Hand-written HTTP client mishandles qBittorrent auth | Fake-server tests; 401 → error; no credentials in logs |
| A-6 | Prefetch degrades playback | Task 5 Step 3: prefetch failure never fails the playback session |
| A-7 | Storage grows without bound | G-8 quota + Task 5 Step 4 pause-at-cap |
