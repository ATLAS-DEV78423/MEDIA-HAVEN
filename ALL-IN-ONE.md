# MEDIA_HAVEN — All-in-One User & Contributor Guide

> **A free, local-first media appliance for a spare Windows laptop plugged into a TV.**
> Point it at folders you already own and browse them through a full-screen, remote-friendly interface — with no account, no subscription, no cloud, and no deletion of your source files.

This is the GitHub-rendered companion to [`all-in-one-final.txt`](all-in-one-final.txt). Both contain the same information; this version is formatted for reading in a browser. For binding requirements, see [`Development.md`](Development.md); for what was actually observed, see [`progress.md`](progress.md).

---

> [!IMPORTANT]
> **Honest status:** MEDIA_HAVEN is a **functional prototype**. The React UI, Rust backend, Tauri host and local tests run today, but **no roadmap checkpoint (CP-0 … CP-7) has passed**. Real video playback, the running Docker boundary, live provider requests and a clean-VM install are **NOT RUN**. No installer has been published as a GitHub Release. See [Evidence ledger](#5-evidence-ledger) for the full list.

**In this guide**

1. [How everything works](#1-how-everything-works)
2. [File structure, special features and why choose us](#2-file-structure-special-features-and-why-choose-us)
3. [What we require from you](#3-what-we-require-from-you)
4. [Installation and first use](#4-installation-and-first-use)
5. [Reference](#5-reference)

---

## 1. How everything works

### 1.1 The mental model

MEDIA_HAVEN turns a spare Windows laptop into a local media appliance. Everything that **decides, stores, authorises or parses untrusted bytes** runs inside a mandatory Docker boundary. Only the final step — turning already-authorised bytes into pixels and sound — runs on the Windows host.

### 1.2 System shape

```text
Windows 11 laptop / TV via HDMI
+--------------------------------------------------------------------------+
| Windows host                                                             |
|   Tauri 2 shell                                                          |
|    +-- React 19 / TypeScript UI (WebView2), full-screen TV interface     |
|    +-- Named product commands (typed, validated; no Docker access)       |
|    +-- Runtime supervisor (starts/stops only containers it owns)         |
|    +-- Windows playback component (mpv via JSON IPC - host-side)         |
|               |                                                         |
|               |  one host/container API boundary                         |
|               |  loopback only, per-install bearer token + HMAC-SHA256   |
+---------------+----------------------------------------------------------+
                v
+--------------------------------------------------------------------------+
| Required Docker / Compose workspace (internal; invisible to the user)    |
|   core  - Rust backend: product rules, API, SQLite, jobs, storage policy |
|   probe - isolated, networkless media inspection (only untrusted parser) |
+---------------+----------------------------------------------------------+
                v
    user-selected original media folders (READ-ONLY mounts, never modified)
```

Containers are an internal implementation detail: **one window, one navigation model, one library**. The user never sees a container dashboard, a media-server UI or a terminal.

### 1.3 The deciding / pixels split

| Where | What it owns | Why |
|---|---|---|
| Docker `core` | Library, history, resume, queue, watchlist, all product policy, SQLite, signed API | One audit surface; the backend is the sole authority |
| Docker `probe` | ffprobe metadata, thumbnails, trickplay — the **only** component that opens untrusted media | Distinct non-root UID, read-only rootfs, capped tmpfs, no credentials, no database, no gateway |
| Windows host | Native window, input, runtime supervisor, playback engine | GPU, audio endpoint and vsync exist only here ([ADR-0002](docs/decisions/0002-playback-engine.md)) |

If the UI is tricked into displaying something, the backend has already validated it. The frontend never enforces a security rule, computes completion, decides what may be deleted, or receives an actionable filesystem path.

### 1.4 Startup, shutdown and failure

**Startup:** validate config (quarantine + defaults on failure) → open SQLite, back up before migrations → reconcile state → detect the runtime read-only → start the workspace through the supervisor. If the runtime is absent or unhealthy, the UI stays available in recovery mode and backend features remain unavailable — **there is no host-side backend fallback**.

**Shutdown:** persist playback position and queue transactionally → stop services in reverse order with bounded timeouts (`compose stop`, never `down`, so volumes survive) → close database → flush logs → exit with no orphaned children. There is no autostart service.

| Failure | Designed response |
|---|---|
| Corrupt config | Quarantine file, load defaults, warn |
| Failed migration | Roll back, keep backup, enter recovery state |
| Player crash | Surface error, offer retry, never mark complete |
| Runtime unavailable | Recovery UI, backend features off, no bypass |
| Disk full | Refuse write, keep prior state intact, warn |
| Power loss | Recover to last consistent state on next start |

### 1.5 Data flows

**First run / preflight.** A read-only script reports Windows edition/build, architecture, RAM, CPU, GPU, free disk, virtualisation, Docker/WSL2 presence and networking mode, shared-runtime detection, port availability, WebView2 and existing app data. It installs nothing, enables no feature, edits no firewall rule and never asks for administrator rights. Nothing is written until final confirmation.

**Library scan.** You select specific folders. One root = one read-only mount (`/media/root-0`, …). Drive roots, reparse points, missing paths, duplicates and nested roots are refused before any mount is emitted. Paths are containment-checked twice; the read-only mount is the third layer. Originals are never moved, renamed or deleted.

**Browsing.** Typed command → argument validation → application use case → indexed SQLite query → path-free DTOs → lazy artwork loading in the UI.

**Playback, resume and completion.** The backend stays the authority. The host resolves the source from the media id only — no path or token in the renderer ([ADR-0015](docs/decisions/0015-playback-descriptor.md)). A seek-to-end alone does not mean "watched"; stopping early preserves your place.

**Acquisition (shipped code; live run NOT RUN).** For lawfully sourced media only: you attach a magnet or `.torrent` on the Details screen. qBittorrent runs on an egress-only network with no published port. A download is playable only when tied to a `media_id` **and** both the probe and Windows Defender report Clean — a NULL verification is never an allow. While an episode plays, the next two are prepared under a 20 GiB cap. No tracker, indexer or catalogue is bundled.

**Cleanup (fail-safe).** Preview exact paths and counts → explicit confirmation → audit event → stage to managed trash → verify. For downloaded media, deletion is delegated to qBittorrent, not performed by `core` (which has only a read-only mount). A zero-file result is reported as `failed`. There is no "watched → deleted" shortcut.

### 1.6 Security and privacy

- No account, subscription, telemetry or mandatory paid API. No library data leaves the machine without opt-in.
- One loopback-only API with a per-install token and HMAC request signing. No cookies or permissive CORS.
- Docker socket never mounted; no privileged containers; all capabilities dropped; no-new-privileges enforced.
- WSL2 must stay in **NAT** mode — mirrored mode is a blocking preflight failure because it would expose the port to the LAN.
- Secrets live in Windows Credential Manager, never in source, logs or DTOs. Diagnostics are redacted.

**Honest limits** (documented, not hidden): Docker is a process/filesystem boundary, not a security guarantee; a WSL2 kernel escape defeats it; `\\wsl$` makes named volumes readable from Windows, so they are not secret stores; AppArmor is **not available** on the WSL2 kernel (observed) — seccomp, cgroup namespaces and `probe`'s networkless design carry the weight; storage quotas are enforced in application policy, not by the kernel.

**Optional external data, all off by default:** local NFO sidecars; TMDB when opted in; offline IMDb datasets supplied by you (ADR-0022, **Proposed**); OMDb unwired in this build. In the current UI the IMDb/OMDb switches are visibly disabled and labelled "Not available".

### 1.7 Technology stack

| Layer | Choice |
|---|---|
| Frontend | React 19 + TypeScript + Vite 7 + React Router 7 |
| Shell | Tauri 2 on Windows, WebView2 |
| Backend | Rust (Tokio, Axum) compiled into `core` |
| Persistence | SQLite via SQLx in the `core-data` named volume |
| Isolation | Docker Engine / Compose with WSL2 (NAT mode) |
| Inspection | ffprobe in the networkless `probe` container |
| Downloader | qBittorrent sandbox (optional, lawful sources only) |
| Playback | Windows host component, mpv via JSON IPC (engine provisional) |
| Toolchain | Rust 1.99.0 pinned in CI; Node >= 22 |

---

## 2. File structure, special features and why choose us

### 2.1 Repository map

```text
MEDIA-HAVEN/
+-- AGENTS.md                  Contributor contract and permission boundaries
+-- Development.md             BINDING product + engineering specification
+-- roadmap.md                 8 phases, 8 checkpoints, open decisions
+-- progress.md                Observed state, PASS / NOT RUN evidence
+-- CHANGELOG.md / PRODUCT.md / README.md
+-- all-in-one-final.txt       Plain-text compilation
+-- ALL-IN-ONE.md              This GitHub-rendered guide
+-- Cargo.toml / Cargo.lock    Rust workspace (10 members)
|
+-- apps/desktop/              THE PRODUCT SURFACE (one window)
|   +-- src/pages/             Home, Movies, Shows, Anime, Details, Player,
|   |                          Library, Watchlist, Downloads, Search,
|   |                          Settings, Setup, Profiles
|   +-- src/components/        nav, hero, cards, shelf, browse, UI primitives
|   +-- src/hooks/             useSpatialFocus - geometry-based arrow focus
|   +-- src/services/          api.ts (only network boundary, HMAC signing),
|   |                          types (DTOs), adapt (view model)
|   +-- src/state/             AppState provider
|   +-- src/styles/            tokens.css = ONLY colour/spacing literals
|   +-- src-tauri/             Rust host: commands, supervisor, mounts.rs
|   +-- scripts/               check-tokens.mjs, check-bundle.mjs
|
+-- crates/                    THE RULES (all product policy lives here)
|   +-- app-core/              domain (no I/O), use cases, ports
|   +-- persistence/           SQLite queries, migrations
|   +-- media-library/         Safe scanning, roots, containment
|   +-- metadata/              Provider adapters and local cache
|   +-- acquisition/           qBittorrent boundary, buffer, cleanup policy
|   +-- playback/              Playback/completion state
|   +-- contract/              Typed command/DTO contract + route parity
|   +-- core/                  Backend binary deployed in `core`
|   +-- diagnostics/           Health snapshot, redacted bundle export
|
+-- infrastructure/
|   +-- containers/compose.yaml            Required core/probe workspace
|   +-- packaging/windows/build.ps1        Unsigned per-user NSIS build + SHA-256
|   +-- packaging/windows/collect_licenses.py
|
+-- scripts/windows/preflight.ps1          Read-only compatibility checks
+-- docs/            architecture, decisions (ADRs 0001-0024), design, security,
|                   setup, testing, operations
+-- tests/           integration, e2e, security, fixtures
+-- .github/workflows/           ci.yml; release.yml (artifacts only)
```

Dependencies point **inward only**: domain depends on nothing; application depends on ports; adapters depend on the inner layers. A container SDK or player SDK in `domain` is an architectural defect, enforced by `tests/dependency_graph.sh` in CI.

### 2.2 What is in the box today

**Shipped code** (gates recorded in `progress.md`):

- Full TV-style UI: Home, Movies, Shows, Details, Player, Library, Watchlist, Search, Downloads, Settings, Setup, Profiles
- Library scan/indexing of selected folders; search, sorting, filtering
- SQLite persistence, versioned migrations, backup-before-migration
- Durable queue and Up Next, resume checkpoints, completion tracking
- Watchlist: manual order, offline, missing files shown as unavailable
- Profiles; full arrow / Enter / Escape navigation with focus restoration
- Offline IMDb metadata plumbing (ADR-0022 **Proposed**): opt-in via `MEDIAHAVEN_IMDB_DIR`, signed lookup, attribution
- Authorized-media acquisition: qBittorrent sandbox, rolling two-episode buffer, 20 GiB cap
- Fail-closed probe + Windows Defender gates; R-01 per-item source mapping
- Read-only preflight; hardened Compose workspace with digest-pinned images
- Unsigned per-user NSIS installer built locally (v0.1.0, x64, ~3.60 MiB) with checksums

**NOT RUN** — do not treat as verified: real video playback; the running container boundary; live TMDB/IMDb/OMDb/Torznab requests; live acquisition, prefetch and cleanup with lawful media; clean-VM install and uninstall against the built installer; second-machine LAN reachability; SBOM; all checkpoint sign-offs.

### 2.3 What makes MEDIA_HAVEN different

| # | Commitment | Concrete backing |
|---|---|---|
| 1 | **Local-first, not cloud-first** | No account, sync, telemetry or device registration |
| 2 | **No AI at runtime** | After setup, no model, API key or agent is needed; works offline |
| 3 | **Real boundary for untrusted files** | Only `probe` opens media bytes, with no credentials, network or database |
| 4 | **Read-only by default** | Original folders mounted `:ro`; a traversal bug cannot damage your library |
| 5 | **Destructive actions need a human** | Preview → confirm → stage to trash → audit → verify; zero deletions = failure |
| 6 | **Fail-closed download gates** | Tied to `media_id` **and** probe + Defender Clean; NULL = denial |
| 7 | **Content-source neutral** | No bundled trackers, indexers, catalogues or acquisition guidance |
| 8 | **Infrastructure you never see** | Headless supervisor, no second dashboard, `stop` not `down` |
| 9 | **Independent of proprietary runtime licences** | Docker Engine in a dedicated WSL2 distro proposed over Docker Desktop |
| 10 | **TV distance is a requirement** | No essential text under 16 px; deterministic focus; reduced-motion support |
| 11 | **Honest reporting as culture** | NOT RUN ≠ PASS; unexecuted plans stay plans; Proposed ADRs stay unaccepted |
| 12 | **Small, explainable workspace** | Two containers; SQLite is a file; every extra service needs an ADR |

### 2.4 Why choose MEDIA_HAVEN

- **Rather than a streaming service:** your own appliance for media you already own, with no monthly fee.
- **Rather than a general media server:** one excellent local, couch-usable experience on the laptop plugged into your TV. LAN streaming, phone apps and multi-user server mode are deliberately deferred.
- **Rather than DIY scripts and containers:** the hard parts are already decided — safe roots, read-only mounts, per-install credentials, HMAC signing, fail-closed verification, bounded cleanup, redacted diagnostics.
- **If you care about privacy:** no telemetry path by default; optional providers disclose exactly what leaves the machine (title/year for search, an IMDb id for ratings).
- **If you care about your files:** removing a library root deletes zero original files; cleanup never touches source media, unfinished items, playing items or unverified downloads.
- **If you care about maintainability:** one codebase, inward-only dependencies, a frozen typed command contract, drift guards in CI, and ADRs recording why each boundary exists.

**The honest caveat:** this is a prototype. Choose it if you want to follow or contribute to a carefully engineered local-first appliance; do not choose it today if you need a verified, signed, one-click consumer product — those gates are still open.

### 2.5 Design and accessibility

Near-black ground, centred floating glass navigation, full-bleed cinematic hero, quiet surfaces so artwork carries the colour. Focus ~140 ms, overlays ~180 ms, hero ~240 ms; `prefers-reduced-motion` zeroes non-essential motion; nothing autoplays on focus. Exactly one focused element; shelf edge clamping; column preservation; Escape unwinds one overlay and restores focus. WCAG AA contrast over actual artwork; focus never colour-only; complete keyboard-only path. Tokens only — no literal colour or spacing outside `tokens.css` (enforced in CI).

---

## 3. What we require from you

### 3.1 Hard requirements

- **Windows 11 x64.** Windows 10 x64 and ARM64 are not claimed until tested (roadmap O-02).
- **A Docker Engine-compatible runtime** for backend features. It is detected read-only and never silently installed; there is no host fallback.
- **WSL2 in NAT networking mode.** A mirrored `.wslconfig` is a blocking preflight condition.
- **WebView2** (present on Windows 11; preflight gives the official installer link if missing).
- **Enough RAM** for Windows + WSL2 + `core` + `probe` + playback. 8 GB is the entry candidate; 4 GB is explicitly not a support target.
- **Free disk space** on the media and managed-storage drives.
- **Media you own or are authorised to use.** For downloads you supply your own lawful source; nothing is bundled.

### 3.2 Hardware tiers

| Tier | RAM | Status (all UNTESTED as support claims) |
|---|---|---|
| Minimum experiment | 4 GB | Not a support target |
| Entry | 8 GB | Candidate: library + playback; optional services gated |
| Recommended | 16 GB | Candidate: full MVP experience |
| Higher | 32 GB+ | Candidate for heavier workloads; no headless/streaming claim |

Reference dev machine observed: Windows 11 Pro build 26300, 16 GB RAM, i7-13700H, Iris Xe, Docker Engine 29.9.3 on WSL2 with an 8 GB VM cap and no AppArmor. Baseline measurements, not a published minimum.

### 3.3 What you provide

One or more **specific** media folders (no whole-profile/Desktop/Documents scanning); a managed-storage location; explicit consent before any host change (runtime, WSL2/virtualisation, admin elevation, firewall, shared-engine reuse); human decisions on Proposed ADRs before relying on their features; and, for source builds, Node >= 22, Rust 1.99.0 and approved network access for `npm ci`.

### 3.4 What you approve

The final setup review (what is created, where, which runtime changes); the first cleanup and any deletion-policy change; removal of app data on uninstall (a separate confirmation); and any optional provider — nothing is default-on.

### 3.5 What we explicitly do NOT require

No account, email, phone, subscription, credit card, paid API, paid VPN, cloud storage, AI service or AI key, telemetry consent, administrator rights for normal use, Docker Desktop GUI, terminal use, or Docker commands.

### 3.6 Your responsibilities

Use only lawful media; back up your own source files; supply your own Torznab endpoint and key if the Proposed discovery feature is accepted; keep Windows/WSL2/runtime updated; and verify installer checksums from a different channel. The v1 build is **unsigned** — Windows SmartScreen will warn, and that warning must not be hidden.

---

## 4. Installation and first use

### 4.1 Path A — end-user installer (intended; not yet published)

> [!WARNING]
> A real unsigned NSIS installer was **built and checksummed locally** on 2026-11-21 (v0.1.0, x64, ~3.60 MiB, `com.mediahaven.desktop`). It is gitignored, **not published** as a GitHub Release, and **not tested on a clean VM**. These are the intended steps, not a verified consumer experience.

1. Obtain the installer and `SHA256SUMS.txt` from the official release source. Do not accept a checksum relayed through the same channel as the binary.
2. Verify it:

   ```powershell
   Get-FileHash -Algorithm SHA256 .\MEDIA_HAVEN_0.1.0_x64-setup.exe
   ```

   Compare the lowercase hash with the line for that filename. `certutil -hashfile <file> SHA256` also works.
3. Run the installer. It is per-user (`currentUser`): no administrator rights, no system software installed.
4. Expect a SmartScreen "Windows protected your PC" warning — expected for unsigned v1. Verify the checksum, then **More info → Run anyway**. Never disable Defender or SmartScreen to ease installation.
5. Complete the guided first-run setup below.
6. Back up database and config before any update; updates preserve settings and queue and never overwrite media.

### 4.2 Path B — run from source today (verified contributor path)

From the repository root, with Node >= 22 and Rust 1.99.0 installed:

```powershell
# 1. Read-only environment check (changes nothing)
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/windows/preflight.ps1
# Add -Json -MediaPath 'D:\Media' for machine-readable output and drive free space

# 2. Frontend dependencies (once)
cd apps/desktop
npm ci
cd ../..
```

```bash
# 3. Backend with a seeded demo library (terminal 1)
MEDIAHAVEN_DEV=1 MEDIAHAVEN_SEED=1 \
MEDIAHAVEN_DATA_DIR="$PWD/.mediahaven-dev" \
cargo run -p mediahaven-core

# Interface (terminal 2) -> http://localhost:5173/
cd apps/desktop && npm run dev
```

```bash
# 4. Or the real Tauri window
cd apps/desktop && npm run tauri:dev
```

> **Note:** the current host exposes only `get_app_info`; the renderer still calls the API over HTTP. Do not treat this window as the final transport. Real playback here is NOT RUN.

```powershell
# Build an unsigned installer locally (tools must exist; installs/publishes nothing)
powershell -NoProfile -ExecutionPolicy Bypass -File infrastructure\packaging\windows\build.ps1
# Exit codes: 0 success, 2 config precondition, 3 missing tool, 4 build failure
```

| Variable | Default | Purpose |
|---|---|---|
| `MEDIAHAVEN_API_TOKEN` | dev token when `MEDIAHAVEN_DEV=1` | Required >= 32 chars otherwise |
| `MEDIAHAVEN_BIND` | `127.0.0.1:8080` | Listener address |
| `MEDIAHAVEN_DATA_DIR` | platform data dir | SQLite location in dev |
| `MEDIAHAVEN_SEED` | off | Seed demo library once |
| `VITE_MEDIAHAVEN_API` | `http://127.0.0.1:8080` | Backend origin |
| `VITE_MEDIAHAVEN_TOKEN` | dev token in `npm run dev` | Install secret |

### 4.3 The nine-step first-run experience

1. **Welcome** — what the app does and what it never does: no uploads, no deletion, no account.
2. **Compatibility summary** — preflight results as ready / recommended / needs attention.
3. **Media folders** — pick folders; found-file counts are shown; nothing is read before you confirm.
4. **Managed storage** — choose where app data, staging and trash live, with free-space validation.
5. **Required backend runtime** — plain-language explanation, every prerequisite shown, explicit consent required. Declining leaves backend features unavailable; there is no fallback.
6. **Optional services** — only separately approved providers appear.
7. **Review** — one summary of exactly what will be created, where, and which runtime changes occur.
8. **Confirm** — explicit action. Nothing is written before this point.
9. **Done** — the library opens only when the backend is ready; otherwise recovery guidance appears.

Setup never installs software, enables WSL2/Hyper-V/virtualisation, requests admin rights, changes firewall/proxy/DNS, scans broad personal folders, or writes config before confirmation.

### 4.4 Adding media and starting to watch

1. Settings → Library folders (or Setup) → add a specific folder.
2. Wait for the scan. Only selected folders are read; originals are unchanged; missing files are reported.
3. Home shows Continue Watching, Recently Added, Up Next and a deterministic local recommendation shelf.
4. Browse Movies / TV Shows or use Search (local library first).
5. Details shows metadata, season chips, episode progress and — for media you supply — the Source field.
6. Press Play. Arrows, Enter, Escape, mouse or mapped remote/controller all work.
7. Progress and resume position are saved; stopping early keeps your place.
8. Add to Watchlist from any card or Details view: manual order, offline, no deletion.

> **Honest note:** the current player shows artwork with a simulated 4× clock and labels itself "Playback preview". Library scanning, persistence, search, watchlist, profiles, sessions and checkpoints are real; real video playback is NOT RUN.

### 4.5 Verification commands

```bash
# Rust
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
bash tests/dependency_graph.sh
```

```bash
# Frontend (from apps/desktop)
npm run typecheck && npm run lint && npm test
npm run check:tokens && npm run check:bundle && npm run build
```

Observed in the final review wave: Rust gates PASS; 122 Rust tests passed, 0 failed, 1 ignored; 18 Vitest tests; 66 files token-checked with 0 violations; frontend build 93 modules. These are **local** results — CI and clean-VM runs are NOT RUN.

### 4.6 Troubleshooting

| Symptom | What to do |
|---|---|
| Backend features unavailable | Runtime absent/unhealthy — run preflight; no host fallback exists |
| Mirrored WSL2 networking blocked | Return to NAT mode; do not work around the check |
| Shared Docker engine detected | Review and consent explicitly; a dedicated engine is preferred |
| Port in use | Resolve the loopback conflict; never publish to LAN or add a portproxy rule |
| Config corrupt | Defaults loaded; quarantined file kept for inspection |
| Migration failed | Rolled back, backup retained, recovery state — nothing discarded without confirmation |
| Player crashes | Retry offered; item not marked complete; resume preserved |
| Download not playable | Both probe and Defender must be Clean and item tied to `media_id` |
| Setup interrupted | Next launch offers resume or restart; no half-applied config |

### 4.7 Uninstall

- **Remove application only** (default): program files and shortcuts removed; database, config, artwork, logs and managed media kept.
- **Remove application and managed data**: exact paths shown; separate explicit confirmation required.

Never removes source media or a library-root folder. Never uninstalls WSL2, Docker Desktop or a shared runtime. A MEDIA_HAVEN-owned distro/volume is a separate confirmed action after backup. **Uninstall against the built installer is NOT RUN.**

### 4.8 Quick command reference

| Task | Command |
|---|---|
| Preflight | `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/windows/preflight.ps1` |
| Dev backend | `MEDIAHAVEN_DEV=1 MEDIAHAVEN_SEED=1 MEDIAHAVEN_DATA_DIR="$PWD/.mediahaven-dev" cargo run -p mediahaven-core` |
| Dev UI | `cd apps/desktop && npm run dev` |
| Tauri window | `cd apps/desktop && npm run tauri:dev` |
| Installer build | `powershell -NoProfile -ExecutionPolicy Bypass -File infrastructure\packaging\windows\build.ps1` |

`docker compose up` is **not** a supported entry point: the Windows supervisor owns lifecycle, token provisioning and per-root mount rendering.

---

## 5. Reference

### 5.1 Document map

| Purpose | File |
|---|---|
| Start here | `ALL-IN-ONE.md` / `all-in-one-final.txt` |
| Binding contract | `Development.md` |
| Sequencing and checkpoints | `roadmap.md` |
| Observed evidence | `progress.md` |
| Change summary | `CHANGELOG.md` |
| Contributor rules | `AGENTS.md`, `CONTRIBUTING.md` |
| Product definition | `PRODUCT.md` |
| Architecture | `docs/architecture/` |
| Decisions | `docs/decisions/` |
| Design system | `docs/design/` |
| Security | `docs/security/` |
| Setup | `docs/setup/first-run.md` |
| Testing | `docs/testing/` |
| Operations | `docs/operations/` |

### 5.2 Architecture decision records

| ADR | Decision | Status |
|---|---|---|
| 0001 | Docker mandatory in v1; runtime distribution | Accepted / Accepted (provisional) |
| 0002 | Playback engine and host placement | Accepted (provisional); engine open |
| 0003–0004 | Modular monolith; local playback first | Accepted |
| 0005–0010 | Transport, filesystem, secrets, deployment, contract, entities, providers, profiles, sequencing, token storage, descriptor | Accepted |
| 0011–0019 | Metadata, profiles, sequencing, DPAPI, descriptor, blanket acceptance, acquisition, qB sandbox, cleanup | Accepted |
| 0021 | R-01 per-item operator-supplied source | Accepted (reversible) |
| 0022 | Offline IMDb metadata | **Proposed** |
| 0023 | TMDB artwork transport | **Proposed** |
| 0024 | Host-side search with operator confirmation | **Proposed** |

No ADR-0020 exists — the number was skipped. **Only the human owner may accept a Proposed ADR.**

### 5.3 Evidence ledger

**PASS (observed; see `progress.md`):** Rust fmt/clippy/tests (122 passed); frontend typecheck/lint/tests/token/bundle/build; contract route parity (24 commands); dependency-graph assertions; local unsigned NSIS build + checksums; Compose config parse; read-only preflight behaviour; keyboard-navigation browser walkthrough with zero console errors; static container security review.

**NOT RUN:** real playback, codec/GPU/audio/subtitle matrix; container startup and live host/container boundary; live TMDB/IMDb/OMDb/Torznab requests; live acquisition, prefetch and cleanup; clean-VM install, uninstall and upgrade/rollback; second-machine LAN reachability; low-disk and power-interruption tests; SBOM; signed build; reproducibility; every-screen automated keyboard walkthrough; all CP-0…CP-7 sign-offs.

### 5.4 Glossary

| Term | Meaning |
|---|---|
| `core` | Required Rust backend container; sole product authority |
| `probe` | Required networkless container that opens untrusted media |
| Named command | Typed, validated operation exposed by the Tauri host |
| DTO | Path-free data sent to the renderer |
| Root | A user-selected media folder mounted read-only |
| Managed media | App-owned files under `incoming/ready/quarantine/trash` |
| `source_set` | Existing command chain attaching a source to an item |
| Fail-closed | Missing or unverified evidence is treated as denial |
| HMAC | Per-request signature over timestamp, method, path and body |
| ADR | Architecture Decision Record; Proposed is not binding |
| CP | Checkpoint; all eight remain OPEN |
| NAT mode | WSL2 default networking; keeps the API off the LAN |

### 5.5 Legal and third-party notices

MIT licence ([LICENSE](LICENSE)). Third-party notices in `infrastructure/packaging/windows/THIRD_PARTY_NOTICES.md` cover 536 Rust crates and 343 npm packages; 177 Rust licences remain "unresolved offline" pending an approved fetch. IMDb attribution is rendered where IMDb data appears; TMDB attribution is not yet implemented. SBOM is NOT RUN. The v1 installer is unsigned — see `docs/operations/release.md`. No third-party design assets ship; reference screenshots and the UI brief are comparison inputs only. Report security issues privately per [SECURITY.md](SECURITY.md).

---

**Questions or contributions?** See [CONTRIBUTING.md](CONTRIBUTING.md) and [SECURITY.md](SECURITY.md).
