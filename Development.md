# Development.md

# Personal Media Appliance --- Complete Engineering & Agent Specification

> **Document status:** Consolidated implementation specification — authoritative baseline\
> **Audience:** AI coding agents, maintainers, testers, and
> contributors\
> **Primary platform:** Windows 11 x64\
> **Support claim (O-02, decided 2026-11-18):** Windows 11 x64 only for v1.
> Windows 10 is not a supported target.\
> **Product principle:** A free, local-first media appliance with one unified TV-first application, a Docker-managed backend and media-inspection boundary, and no AI runtime dependency.

------------------------------------------------------------------------

## 0. How to use this document

This is the project's single top-level engineering contract. It
combines:

-   Product requirements and non-goals
-   Architecture and system design
-   Technology choices and rationale
-   Repository/file structure
-   Implementation boundaries
-   Security, privacy, reliability and data-loss precautions
-   Testing and acceptance criteria
-   `roadmap.md` creation and maintenance rules
-   `progress.md` format and handover rules
-   Instructions for AI coding agents

If a requirement is undecided, it is marked **OPEN DECISION**. Do not
silently convert an open decision into a permanent product requirement.
Record a proposal, explain trade-offs, and request approval before
making an expensive or irreversible commitment.

### Source-of-truth hierarchy

When instructions conflict, use this order:

1.  Explicit user instruction in the current task
2.  This `Development.md`
3.  `roadmap.md` for approved sequencing and scope
4.  `progress.md` for current implementation state and handover
5.  Existing code and tests, provided they do not contradict a
    higher-level requirement

Do not treat comments, generated plans, old agent notes, or assumptions
as approval to change product requirements.

------------------------------------------------------------------------

# 1. Product definition

## 1.1 Purpose

Turn a spare or older Windows laptop into a simple, living-room-friendly
media appliance. The laptop may be connected directly to a television
through HDMI. The user should be able to open the application and browse
and play media through a polished TV interface.

The product should be free to install and use, open source, local-first,
and understandable to a non-technical Windows user.

## 1.2 Core user experience

The target experience is similar in interaction quality to a modern TV
streaming interface:

-   Full-screen, dark, cinematic interface
-   Large featured artwork and clear media details
-   Horizontal content shelves and media cards
-   Continue Watching and Up Next
-   Watchlist
-   Search, library, downloads and settings
-   Full-screen playback
-   Keyboard, mouse, controller and remote-friendly navigation
-   Large focus targets and visible focus indicators
-   Smooth but restrained transitions
-   Clear download and storage status
-   Setup and diagnostics that do not expose Docker, WSL or command-line
    concepts to ordinary users

The visual direction is inspired by Apple TV-style interfaces. Do not
copy proprietary assets, branding, icons or exact layouts. Use original
artwork or artwork the user is authorised to use.

## 1.3 Product principles

1.  **AI is a setup assistant, not a runtime dependency.** The AI coding
    agent may install, configure and verify the application. Once setup
    is complete, the product must work without an AI model, AI API,
    OpenRouter account, cloud agent or recurring AI usage.
2.  **Local-first.** User settings, library metadata, playback state and
    queue state are stored locally by default.
3.  **No mandatory paid services.** No required subscriptions, paid
    APIs, paid cloud storage, paid VPN, account creation or credit card.
4.  **Windows-first.** Do not require users to replace Windows with
    Linux or install a server-oriented Linux distribution.
5.  **Simple for ordinary users.** Technical infrastructure is hidden
    behind a clear setup experience.
6.  **Safe by default.** Least privilege, narrow filesystem access,
    explicit destructive-action safeguards and secure network defaults.
7.  **Recoverable.** A crash, reboot or interrupted download must not
    corrupt the library or silently lose state.
8.  **Content-source neutral.** The software manages media the user
    owns, has permission to access, or can lawfully obtain. It must not
    ship piracy links, copyrighted-content indexes or instructions to
    evade copyright restrictions.
9.  **Transparent.** Explain what is stored, where it is stored, which
    services run, and what cleanup will delete.
10. **Resource-conscious.** Idle CPU/RAM usage, startup time and disk
    usage are first-class requirements, not afterthoughts.

## 1.4 Scope for the first release (MVP)

The MVP should focus on a dependable local appliance, not a huge feature
list.

**Must have:**

-   Windows preflight checks and guided installation
-   A local application that starts and stops predictably when its required Docker-compatible runtime is available; missing/unhealthy runtime is a clear setup/recovery state, not a reason to bypass the container boundary
-   TV-style home screen and library browsing
-   Local media discovery/import from user-selected folders
-   Metadata and artwork handling with a clear offline/fallback state
-   Media playback, resume position and playback completion tracking
-   Queue and Up Next management
-   A local watchlist
-   Persistent settings and database
-   Storage usage visibility and configurable limits
-   Logs, health checks, diagnostics and safe uninstall
-   Docker-managed backend and isolated media inspection; any download integration remains optional and requires separate lawful-use, security and provider approval
-   Keyboard/mouse operation and remote-friendly focus navigation

**Not required for the first release:**

-   Cloud accounts or synchronisation
-   Multi-user remote access over the public internet
-   Mobile companion application
-   Transcoding farm
-   Kubernetes
-   AI recommendations or AI media agents
-   Paid metadata services
-   Automatic access to arbitrary network shares
-   Automatic deletion of source media
-   Any bundled list of torrent trackers, indexers or content websites

## 1.5 Important product boundary: local playback versus server streaming

The first release prioritises playback on the same Windows laptop connected to the TV. **Playback runs on the Windows host**, because the display, GPU, audio endpoint and vsync that the product's core promise depends on exist only on the Windows side of the WSL2 boundary. This is an intentional, documented architectural exception, recorded in ADR-0002, not an oversight: everything that *decides*, *stores*, *authorises* or *parses untrusted media bytes* runs in Docker; the component that turns already authorised bytes into pixels and sound runs on the host.

The backend remains the authority on what to play — library, resume position, track preferences and policy all stay in `core`. The host owns only the presentation.

A future headless/server mode is out of scope until the local appliance is reliable and a new requirement is approved. Do not design the MVP around LAN or remote streaming. Remote access materially expands authentication, network exposure, TLS, patching and threat-model requirements.

------------------------------------------------------------------------

# 2. Key architecture decisions

## 2.1 Proposed high-level architecture

```text
Windows 11 laptop / TV via HDMI
┌───────────────────────────────────────────────────────────────┐
│ Windows host                                                  │
│  Tauri shell + React UI                                       │
│   ├─ named product commands; no Docker daemon access          │
│   ├─ narrowly scoped runtime supervisor                       │
│   └─ Windows playback component  ← intentional exception      │
│          │ host-to-container API (token + HMAC, ADR-0005)      │
└──────────┼────────────────────────────────────────────────────┘
           │ Docker Engine; distribution PROPOSED (ADR-0001 Part 2)
           ▼
┌───────────────────────────────────────────────────────────────┐
│ Compose workspace (required; not yet validated on a real host)                │
│  core: Rust backend/API, policy, SQLite persistence            │
│  probe: isolated untrusted-media inspection worker              │
│  Optional services only after separate approval                 │
└───────────────────────────────────────────────────────────────┘

User-selected source roots: read-only, narrow mounts only
Managed state: named volume inside the WSL2 distro (ADR-0006)
```

This is a proposed trust-boundary diagram, not a deployment guarantee. Docker/Compose is a mandatory v1 backend boundary. Runtime distribution, host-to-container reachability, authentication, egress, exact mounts, persistent storage, resource limits and service health behavior require recorded P0 evidence before implementation. Playback placement is decided in principle by ADR-0002 and still needs codec/GPU/audio/subtitle/licence evidence to be accepted.

## 2.2 Docker-hosted Rust backend and Windows host responsibilities

Rust is used on both sides of a deliberate process boundary:

- **Container `core` (required):** product rules, configuration, library management, playback state, queue, storage policy, API and SQLite persistence. It is the backend authority and must not be shipped as a bare host-process fallback.
- **Container `probe` (required):** narrowly scoped media inspection of untrusted files. Its parsing scope, communication path, mounts and network isolation must be verified; Docker alone does not make hostile media safe.
- **Windows Tauri host:** window/lifecycle, named product commands, a narrowly scoped runtime supervisor, and the Windows playback component. It owns only the host operations required by the UI/runtime/player boundary.
- **Docker Engine / Compose:** required v1 backend and inspection boundary. The Windows runtime distribution is *proposed*, not accepted (ADR-0001 Part 2).
- **SQLite:** persistent state for `core`, stored in a named volume inside the WSL2 distro, outside a disposable container layer (ADR-0006).
- **React/TypeScript:** presentation only; it must not receive Docker daemon access.

Do not put product policy in Compose or shell scripts. The container backend owns product decisions; the Windows host owns its platform-specific boundary. The API contract and transport remain subject to ADR-0005 and verification.

The modular monolith is one codebase deployed in two units — host-side crates in the Tauri process, backend crates in `core` — with the host/container boundary as the only interface between them (ADR-0008). This is not a microservice architecture and must not grow into one.

## 2.3 Docker is the mandatory backend and media-inspection boundary for v1

The v1 Rust backend and untrusted-media inspection workload run in a Docker Engine-compatible Compose workspace. This is a product requirement and a security boundary, not an optional integration or a reason to split the product into multiple user-facing applications.

- The product is one application with one frontend, navigation model, library, search, player and settings experience.
- Compose is internal infrastructure. Containers are not separate screens or products from the user's perspective.
- `core` is the backend authority for product rules, API, persistence and policy. `probe` is an isolated service for parsing untrusted media during inspection. Responsibilities and images remain subject to ADRs and validation.
- The Windows host supervisor, not the frontend, owns the narrowly scoped runtime lifecycle. The frontend receives named product operations and no Docker daemon access.
- Docker is not a substitute for application logic, database design or safety policy.

**The supported Windows runtime distribution is proposed, not accepted.** ADR-0001 Part 2 recommends **Docker Engine (rootful) inside a dedicated WSL2 Linux distribution**, managed from Windows via `wsl.exe`, over Docker Desktop. The reasons are a licence cliff (Docker Desktop requires a paid subscription for organisations above 250 employees or $10M annual revenue, and for government entities regardless of size), one fewer privileged host-integration layer, and a genuinely separate daemon and VM budget so unrelated workloads on the machine cannot stop or starve ours. The honest costs — we own engine and distro maintenance, and users who already run Docker Desktop end up with two engines — are recorded in the ADR, not hidden.

Do not silently install a runtime, enable WSL2 or virtualisation, alter Windows features or request administrator rights. Explain prerequisites and obtain consent before host changes. Do not assume an unavailable runtime can be bypassed: the application must show a clear, recoverable runtime error and must not run the backend outside the approved boundary.

The Compose design must use pinned image digests, meaningful health checks, bounded restart behaviour, non-root users, dropped capabilities, read-only root filesystems where practical, resource limits, explicit networks and narrowly scoped persistent volumes. Never mount the Docker control socket into a workload container. Never use privileged containers as a shortcut. Do not expose management APIs or service dashboards to the LAN or internet. Publish only the minimum API endpoint to loopback if host/container reachability tests prove the configuration.

**WSL2 must run in its default NAT networking mode.** WSL2's mirrored mode is designed to make the instance reachable from the LAN and would convert a loopback-only API publication into a LAN-reachable one. A mirrored `.wslconfig` is a preflight failure, not something to work around.

Network reachability, egress policy and WSL2 behavior remain unverified until P0 evidence is recorded. Re-check upstream runtime and image security before each release.

### 2.3.1 What the Docker boundary does and does not buy

Docker gives MEDIA_HAVEN a *process and filesystem* boundary. It is not a security guarantee, and the following are true on Windows and must never be claimed otherwise:

- Containers share one WSL2 guest kernel; a kernel exploit defeats the container boundary (R-08).
- The WSL2 filesystem is readable **and writable** from the Windows host via `\\wsl$`, so a named volume is not a secret store and the database is not confidential against the same Windows user (R-15).
- AppArmor is **not available** under the WSL2 kernel — observed, not assumed. Seccomp and cgroup namespaces remain (R-17).
- The Docker daemon is root inside the VM and is trusted (R-11).
- There is no kernel-enforced storage quota on volumes; quotas are enforced in application policy instead (R-09).

The boundary still earns its place: original media is mounted read-only into every container, `probe` has no credentials and no outbound network, no container can reach the database except `core`, and the frontend cannot reach the Docker daemon.

## 2.4 Keep the mandatory Compose workspace small

Docker/Compose is required for the v1 backend and inspection boundary, but that does not put every component in a container. A Windows virtualization/runtime layer has memory, disk, startup and maintenance costs; measure them before making hardware-support claims. Keep additional services out unless a measured requirement and approval justify them.

| Component | Proposed v1 location | Status / rationale |
|---|---|---|
| UI and Tauri shell | Windows host | Native window, input and lifecycle |
| Runtime supervisor | Windows host | Proposed narrow lifecycle owner; no frontend Docker access; exact implementation awaits ADR-0005/runtime evidence |
| Rust product backend/API (`core`) | Required Docker container | Mandatory v1 boundary; Compose/runtime behavior not yet verified |
| SQLite state | Persistent storage for `core` | Storage/backup design in ADR-0006 remains proposed |
| Media inspection (`probe`) | Restricted Docker container | Proposed parser boundary; isolation, access and IPC need proof |
| Playback engine | Windows host | Intentional exception (ADR-0002): the GPU, audio endpoint and vsync exist only on the host. Validate codec, subtitle and licence evidence before ADR-0002 acceptance |
| Metadata/artwork | Local cache; optional provider | Offline operation remains useful; external access is opt-in and needs egress review |
| Download service | Optional separate container only if approved | Not required for local playback; legal/security approval required |
| Media server | Deferred | No confirmed headless or multi-device requirement |

Do not run Jellyfin and a second independent playback stack without a clear reason. The user-facing experience remains one MEDIA_HAVEN application.

## 2.4.1 One product surface, internal services

The user must never have to launch a media-server UI, container console or separate management dashboard. The Tauri UI owns browsing, details, search, queue and playback presentation. The host supervisor exposes named product operations and no generic Docker control to the UI.

The required baseline Compose workspace contains only:

- `core`: backend/API, product policy and SQLite access.
- `probe`: restricted media-inspection worker for untrusted-file parsing.

These responsibilities, image identities, health checks and communication paths are
defined in `docs/architecture/docker-workspace.md` and implemented in
`infrastructure/containers/compose.yaml`, but are unvalidated against a running engine
until P0-10/P0-13/P0-14 and P6-03. Optional provider/helper services require separate
justification, approval and review.

Persistent state must not live only in the disposable container layer. ADR-0006 proposes named persistent storage for backend state and exact read-only mounts for user-selected source roots; physical layout, backup/export and runtime access still require validation. Only an approved download service may receive read/write access to its specific managed directory.

## 2.4.2 Proposed managed-media and persistent-state model

Where an approved managed-media feature exists, its logical directories are:

```text
incoming/       incomplete items; never playable or auto-deleted as watched
ready/          verified, app-managed media
quarantine/     suspicious, malformed or failed-verification items
trash/          reversible deletion staging, subject to retention
```

The database, configuration, cache, staging and secrets have separate access classes. Exact physical locations, volume ownership, backup/restore and uninstall behavior are specified as proposals in ADR-0006/0007 and must be validated before implementation. Container replacement must not delete persistent state. A volume is not a backup.

No service may receive the full home directory, system drive or a broad profile mount. User-selected original media remains read-only by default. Exact Windows-to-runtime path translation, reparse-point defenses and the host-side playback path must be verified. Never assume a container boundary makes hostile media harmless.

## 2.5 Process model

Keep the number of processes low while preserving the mandatory v1 boundary.

Required for application operation:

1. Windows Tauri shell and React UI.
2. Windows host supervisor and the Docker Engine-compatible runtime distribution selected after P0 validation.
3. `core` backend/API container and `probe` media-inspection container (required workspace; no host-side backend fallback).
4. Windows playback component when media is playing (host placement is a deliberate, documented exception per ADR-0002; codec/GPU/audio/subtitle/licence evidence still pending).

Optional only after separate approval: a download-provider container or helper justified by an ADR. Start only the runtime/workspace processes the app owns for the current session and stop them predictably at exit; do not install or configure host prerequisites silently. Lifecycle, shutdown, restart and resource cost require P0/P6 verification.

------------------------------------------------------------------------

# 3. Technology stack

The following is the proposed default stack. Validate compatibility and
pin supported versions before the first implementation milestone. Do not
upgrade major versions casually.

  -----------------------------------------------------------------------
  Layer                   Proposed technology     Responsibility
  ----------------------- ----------------------- -----------------------
  Language                Rust                    Backend, policies,
                                                  state and service
                                                  adapters

  Desktop shell           Tauri 2                 Windows desktop
                                                  packaging, lifecycle
                                                  and IPC

  Frontend                React + TypeScript      TV UI

  Build tooling           Vite                    Frontend
                                                  development/build

  UI styling              CSS modules or Tailwind Consistent tokens and
                          (choose one)            responsive layouts

  Database                SQLite                  Local persistent state

  DB access               SQLx or a carefully     Typed queries and
                          selected Rust SQLite    migrations
                          library                 

  Serialization           Serde                   Config and API data

  Async runtime           Tokio                   Asynchronous I/O and
                                                  background tasks

  Logging                 `tracing` + rolling     Structured diagnostics
                          file appender           

  Error handling          `thiserror` for domain  Consistent error
                          errors; `anyhow` at     reporting
                          application boundaries  

  Schema validation       Serde plus explicit     Reject invalid config
                          validation              

  Media playback          Technical spike         Choose based on codecs,
                          required                hardware acceleration,
                                                  subtitles and Tauri
                                                  integration

  Container runtime       Docker Engine-compatible         Required for v1
                          runtime; distribution PROPOSED   backend boundary
                          (dedicated WSL2 distro,          (ADR-0001 Part 2)
                          ADR-0001 Part 2)

  Windows installer       Tauri-compatible        Signed, reproducible
                          Windows installer       installation
                          tooling                 

  Tests                   Rust unit/integration   Automated verification
                          tests, frontend tests,  
                          Playwright/WebDriver    
                          where applicable        

  CI                      GitHub Actions          Formatting, linting,
                                                  tests and build checks
  -----------------------------------------------------------------------

### 3.1 Backend crate/module responsibilities

Use a modular monolith initially. Do not split into microservices
without measured need.

-   `domain`: entities, invariants and domain rules; no UI or
    container-runtime dependencies.
-   `application`: use cases and orchestration; calls domain logic and
    ports.
-   `infrastructure`: SQLite, filesystem, player and external service
    adapters.
-   `platform/windows`: Windows-specific APIs, process management,
    startup and installer integration.
-   `runtime`: required adapter for the approved Docker-compatible runtime/Compose lifecycle; runtime details stay outside the domain layer.
-   `api`: Tauri commands and DTOs; validates input and delegates to
    application use cases.

### 3.2 Frontend responsibilities

The frontend must not implement security-critical business rules. It may
present state and collect user intent, but the Rust backend must
validate every operation.

Frontend owns:

-   Navigation and visual presentation
-   Search/filter controls
-   Focus and remote-friendly interaction
-   Playback controls presentation
-   Download/queue presentation
-   Setup wizard screens
-   Error and status display

Backend owns:

-   File access policy
-   Path validation
-   Database writes
-   Download controls
-   Deletion and retention policy
-   Process/runtime management
-   Authentication/secrets if any are introduced
-   Network binding policy
-   All privileged operations

------------------------------------------------------------------------

# 4. Product modules and detailed responsibilities

## 4.1 Setup and preflight

Responsibilities:

-   Detect Windows edition, version, architecture and build.
-   Detect available RAM, free disk space, CPU architecture and
    virtualization support.
-   Detect whether the device is x64 or ARM64.
-   Detect whether WSL2 is available only if the chosen runtime needs
    it.
-   Detect existing Docker/Docker Desktop installations without modifying them.
-   Check whether required ports are already occupied.
-   Explain prerequisites and why each is needed.
-   Present a clear summary before installing optional infrastructure.
-   Record setup results and provide a diagnostic bundle.

Rules:

-   Read-only checks first.
-   Never enable virtualization, install WSL, change Windows features,
    alter firewall rules or install software without explicit user
    consent.
-   If a reboot is needed, stop and ask the user to schedule it.
-   Do not assume administrator rights.
-   Do not change the user's default Docker context or existing Docker
    installation without approval.
-   Provide a clean abort path.

## 4.2 Library manager

Responsibilities:

-   Import user-selected media folders.
-   Store canonical paths and file metadata.
-   Identify duplicate files without deleting them.
-   Track scan status and errors.
-   Support rescan and removal from the app's library without deleting
    the original file.
-   Preserve user data when the app is upgraded or uninstalled.

Rules:

-   Never scan the entire user profile, Desktop, Documents or all drives
    by default.
-   Only access folders the user explicitly selects.
-   Do not move, rename or delete original media as part of library
    scanning.
-   Handle inaccessible, offline and removable drives gracefully.
-   Treat filenames and embedded metadata as untrusted input.

## 4.3 Metadata and artwork

Responsibilities:

-   Store title, year, description, genres, artwork references and user
    corrections.
-   Cache downloaded metadata/artwork locally.
-   Work with no network connection using cached/local data.
-   Show placeholders when artwork is unavailable.
-   Allow manual correction.
-   Make external metadata providers optional and clearly disclose
    network requests.

Rules:

-   No mandatory account or paid metadata API.
-   Do not upload filenames or library contents without explicit opt-in.
-   Avoid scraping services in violation of their terms.
-   Sanitize text and image metadata before display.
-   Apply cache size limits and cleanup rules.

## 4.4 Playback manager

Responsibilities:

-   Start, pause, seek, stop and resume playback.
-   Persist playback position periodically and at safe lifecycle events.
-   Track media duration and completion state.
-   Handle subtitles, audio tracks and aspect ratio where supported.
-   Recover cleanly from missing files, unsupported codecs and player
    crashes.
-   Provide next/previous episode actions.

Completion logic:

-   Do not mark an item complete from a single seek-to-end event.
-   Use a configurable threshold based on watched duration and remaining
    time.
-   Distinguish user-initiated stop from completion.
-   Preserve progress if playback crashes.
-   Never delete media merely because playback is marked complete.

**OPEN DECISION:** Select the playback engine after a proof of concept
comparing codec coverage, hardware decoding on Intel/AMD/NVIDIA,
subtitles, HDR needs, memory use, licensing and Tauri integration.
Document the chosen engine and its licences in `docs/decisions/`.

The **placement** question is already answered in ADR-0002: the engine runs on the
Windows host, because the display, GPU, audio endpoint and vsync exist only there, and
WSL2 offers no cross-vendor hardware video-decode path. The engine selection, the
`--hwdec`/`--gpu-context` configuration, the LGPL-vs-GPL licence choice (O-09) and the
codec matrix remain open.

## 4.5 Queue and Up Next
Responsibilities:

-   Maintain a durable queue.
-   Support manual reorder, remove, pause and resume.
-   Keep current item, next item and download state separate.
-   Recover queue state after application restart.
-   Avoid duplicate queue entries unless explicitly allowed.
-   Keep a history of completed/removed queue entries where useful.

Queue state must be stored in SQLite. Do not rely only on frontend
memory.

## 4.5.1 Watchlist

Responsibilities:

-   Save and remove media from a watchlist, from any media card or details
    surface.
-   Keep the user's manual order. Nothing reorders it automatically.
-   Support movies and episodes.
-   Show entries whose file has gone missing as unavailable, and keep them —
    never silently drop a row because a file moved.
-   Work with the network disconnected. The watchlist is local state.

Rules:

-   The watchlist is **not** the queue. The queue is "play next"; the watchlist
    is "someday". Adding to one must not add to the other.
-   The watchlist is not an automatic recommendation list. The MVP's
    "Recommended from Your Library" shelf is derived deterministically from
    local data and is not editable into the watchlist.
-   Removing a watchlist entry deletes **no media**. It deletes a row.
-   Watchlist state is stored in SQLite and is backed up with the rest of the
    database.
-   No cloud sync, no account, no sharing. If a sync feature is ever proposed
    it requires its own spec, threat model entry and ADR.

## 4.6 Download manager — optional Docker-managed feature

Downloads are not required for the core local-media MVP. Build the
integration behind a provider interface and do not enable it by default
until the product's legal and security review is complete.

The application must be content-source neutral and must not bundle
piracy indexes, tracker lists, copyrighted-content search or
instructions for obtaining unauthorised content.

If a user configures a lawful download provider:

-   Use an adapter interface; do not couple queue logic directly to a
    specific provider.
-   Use documented APIs where available.
-   Require explicit configuration and user action before starting
    downloads.
-   Enforce download directory boundaries.
-   Enforce per-task and total storage limits.
-   Provide pause, resume, cancel and status.
-   Treat filenames, metadata and downloaded files as untrusted.
-   Do not execute downloaded files.
-   Do not expose a download WebUI to the LAN or internet by default.
-   Use unique credentials for any local control API.
-   Bind control interfaces to loopback or a private application-only
    channel.
-   Never store credentials in source code, logs or plaintext
    configuration.
-   If an optional VPN-dependent mode is supported, fail closed when the
    tunnel drops; never silently fall back to an unprotected route.
-   Do not require a paid VPN or claim a VPN makes unlawful activity
    lawful.

### Download lifecycle

``` text
Requested by user
      |
      v
Validate provider + destination + limits
      |
      v
Queued -> Downloading -> Verifying -> Available
   |          |               |
   |          v               v
   +------ Paused/Failed   Error/Retry
                              |
                              v
                         User-visible status
```

Do not automatically delete a completed download after playback. Cleanup
must follow an explicit retention policy.

## 4.7 Storage manager and cleanup

Responsibilities:

-   Report used/free space for managed storage.
-   Set maximum storage usage and per-category limits.
-   Pause downloads at a high-water mark.
-   Resume only when enough space is available.
-   Identify orphaned temporary files.
-   Provide a dry-run cleanup preview.
-   Keep an audit log of deletions.
-   Support configurable retention.

Deletion safety:

1.  Never delete original user media by default.
2.  Never delete unfinished downloads.
3.  Never delete a file currently being played or written.
4.  Never delete solely because a queue item disappeared.
5.  Use a multi-signal completion/retention rule.
6.  Require user approval for the first cleanup and for any change to
    deletion policy.
7.  Prefer moving managed files to an application-managed
    trash/quarantine area before permanent deletion, where feasible.
8.  Make permanent deletion explicit and recoverability limits clear.
9.  Resolve and validate canonical paths; reject traversal,
    symlink/reparse-point escapes and paths outside the managed root.
10. Use atomic state transitions and recover after power loss.

Suggested initial defaults (must be configurable and validated):

-   Warn when managed storage reaches 80%.
-   Pause new downloads at 90%.
-   Do not automatically delete anything in the MVP.
-   Offer a manual cleanup preview.

## 4.8 Settings and user preferences

Store preferences in a versioned config format and validate on load.
Settings include:

-   Theme and UI preferences
-   Startup behaviour
-   Library folders
-   Playback preferences
-   Storage root and limits
-   Optional provider settings
-   Runtime status
-   Diagnostics and privacy controls

Do not put secrets in ordinary settings. Use Windows Credential Manager
or another reviewed OS secret store if credentials become necessary.

## 4.9 Diagnostics and health

Provide:

-   Runtime status
-   Database status
-   Storage availability
-   Player status
-   Optional container service status
-   Recent error summaries
-   Exportable diagnostic bundle

Diagnostics must redact tokens, passwords, private URLs, personal file
paths where possible, and media filenames unless the user explicitly
includes them.

------------------------------------------------------------------------

# 5. Data model

Use SQLite with migrations. Keep data local under the application data
directory, not in the installation directory.

Suggested initial entities:

  -----------------------------------------------------------------------
  Table                               Purpose
  ----------------------------------- -----------------------------------
  `schema_migrations`                 Applied database migrations

  `media_items`                       Media identity, path, type,
                                      duration and technical metadata

  `media_files`                       One or more file locations
                                      associated with a media item

  `collections`                       Shows, seasons, folders or user
                                      collections

  `playback_sessions`                 Playback start/stop and progress
                                      checkpoints

  `playback_state`                    Current resume position and
                                      completion state

  `queue_items`                       Ordered queue and state

  `watchlist_items`                   User-saved watch-later entries
                                      and their manual order

  `downloads`                         Optional provider task references
                                      and status

  `storage_records`                   Managed file ownership and
                                      retention metadata

  `settings`                          Non-secret preferences

  `metadata_cache`                    Cached metadata and artwork
                                      references

  `audit_events`                      Destructive actions and policy
                                      changes
  -----------------------------------------------------------------------

Requirements:

-   Use foreign keys.
-   Use transactions for multi-row state changes.
-   Add indexes for common library, queue and playback queries.
-   Store timestamps in UTC in a consistent representation.
-   Store paths in a platform-aware form and normalise only for
    comparison.
-   Never store passwords or API tokens in `settings`.
-   Database migrations must be forward-safe and tested against a copy
    of representative prior-version data.
-   Back up the database before schema migrations that could alter or
    remove data.

------------------------------------------------------------------------

# 6. Filesystem layout and repository structure

## 6.1 Installed application data and container persistence

Keep host-side application files, runtime state, container-persistent data and original media as distinct storage classes. The following is a logical proposal, not an approved physical path layout:

```text
%LOCALAPPDATA%/ProjectName/                 # host-side config, logs, diagnostics, exports

Docker-managed persistent storage          # core SQLite/config/cache; ADR-0006 proposal
  core-data
  managed-media (only if an approved feature requires it)

User-selected source folders               # original paths, narrowly mounted read-only
```

The user may choose a managed-media or backup destination during setup. Do not assume the system drive has enough free space. Never store the only copy of state in a container writable layer or ephemeral storage. A named volume survives container recreation but is not a backup; provide a verified export/restore path. Confirm exact mount scope, Windows path translation, volume lifecycle and backup behavior before implementation.

A named volume inside the WSL2 distro is **not** a secret store: WSL exposes distro
filesystems to the Windows host via `\\wsl$`, so a Windows process running as the same
user can read and write them. The volume is a boundary between containers, not a
confidentiality boundary against the Windows user (R-15). Keep secrets in Windows
Credential Manager.

## 6.2 Proposed repository tree

``` text
project-root/
├── Development.md
├── AGENTS.md
├── README.md
├── LICENSE
├── CONTRIBUTING.md
├── SECURITY.md
├── CHANGELOG.md
├── roadmap.md
├── progress.md
├── .gitignore
├── .editorconfig
├── .github/
│   ├── workflows/
│   │   ├── ci.yml
│   │   ├── security.yml
│   │   └── release.yml
│   ├── ISSUE_TEMPLATE/
│   └── PULL_REQUEST_TEMPLATE.md
│
├── apps/
│   └── desktop/
│       ├── package.json
│       ├── vite.config.ts
│       ├── tsconfig.json
│       ├── index.html
│       ├── src/
│       │   ├── app/
│       │   │   ├── App.tsx
│       │   │   ├── routes.tsx
│       │   │   └── providers/
│       │   ├── components/
│       │   │   ├── navigation/
│       │   │   ├── media/
│       │   │   ├── player/
│       │   │   ├── cards/
│       │   │   ├── shelves/
│       │   │   ├── dialogs/
│       │   │   └── feedback/
│       │   ├── pages/
│       │   │   ├── Home/
│       │   │   ├── Movies/
│       │   │   ├── Shows/
│       │   │   ├── Library/
│       │   │   ├── Watchlist/
│       │   │   ├── Downloads/
│       │   │   ├── Search/
│       │   │   ├── Player/
│       │   │   ├── Settings/
│       │   │   └── Setup/
│       │   ├── hooks/
│       │   ├── state/
│       │   ├── services/
│       │   ├── styles/
│       │   │   ├── tokens.css
│       │   │   ├── global.css
│       │   │   └── motion.css
│       │   ├── types/
│       │   └── test/
│       └── src-tauri/
│           ├── Cargo.toml
│           ├── tauri.conf.json
│           ├── capabilities/
│           ├── icons/
│           ├── resources/
│           └── src/
│               ├── main.rs
│               ├── commands/
│               └── lib.rs
│
├── crates/
│   ├── app-core/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── domain/
│   │       ├── application/
│   │       ├── ports/
│   │       └── events/
│   ├── persistence/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── sqlite/
│   │       └── migrations/
│   ├── media-library/
│   │   └── src/
│   ├── playback/
│   │   └── src/
│   ├── queue/
│   │   └── src/
│   ├── storage-policy/
│   │   └── src/
│   ├── metadata/
│   │   └── src/
│   ├── downloads/
│   │   └── src/
│   ├── runtime-adapter/
│   │   └── src/
│   ├── platform-windows/
│   │   └── src/
│   └── diagnostics/
│       └── src/
│
├── infrastructure/
│   ├── containers/
│   │   ├── compose.yaml
│   │   └── README.md
│   ├── runtime/
│   │   ├── kiri/
│   │   └── compatibility/
│   └── packaging/
│       └── windows/
│
├── scripts/
│   ├── windows/
│   │   ├── preflight.ps1
│   │   ├── install-dev-deps.ps1
│   │   ├── build.ps1
│   │   ├── test.ps1
│   │   ├── verify.ps1
│   │   ├── diagnostics.ps1
│   │   └── uninstall-dev.ps1
│   └── common/
│
├── tests/
│   ├── integration/
│   ├── e2e/
│   ├── performance/
│   ├── security/
│   ├── fixtures/
│   └── test-media/
│
└── docs/
    ├── architecture/
    ├── decisions/
    ├── design/
    ├── security/
    ├── testing/
    ├── setup/
    └── operations/
```

This is a target structure, not a requirement to create every directory
on day one. Create folders when a real module needs them. Avoid empty
scaffolding and unnecessary crates.

## 6.3 File ownership rules

  -------------------------------------------------------------------------------------
  Location                        What belongs there            What must not go there
  ------------------------------- ----------------------------- -----------------------
  `apps/desktop/src/pages`        Screen composition and        Filesystem deletion
                                  page-level UI                 logic, SQL

  `apps/desktop/src/components`   Reusable UI components        Product policy or
                                                                privileged operations

  `apps/desktop/src/services`     Typed frontend wrappers       Direct OS commands
                                  around Tauri commands         

  `src-tauri/src/commands`        Validated UI command boundary Large domain
                                                                implementations

  `crates/app-core`               Domain and use-case logic     React, Tauri UI details

  `crates/persistence`            SQL queries and migrations    UI-specific logic

  `crates/media-library`          Library scanning/import and   Arbitrary whole-drive
                                  metadata association          scanning

  `crates/playback`               Player adapter and playback   Automatic deletion
                                  state                         

  `crates/queue`                  Queue ordering and            Provider-specific
                                  transitions                   network code

  `crates/storage-policy`         Storage limits, retention and Unrestricted filesystem
                                  safe cleanup                  access

  `crates/downloads`              Optional provider             Hard-coded content
                                  abstractions and adapters     sources

  `crates/runtime-adapter`        Docker-compatible        Product business rules
                                  runtime abstraction           

  `crates/platform-windows`       Windows-specific APIs         Cross-platform domain
                                                                logic

  `scripts/windows`               Repeatable                    Hidden destructive
                                  setup/build/test/diagnostic   behaviour
                                  scripts                       

  `infrastructure`                Container/runtime             Persistent user data in
                                  configuration                 image layers

  `tests`                         Automated tests and fixtures  Real user data or
                                                                copyrighted test media

  `docs/decisions`                Architecture Decision Records Unreviewed assumptions
                                  (ADRs)                        presented as facts
  -------------------------------------------------------------------------------------

------------------------------------------------------------------------

# 7. UI and navigation specification

## 7.1 Main screens

1.  **Home**
    -   Featured hero area
    -   Continue Watching
    -   Recently Added
    -   Up Next
    -   Recently Played
    -   Empty state with an Import Media action
2.  **Movies**
    -   Grid/shelf browsing
    -   Sort and filter
    -   Details view
3.  **TV Shows**
    -   Series, seasons and episodes
    -   Episode progress
    -   Next episode
4.  **My Library**
    -   User-selected folders and indexed media
    -   Scan status
    -   Missing/offline files
5.  **Downloads** (only if optional download feature is enabled)
    -   Active, queued, paused, completed and failed
    -   Storage meter
    -   Clear per-task controls
6.  **Search**
    -   Search local library first
    -   Optional metadata search only if configured
    -   Clear indication of online searches
7.  **Watchlist**
    -   Items the user has saved to watch later
    -   Movies and episodes, in the order the user arranged them
    -   Add and remove from any media card or details surface
    -   Reorder by hand; no automatic reordering
    -   Works fully offline; it is local state, not a cloud list
    -   Missing files are shown as unavailable, never silently dropped
8.  **Player**
    -   Full-screen playback
    -   Play/pause, seek, volume
    -   Subtitle/audio track controls where supported
    -   Episode next/previous
    -   Resume and completion behaviour
9.  **Settings**
    -   General
    -   Library folders
    -   Playback
    -   Storage
    -   Optional services
    -   Diagnostics
    -   About, licences and updates
10. **First-run setup**
    -   Welcome and purpose
    -   System compatibility check
    -   Storage location
    -   Library import
    -   Optional service choices
    -   Final review and explicit confirmation

## 7.2 TV navigation requirements

-   Every interactive control must have a predictable focus order.
-   Focus must remain visible at TV viewing distance.
-   Arrow keys must navigate shelves and cards.
-   Enter/Space activates focused items.
-   Escape/Back returns to the previous view or exits overlays.
-   Focus must be restored when dialogs close.
-   Mouse and keyboard remain supported.
-   Do not rely on hover to reveal essential controls.
-   Avoid tiny text and low-contrast secondary information.
-   Support reduced-motion preferences.
-   Do not animate large artwork in ways that cause dropped frames on
    low-end hardware.

## 7.3 Design tokens

Create central tokens for:

-   Backgrounds and surfaces
-   Text hierarchy
-   Accent and focus colours
-   Spacing scale
-   Corner radii
-   Elevation/shadows
-   Motion durations and easing
-   Typography sizes
-   Focus ring thickness

Do not scatter hard-coded colours and spacing values through components.
Use a consistent dark palette, with accessible contrast and restrained
translucent surfaces. The design reference is directional, not a licence
to copy another product's assets.

## 7.4 Apple TV-inspired visual direction — detailed implementation contract

Create an original, premium, cinematic TV interface inspired by the clarity,
scale and browsing rhythm of Apple TV. This is a design reference, not a request
to reproduce Apple's exact UI, trademarks, icons, proprietary assets or
screenshots. Use original components and properly licensed artwork.

### 7.4.1 Overall composition

- Default to a dark, edge-to-edge canvas with a near-black/charcoal base.
- Use a restrained translucent navigation rail/header where it improves
  hierarchy; do not make every surface glass-like.
- Let high-quality landscape artwork dominate the home screen.
- Use generous spacing, large readable typography and clear hierarchy.
- Use gradients behind hero text to preserve contrast over artwork.
- Avoid dense tables, admin-dashboard styling and excessive borders.
- The UI must scale from a laptop display to a 1080p/4K television at typical
  living-room viewing distance.
- Provide a compact mode only as a secondary option; TV mode is the default.

### 7.4.2 Main application shell

The application is one continuous experience. Top-level navigation changes the
content view, not the application identity or backend. Recommended navigation:

- Home
- Movies
- TV Shows
- Library
- Search
- Downloads (only when enabled)
- Settings

The active navigation item must be obvious but visually quiet. Keep navigation
available without consuming excessive vertical space. On TV/remote use, the
navigation can collapse to icons with accessible labels; on mouse use, labels
may remain visible. Never require the user to switch to another app to manage
media, downloads or playback.

### 7.4.3 Home screen hierarchy

The Home screen is a vertically scrolling collection of media shelves. Its
content is data-driven; shelves are not hard-coded to particular titles.

Recommended order, conditionally rendered when data exists:

1. Featured / Continue Watching hero
2. Continue Watching shelf
3. Up Next shelf (series queue and next episode)
4. Recently Added Movies
5. Keep Watching / In Progress
6. Recently Added Shows
7. Recommended from Your Library (deterministic local rules only in MVP)
8. Recently Played
9. Watchlist
10. Collections / Genres

Do not show empty shelves. If the library is empty, show a designed onboarding
state with a prominent `Add Media Folder` action and a short explanation.

### 7.4.4 Hero feature

- Use a wide landscape backdrop, ideally 16:9 or wider source artwork.
- Apply a dark left-to-right and bottom gradient for legible text.
- Display title, year, content type, runtime/season information where known,
  a short synopsis and a subtle metadata line.
- Primary action: Play or Resume.
- Secondary actions: More Info and Add to Queue where applicable.
- Do not autoplay trailers or sound on focus.
- Hero rotation, if enabled, must be slow, deterministic and pauseable; never
  change the hero while the user is interacting with its controls.

### 7.4.5 Media cards and horizontal shelves

The core browsing interaction is horizontal carousels of movies, shows,
seasons, episodes and collections. These are content rows inside one app, not
separate applications or service views.

- Movie/show posters use portrait artwork (approximately 2:3).
- Featured and continue-watching cards may use landscape artwork (16:9).
- Episode cards may use landscape stills (16:9) with episode number/title.
- Cards must have consistent image ratios, rounded corners and stable layout.
- Show title and secondary metadata beneath or over the artwork with strong
  contrast; do not truncate every title unnecessarily.
- Continue Watching cards show a progress bar and resume action.
- Downloaded/available/offline states use small, consistent indicators.
- Hover/focus enlarges a card subtly (roughly 1.03–1.06 scale) with a shadow or
  focus ring; it must not cause neighbouring cards to jump or reflow.
- Focused cards may reveal actions such as Play, Details and Queue, but core
  actions must remain keyboard-accessible.
- Use virtualisation or incremental rendering for very large libraries.
- Lazy-load artwork and provide skeleton/placeholder states.
- Artwork failures must not break card dimensions or navigation.

### 7.4.6 Details view

Selecting a movie or show opens an in-app detail surface/page, not an external
service UI. It should contain backdrop/poster artwork, title, year, synopsis,
genres, runtime or season count, playback state and clear actions.

For shows, include season selection and an episode list with watched/progress
states, episode stills, duration and a Play/Resume action. Show the next episode
and queue/download state when relevant. For movies, provide Play/Resume, Add to
Queue, and metadata details. Distinguish missing local files from items that
are available to play.

### 7.4.7 Player experience

- Provide a distraction-free full-screen player.
- Controls appear on user input and fade after a configurable idle period.
- Include play/pause, seek, elapsed/remaining time, volume, subtitle/audio
  selection where supported, next episode and back/details.
- Seeking must be smooth and report actual seek completion.
- Show a brief title/episode overlay when playback begins.
- At the end of an episode, show a clear next-episode prompt and countdown only
  if autoplay is enabled; default autoplay to off until the user chooses.
- Preserve progress on pause, stop, crash and application shutdown.
- Playback completion is a backend decision based on watched duration and
  remaining duration, not merely a seek event.

### 7.4.8 Motion and interaction

- Use restrained transitions (roughly 140–240 ms for ordinary focus/overlay
  transitions); tune after profiling.
- Avoid parallax-heavy effects, constant animated backgrounds and blur layers
  that cause high GPU/CPU use.
- Respect reduced-motion settings.
- Focus movement must be deterministic. Horizontal arrows move within a shelf;
  vertical arrows move between shelves while preserving the nearest column.
- Left/right at shelf edges must not unexpectedly jump to unrelated controls.
- Back/Escape closes the topmost overlay before navigating away.
- Keep focus restoration after dialogs, details views and player exit.
- Support keyboard, mouse, game controller and common remote key events where
  available. Provide a visible focus state even when a mouse was used last.

### 7.4.9 Accessibility and readability

- Meet WCAG AA contrast for normal text where applicable.
- Use scalable typography and avoid essential text below 16 CSS px in TV mode.
- Never convey download, playback or error state by colour alone.
- Provide accessible names for icon-only buttons.
- Ensure all actions are reachable without a mouse.
- Provide captions/subtitle settings and respect OS reduced-motion preference.

### 7.4.10 Frontend component system

Build reusable components rather than page-specific copies:

- `AppShell`, `PrimaryNavigation`, `PageHeader`
- `HeroFeature`, `MediaShelf`, `MediaCard`, `EpisodeCard`, `CollectionCard`
- `ProgressBar`, `StatusPill`, `Artwork`, `SkeletonCard`
- `MediaDetails`, `SeasonPicker`, `EpisodeList`
- `PlayerControls`, `PlaybackOverlay`, `UpNextPrompt`
- `SearchInput`, `FilterBar`, `EmptyState`, `ConfirmDialog`, `Toast`
- `FocusManager` and a tested directional-navigation hook/service

Use design tokens for colours, typography, spacing, radii, shadows, focus
rings, z-index layers and motion. Keep content data separate from presentation.
Do not hard-code real media titles or poster URLs into production UI.

### 7.4.11 Responsive behaviour

- TV/large display: large hero, wide shelves, larger cards and generous gaps.
- Laptop: reduce hero height and card dimensions while preserving the same
  information hierarchy.
- Narrow window: allow horizontal shelves to scroll and collapse navigation;
  do not turn the entire interface into a cramped desktop dashboard.
- Full-screen mode must be supported and recover gracefully when display
  resolution or monitor changes.

### 7.4.12 Frontend acceptance criteria

- A user can browse the entire app using only directional keys and Enter/Back.
- Horizontal shelves scroll smoothly without layout shifts.
- Focus is always visible and restored predictably.
- Hero text remains legible on bright and dark artwork.
- Large libraries do not render every image eagerly.
- Loading, empty, offline, missing-artwork and error states are designed.
- Playback controls are usable at TV distance.
- The app remains one coherent UI for movies, shows, downloads, queue and
  settings; no internal service dashboard is exposed as the primary UI.

------------------------------------------------------------------------

# 8. Security, privacy and safety requirements

Security must be enforced by the application and scripts. Do not rely on
an AI agent to remember to behave safely.

## 8.1 Threat model

Assume:

-   User-selected media folders may contain malicious or malformed
    files.
-   Filenames and metadata may contain hostile strings.
-   A downloaded image, subtitle or media file may be malformed.
-   A local service may accidentally bind to all network interfaces.
-   A container may be compromised.
-   A setup script may run with elevated privileges.
-   An update or dependency may be compromised.
-   The laptop may be shared with other household users.
-   Power may be lost during writes or cleanup.

## 8.2 Least privilege

-   Run the application as the normal Windows user.
-   Do not require administrator privileges for normal operation.
-   Elevate only for a specific documented system change.
-   Do not run the media app as LocalSystem.
-   Do not mount the Docker/Docker Desktop control socket into application
    containers.
-   Do not use privileged containers.
-   Drop unnecessary Linux capabilities.
-   Use read-only container filesystems where practical.
-   Apply CPU, memory and process limits to every container, including the
    required workspace.
-   Require the container runtime to run in WSL2's default NAT networking
    mode; mirrored mode is rejected by preflight because it makes published
    ports LAN-reachable.
-   Do not silently reuse an existing shared container runtime. Detect one,
    report it, and obtain consent before using it.
-   Never mount the entire Windows home directory or system drive into a
    container.
-   Mount only the exact managed folders required, with read-only access
    unless writes are necessary.

## 8.3 Network security

-   Bind local control APIs to loopback or a private IPC channel.
-   Do not expose management ports to the LAN or public internet by
    default.
-   Do not silently alter Windows Firewall rules.
-   Do not use unauthenticated WebUIs.
-   Use random, per-install credentials if a local service requires
    credentials.
-   Keep secrets out of command-line arguments where possible.
-   Do not log tokens, passwords, session identifiers or private URLs.
-   No remote-access feature until a separate security design is
    approved.

## 8.4 Filesystem security

-   Restrict operations to user-selected and app-managed roots.
-   Canonicalise paths before destructive operations.
-   Protect against `..` traversal, symlink and Windows reparse-point
    escapes.
-   Use safe temporary directories and atomic file replacement.
-   Do not trust file extensions or MIME types alone.
-   Never execute media, subtitle, archive or downloaded files.
-   Ensure cleanup cannot escape the managed storage root.
-   Keep source library files read-only from the application's
    perspective unless the user explicitly chooses otherwise.

## 8.5 Secrets and privacy

-   No mandatory online account.
-   No telemetry by default.
-   If optional telemetry is ever added, it must be opt-in and
    documented.
-   Use OS-backed secret storage if credentials are introduced.
-   Redact sensitive values from logs and diagnostic bundles.
-   Do not transmit library filenames, media metadata or watch history
    without explicit consent.
-   Explain every external network request in settings or setup.

## 8.6 Updates and supply chain

-   Use official release sources.
-   Pin dependencies and review lockfile changes.
-   Verify checksums and signatures where available.
-   Never disable antivirus, SmartScreen, Defender or Windows security
    features to make installation easier.
-   Do not download and execute arbitrary scripts from a URL.
-   Prefer reviewed, versioned scripts in the repository.
-   Keep third-party licences and notices.
-   Produce an SBOM for release builds where feasible.
-   Run dependency and secret scanning in CI.
-   Do not auto-update optional infrastructure without a recovery plan.

## 8.7 Destructive operations

Before deleting, overwriting, moving or changing permissions on user
data:

-   Explain exactly what will change.
-   Show the affected paths and item counts.
-   Provide a dry-run/preview.
-   Require explicit confirmation.
-   Create a backup or reversible quarantine where practical.
-   Log the operation.
-   Verify the result.
-   Stop if paths or ownership are ambiguous.

------------------------------------------------------------------------

# 9. Reliability and resource management

## 9.1 Target hardware profiles

Do not promise support for every old laptop. Establish and test hardware
tiers.

Making Docker mandatory for v1 raises the memory floor, because the WSL2 virtual
machine and the container runtime have a cost a bare host process would not. Docker
Desktop's own WSL2 backend lists 8 GB of system RAM as a system requirement. The tiers
below are therefore **candidates until measured**, and the 4 GB tier is not a support
target.

  ------------------------------------------------------------------------
  Tier                                           RAM Expected support
  --------------------- ---------------------------- ---------------------
  Minimum experiment                            4 GB NOT a support target.
                                                     A 4 GB machine cannot
                                                     carry the WSL2 VM plus
                                                     the required workspace
                                                     plus Windows playback.

  Entry                                         8 GB Candidate test profile;
                                                     full runtime + core + probe
                                                     must be measured

  Recommended                                  16 GB Candidate full-MVP profile;
                                                     runtime and host playback
                                                     must be measured

  Higher                                      32 GB+ Candidate profile for
                                                     heavier workloads; no
                                                     headless/streaming claim
  ------------------------------------------------------------------------

These are initial test categories, not guaranteed minimum
specifications. The playback engine, codec and OS configuration can
change the result. P0-10 and P0-14 measure the real numbers; until then
no minimum specification is published.

## 9.2 Resource policies

-   Measure idle RAM and CPU after startup.
-   Avoid polling loops; use events or sensible backoff.
-   Pause background scans during active playback if necessary.
-   Limit artwork cache size.
-   Limit concurrent metadata lookups.
-   Limit download concurrency and bandwidth.
-   Apply container memory/CPU limits.
-   Measure the Docker-compatible runtime, `core` and `probe` together and separately from Windows playback; operate the required workspace only for application use, with a bounded lifecycle.
-   Avoid transcoding by default; prefer direct play where possible.
-   Show when a file requires transcoding or unsupported decoding.

## 9.3 Recovery behaviour

-   Persist queue and playback state transactionally.
-   Recover incomplete scans after restart.
-   Detect interrupted downloads and ask provider to resume or verify.
-   Reconcile app state with optional service state at startup.
-   Use bounded retries with backoff.
-   Do not restart a failing service indefinitely.
-   Provide a recovery UI that can explain an unavailable or unhealthy required runtime/workspace. Backend-dependent features remain unavailable; never silently run the backend outside its approved container boundary.
-   Provide a recovery/diagnostics command that does not delete data.

------------------------------------------------------------------------

# 10. Installation, updates and uninstall

## 10.1 Installer design

Use a deterministic, version-controlled installer. AI agents may invoke
it, but must not invent ad hoc system modifications.

Installation stages:

1.  Read-only preflight
2.  Show compatibility and prerequisites
3.  Show intended changes
4.  Ask for consent
5.  Install app files
6.  Create per-user data directories
7.  Initialise database and migrations
8.  Provision or detect the required Docker-compatible runtime/workspace only after the user approves the exact host changes; never install or enable host features automatically
9.  Run verification checks
10. Show completion and recovery instructions

Every stage must be idempotent: rerunning it should safely detect
completed work and avoid duplicate configuration.

## 10.2 Updates

-   Back up database and configuration before migrations.
-   Verify package integrity.
-   Use staged update or rollback where feasible.
-   Never overwrite user media.
-   Preserve settings and queue.
-   If migration fails, restore the prior known-good state or leave the
    app in a recoverable mode.
-   Document update compatibility and minimum supported version.

## 10.3 Uninstall

Offer two distinct choices:

-   **Remove application only:** preserve database, settings, artwork
    and managed media.
-   **Remove application and managed data:** show exact paths and
    require a separate explicit confirmation.

Never remove user-selected source media during uninstall. Do not uninstall WSL2, Docker Desktop or any shared Docker-compatible runtime if other applications may depend on them. Detect shared installations and leave them intact. If a dedicated runtime distribution or volume is owned by MEDIA_HAVEN, show its exact scope and require a separate explicit confirmation after a verified backup before removal.

------------------------------------------------------------------------

# 11. Testing strategy

Testing is a release requirement, not a final polish step.

## 11.1 Test layers

### Unit tests

Test domain rules in isolation:

-   Queue ordering and transitions
-   Playback completion thresholds
-   Storage thresholds
-   Retention decisions
-   Path validation
-   Configuration parsing
-   Database mapping
-   Error classification

### Integration tests

Test real boundaries:

-   SQLite migrations and transactions
-   Filesystem import and rescan
-   Player adapter events
-   Optional runtime adapter
-   Download provider adapter using mocks or a local test service
-   Process startup/shutdown and recovery

### End-to-end tests

Test complete user journeys:

1.  First launch
2.  Preflight and setup
3.  Import a test media folder
4.  Browse library
5.  Start playback
6.  Pause, resume and seek
7.  Close/reopen and restore progress
8.  Queue next item
9.  Recover after forced app termination
10. Uninstall while preserving data

### Security tests

-   Path traversal attempts
-   Symlink/reparse-point escapes
-   Malformed filenames and metadata
-   Oversized artwork/subtitles
-   Local API binding checks
-   Missing/invalid credentials
-   Container privilege and mount audit
-   Secret leakage in logs
-   Destructive cleanup dry-run and confirmation
-   Update integrity failure

### Performance tests

Measure:

-   Cold startup and warm startup
-   Idle RAM and CPU
-   Library scan speed
-   UI frame stability while browsing
-   Playback CPU/RAM
-   Seek latency
-   Database growth
-   Artwork cache growth
-   Container runtime overhead (if enabled)
-   Recovery after sleep/resume and reboot

## 11.2 Test matrix

At minimum, test:

-   Windows 11 x64 clean install
-   Windows 11 x64 with existing Docker-compatible runtime / WSL present
-   Windows 11 x64 without WSL2, if Docker Desktop is used
-   4 GB, 8 GB and 16 GB RAM categories
-   SSD and HDD storage
-   Integrated graphics
-   720p and 1080p sample files
-   Common codecs and subtitle formats
-   HDMI/secondary display
-   Keyboard and remote-style navigation
-   Offline mode
-   Low disk space
-   Interrupted installation and update

Windows 10 support must be treated cautiously because its standard
support ended in October 2025. Do not promise secure general-purpose
Windows 10 support; verify the user's actual servicing status and
supported prerequisites.

## 11.3 Test media

Use small, legally distributable sample clips with known codec/subtitle
characteristics. Do not commit copyrighted movies or TV episodes. Keep
test fixtures small and document their source and licence.

## 11.4 Definition of done

A feature is not done merely because it compiles.

A feature is done only when:

-   Requirements are documented.
-   Relevant tests pass.
-   Error paths are handled.
-   Security and privacy implications are reviewed.
-   UI states include loading, empty, success and failure where
    applicable.
-   Logs are useful and redact sensitive data.
-   Resource impact is measured where relevant.
-   Documentation is updated.
-   `roadmap.md` and `progress.md` are updated.
-   No unrelated regressions are introduced.

------------------------------------------------------------------------

# 12. Development workflow and coding standards

## 12.1 General rules

-   Prefer small, reviewable changes.
-   Keep the app modular but avoid premature microservices.
-   Use typed interfaces and explicit error types.
-   Avoid `unwrap()`/`expect()` in normal production paths.
-   Do not swallow errors silently.
-   Do not use shell commands where a safe library/API exists.
-   Validate all external input at boundaries.
-   Keep UI components presentational where possible.
-   Do not duplicate business rules between frontend and backend.
-   Add tests with bug fixes.
-   Explain non-obvious design decisions in code or ADRs.
-   Avoid introducing a dependency for trivial functionality.
-   Pin and review dependency changes.
-   Keep secrets, user data and build artifacts out of Git.

## 12.2 Rust checks

Run from the Rust workspace root:

``` powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo check --workspace
```

Use the project's actual workspace/package structure. If a command is
not applicable yet, document why rather than pretending it passed.

## 12.3 Frontend checks

Typical checks, adjusted to the selected package scripts:

``` powershell
npm ci
npm run lint
npm run typecheck
npm test
npm run build
```

Use the lockfile and `npm ci` in CI. Do not replace the lockfile
casually.

## 12.4 PowerShell script standards

-   Use strict mode where practical.
-   Set `$ErrorActionPreference = 'Stop'`.
-   Check exit codes.
-   Quote paths safely.
-   Avoid `Invoke-Expression`.
-   Avoid downloading and executing remote scripts.
-   Make scripts idempotent.
-   Provide `-WhatIf`/dry-run behaviour for destructive operations.
-   Log actions without exposing secrets.
-   Check that a target path is within the expected managed root before
    deletion.
-   Stop on unexpected state; do not attempt clever automatic repairs
    that could damage user configuration.

## 12.5 Architecture Decision Records

For any major decision, create `docs/decisions/NNNN-title.md` with:

-   Status: proposed / accepted / superseded
-   Context
-   Decision
-   Alternatives considered
-   Trade-offs
-   Security/privacy impact
-   Performance/resource impact
-   Migration/reversal plan

At minimum, create ADRs for:

-   Playback engine
-   Windows runtime distribution for the mandatory Docker/Compose boundary
-   Data and storage layout
-   Installer and update mechanism
-   Metadata provider strategy
-   Download integration, if approved

------------------------------------------------------------------------

# 13. roadmap.md --- required roadmap structure and maintenance

`roadmap.md` is the project sequencing document. It must describe
outcomes, dependencies, acceptance criteria and current status. It is
not a vague wish list.

## 13.1 Required structure

``` markdown
# Roadmap

## Status legend
- [ ] Not started
- [~] In progress
- [x] Complete
- [!] Blocked
- [?] Needs decision

## Current milestone
Name, goal, exit criteria and owner/agent.

## Phase 0 — Discovery and technical validation
Tasks, dependencies, experiments and decisions.

## Phase 1 — Foundation
Repository, build, CI, app shell, logging and config.

## Phase 2 — Core local library
Import, scan, SQLite, browse and search.

## Phase 3 — Playback
Player spike, controls, resume state, completion rules.

## Phase 4 — TV experience
Home, shelves, focus navigation, settings and setup.

## Phase 5 — Reliability and security
Recovery, storage policy, threat tests, diagnostics.

## Phase 6 — Required Docker workspace and optional providers
Implement and verify the required Docker/Compose backend and media-inspection boundary. Additional providers remain optional and separately approved.

## Phase 7 — Packaging and release
Installer, update, uninstall, compatibility matrix.

## Deferred / future
Features intentionally outside the current release.

## Risks and open decisions
Owner, impact, next action and decision deadline/trigger.
```

## 13.2 Roadmap task format

Each substantial task must contain:

``` markdown
### [ ] TASK-ID — Short outcome-focused title
- **Goal:** What user-visible or engineering outcome is delivered?
- **Scope:** What is included?
- **Out of scope:** What must not be changed?
- **Dependencies:** Required completed tasks or decisions.
- **Implementation areas:** Expected folders/modules.
- **Acceptance criteria:**
  - [ ] Observable condition 1
  - [ ] Observable condition 2
- **Tests:** Unit/integration/E2E/security/performance checks.
- **Risks:** Data loss, compatibility, privacy or resource concerns.
- **Status:** Not started / In progress / Blocked / Complete
```

## 13.3 Roadmap rules

-   Order work by dependency and risk, not excitement.
-   Validate the riskiest assumptions early: playback, low-memory operation, Windows installation/lifecycle, Docker runtime distribution, and host/container networking and isolation.
-   Do not start a dependent phase while a blocking decision is
    unresolved.
-   Do not mark tasks complete because code was generated; use
    acceptance criteria and tests.
-   Split tasks that are too large to implement and verify in one
    focused change.
-   Keep future ideas in Deferred/Future rather than expanding the MVP.
-   When scope changes, explain why and update affected tasks.
-   Link relevant ADRs, issues and tests.
-   Update roadmap status when work starts, becomes blocked or is
    completed.
-   Do not rewrite the whole roadmap every session; make focused edits
    and preserve history where useful.

## 13.4 Initial recommended roadmap

### Phase 0 --- Discovery and technical validation

-   [ ] P0-01 Confirm MVP: direct HDMI/local playback first.
-   [ ] P0-02 Create a Tauri + React + Rust proof of concept.
-   [ ] P0-03 Compare playback engine candidates on Windows.
-   [ ] P0-04 Benchmark Windows host, required runtime/workspace and playback resources; state untested profiles honestly.
-   [ ] P0-05 Validate candidate Docker-compatible Windows runtime distributions and boundary requirements using primary docs and an approved test environment.
-   [ ] P0-06 Recommend a supported distribution for the mandatory Docker/Compose boundary, with evidence and human review.
-   [ ] P0-07 Review threat model and ADRs for the mandatory boundary, runtime and playback decisions.
-   [ ] P0-08 Prove the TV focus/shelf navigation model with a keyboard-only prototype.
-   [ ] P0-09 Write the Phase 1 execution plan.
-   [ ] P0-10 Measure idle and active cost of each candidate runtime distribution on this machine.
-   [ ] P0-11 Attempt hardware video decode inside a container and record the actual result.
-   [ ] P0-12 Record the native-Windows codec, GPU, subtitle and audio matrix.
-   [ ] P0-13 Prove host reachability and LAN non-reachability of the published API port.
-   [ ] P0-14 Baseline the total added footprint of the required workspace.

### Phase 1 --- Foundation

-   [ ] P1-01 Create workspace and repository conventions.
-   [ ] P1-02 Add CI for Rust and frontend.
-   [ ] P1-03 Implement structured logging and redaction.
-   [ ] P1-04 Implement versioned configuration.
-   [ ] P1-05 Implement SQLite migrations and backup.
-   [ ] P1-06 Build first-run preflight and setup flow.
-   [ ] P1-07 Stand up the required Docker runtime and the `core`/`probe` workspace.
-   [ ] P1-08 Implement the host↔container boundary, authentication and supervisor lifecycle.

### Phase 2 --- Local media library

-   [ ] P2-01 Add user-selected folder import.
-   [ ] P2-02 Implement safe scanner and metadata extraction.
-   [ ] P2-03 Implement media database entities.
-   [ ] P2-04 Build Movies, Shows and Library views.
-   [ ] P2-05 Add search, sorting, filtering and empty/error states.
-   [ ] P2-06 Implement the metadata and artwork cache with an offline fallback.
-   [ ] P2-07 Implement the local watchlist.

### Phase 3 --- Playback and queue

-   [ ] P3-01 Integrate selected player engine.
-   [ ] P3-02 Implement playback controls.
-   [ ] P3-03 Persist resume position.
-   [ ] P3-04 Implement completion logic and tests.
-   [ ] P3-05 Test subtitles, audio tracks and hardware decoding.
-   [ ] P3-06 Implement durable queue and Up Next.

### Phase 4 --- TV interface

-   [ ] P4-01 Implement design tokens and responsive layout.
-   [ ] P4-02 Build Home hero and media shelves.
-   [ ] P4-03 Implement keyboard/remote focus navigation.
-   [ ] P4-04 Build details, settings and setup screens.
-   [ ] P4-05 Validate readability and performance at TV distance.
-   [ ] P4-06 Define the typed command and DTO contract (ADR-0009).
-   [ ] P4-07 Build the development mock transport.
-   [ ] P4-08 Integrate the frontend with the real backend.

### Phase 5 --- Reliability, security and storage

-   [ ] P5-01 Implement storage usage and configurable limits.
-   [ ] P5-02 Implement non-destructive cleanup preview.
-   [ ] P5-03 Add crash/restart recovery.
-   [ ] P5-04 Run path traversal and filesystem boundary tests.
-   [ ] P5-05 Review local network exposure and process permissions.
-   [ ] P5-06 Build diagnostic export with redaction.
-   [ ] P5-07 Run low-disk, offline and power-interruption tests.

### Phase 6 --- Workspace hardening, optional providers and lifecycle verification

-   [ ] P6-01 Audit and harden the required workspace's security controls.
-   [ ] P6-02 Add an optional lawful provider only after separate legal/security approval; otherwise defer it explicitly.
-   [ ] P6-03 Verify required-workspace startup/shutdown, resource limits, recovery, persistence and uninstall behavior.

### Phase 7 --- Packaging and release

-   [ ] P7-01 Build reproducible Windows installer.
-   [ ] P7-02 Test install/upgrade/rollback/uninstall.
-   [ ] P7-03 Verify signatures/checksums and dependency licences.
-   [ ] P7-04 Run full Windows test matrix.
-   [ ] P7-05 Publish setup guide, troubleshooting and release notes.

------------------------------------------------------------------------

# 14. progress.md --- required progress and handover protocol

`progress.md` is the durable, concise record of what has actually
happened. It is not a second roadmap and must not become a transcript of
every chat message.

Every agent session must read `Development.md`, `roadmap.md` and
`progress.md` before changing code.

## 14.1 Required format

``` markdown
# Progress

## Last updated
YYYY-MM-DD HH:MM UTC

## Current state
One short paragraph describing what works today and the current milestone.

## Active task
- **ID:** P?-??
- **Goal:**
- **Status:** In progress / Blocked
- **Branch/commit:** if available

## Completed this session
- Concrete change and relevant file paths.
- Tests run and exact outcome.

## Current implementation facts
- Important modules and their responsibilities.
- Current runtime/player/database decisions.
- Known working commands.

## Verification
| Check | Result | Notes |
|---|---|---|
| Rust formatting | PASS/FAIL/NOT RUN | |
| Rust tests | PASS/FAIL/NOT RUN | |
| Frontend checks | PASS/FAIL/NOT RUN | |
| Build | PASS/FAIL/NOT RUN | |
| E2E | PASS/FAIL/NOT RUN | |

## Blockers and failures
- Exact error, reproduction steps, what was tried, and what remains unknown.

## Known risks / regressions
- Anything that may break, lose data, expose a port or consume excessive resources.

## Next actions
1. One small, concrete next task.
2. Its acceptance criteria.
3. Tests to run.

## Decisions made
- Link ADRs and identify who/what approved the decision.

## Open questions
- Questions requiring user/maintainer input.
```

## 14.2 Progress rules

-   Update at the end of every meaningful work session.
-   Update before handing work to another agent.
-   Record actual test outcomes; never claim a test passed if it was not
    run.
-   Distinguish `NOT RUN` from `PASS`.
-   Record exact commands and relevant error output where useful.
-   Include file paths and meaningful changes, not vague claims such as
    "improved backend".
-   Record decisions as proposed or accepted. An agent cannot mark a
    major product decision accepted without user/maintainer approval.
-   Keep the next action small enough to be safely resumed.
-   Remove stale status, but preserve useful history in Git commits.
-   Never put secrets, tokens, private media paths or personal data in
    progress notes.

## 14.3 Usage-limit and context handover protocol

When an agent is nearing a usage limit, context limit, session end or
interruption:

1.  Stop starting new work.
2.  Finish or safely pause the current operation.
3.  Do not leave a destructive operation half-explained.
4.  Save all relevant code changes to the working tree; do not commit
    unless requested or project policy allows it.
5.  Update `progress.md` with exact state.
6.  Update `roadmap.md` only if task status or scope genuinely changed.
7.  Record tests run and tests not run.
8.  Record uncommitted files and any known partial implementation.
9.  State the single safest next action.
10. Do not claim completion when the task is partial.

A handover must allow a new agent to continue without relying on the
previous chat transcript.

### Handover template

``` markdown
## Agent handover
- **Task:** P?-??
- **Current state:** Exact point reached.
- **Files changed:** List.
- **Commands run:** List and outcomes.
- **Tests:** PASS / FAIL / NOT RUN.
- **Uncommitted changes:** List.
- **Known hazards:** Any incomplete migration, process, download, or data operation.
- **Do not repeat:** Failed approaches and why.
- **Next safe action:** One specific step.
```

------------------------------------------------------------------------

# 15. AGENTS.md --- instructions every coding agent must follow

Create a concise root `AGENTS.md` that points to this document and
imposes the following rules. The full agent contract is included here so
this file can be derived from it.

## 15.1 Before starting

1.  Read `Development.md`, `roadmap.md`, `progress.md`, `AGENTS.md` and
    relevant ADRs.
2.  Inspect the repository and current Git status.
3.  Identify the active roadmap task.
4.  Confirm the task's scope and acceptance criteria.
5.  Identify whether the task touches user data, system settings,
    networking, credentials, installation or deletion.
6.  If a required decision is open, do not guess. Propose options and
    ask for approval where required.

## 15.2 Permission boundaries

The agent may, without separate approval:

-   Read repository files.
-   Search within the repository.
-   Edit project source and documentation within the agreed task.
-   Run non-destructive formatting, linting and tests.
-   Create test fixtures that contain no personal data.
-   Run local builds that do not install or alter the host system.

The agent must ask before:

-   Installing software or system dependencies.
-   Downloading and executing installers.
-   Enabling WSL2, Hyper-V, virtualization features or Windows optional
    features.
-   Requesting or using administrator privileges.
-   Changing firewall, network, proxy, DNS or security settings.
-   Logging into accounts or using credentials.
-   Accessing folders outside the repository or user-selected media
    roots.
-   Deleting, moving or overwriting user data.
-   Installing, removing or reconfiguring Docker, Docker Desktop, WSL2 or another
    runtime.
-   Starting services that listen on non-loopback interfaces.
-   Making external network requests that are not necessary for the
    approved task.
-   Publishing releases, pushing commits or changing repository
    permissions.

If a tool environment or user instruction already grants a specific
action, remain within that exact scope. Never infer broad permission
from a narrow approval.

## 15.3 Implementation rules

-   Make the smallest change that satisfies the task.
-   Do not refactor unrelated modules.
-   Do not silently change the stack or architecture.
-   Do not introduce a new runtime or service without an ADR and
    approval.
-   Do not invent APIs or claim a dependency supports a feature without
    verifying its documentation.
-   Do not hard-code secrets, credentials, private endpoints or user
    paths.
-   Use mocks/fakes for external services in tests.
-   Validate all inputs at trust boundaries.
-   Ensure every destructive operation has a preview/confirmation
    policy.
-   Keep frontend commands narrow and typed.
-   Backend validates all arguments regardless of frontend validation.
-   Add regression tests for bug fixes.
-   Update docs when behaviour or setup changes.

## 15.4 Agent verification checklist

Before reporting a task complete:

-   [ ] Acceptance criteria met
-   [ ] Scope remained within task
-   [ ] Formatting and lint checks run
-   [ ] Relevant tests run
-   [ ] Build checked
-   [ ] Security implications reviewed
-   [ ] No secrets or user data added
-   [ ] No destructive host changes performed without approval
-   [ ] `roadmap.md` updated
-   [ ] `progress.md` updated
-   [ ] Remaining risks and unverified claims disclosed

## 15.5 Reporting format

At the end of a task, report:

1.  What changed
2.  Files changed
3.  Tests and commands run
4.  Actual results
5.  What was not tested
6.  Risks or open decisions
7.  Next recommended roadmap task

Do not say "everything works" when only a build succeeded.

------------------------------------------------------------------------

# 16. Documentation and maintenance requirements

Keep these documents current:

-   `Development.md`: stable product and engineering contract
-   `AGENTS.md`: short operational instructions for agents, linking here
-   `roadmap.md`: phases, tasks, dependencies and acceptance criteria
-   `progress.md`: current implementation state and handover
-   `README.md`: what the product is, screenshots, install and quick
    start
-   `SECURITY.md`: vulnerability reporting and supported versions
-   `CONTRIBUTING.md`: development setup, style and PR process
-   `CHANGELOG.md`: user-visible changes
-   `docs/decisions/`: accepted/proposed architecture decisions
-   `docs/testing/`: test matrix and reproducible procedures
-   `docs/operations/`: backup, recovery, diagnostics and uninstall

When a change affects setup, update both the documentation and the
corresponding scripts/tests. Documentation must describe actual
behaviour, not intended behaviour.

------------------------------------------------------------------------

# 17. Release gates

Do not publish a public release until all gates pass:

### Product gate

-   Core local library and playback flows work.
-   TV navigation works with keyboard and remote-style controls.
-   First-run experience is understandable without technical knowledge.

### Reliability gate

-   Restart and reboot recovery tested.
-   Low disk and offline behaviour tested.
-   Database backup/migration recovery tested.
-   No known data-loss bug remains open.

### Security gate

-   Least-privilege review complete.
-   Filesystem boundary tests pass.
-   No unintended listening ports.
-   No secrets in repository or logs.
-   Dependencies and licences reviewed.
-   Installer integrity and update process reviewed.

### Performance gate

-   Idle resource use measured.
-   Playback performance measured on target hardware tiers.
-   Required runtime/workspace overhead measured and included in hardware-tier claims; unsupported profiles remain unproven.
-   No unsupported minimum hardware claim.

### Documentation gate

-   Setup, troubleshooting, privacy and uninstall instructions current.
-   `roadmap.md` and `progress.md` current.
-   Known limitations published.

------------------------------------------------------------------------

# 18. Open decisions --- do not silently assume

These decisions require prototype evidence or user approval:

1.  **Playback engine:** DECIDED 2026-11-18 — mpv via JSON IPC, LGPL build,
    host-side placement (ADR-0002, O-09). The shipped artefact must still be
    confirmed LGPL-configured (P0-12) before release.
2.  **Windows Docker runtime/provider:** Docker Engine-compatible runtime and
    Compose are mandatory (ADR-0001 Part 1). Distribution DECIDED 2026-11-18 —
    dedicated WSL2 distro, no reuse of an existing engine (ADR-0001 Part 2,
    O-06/O-07). P0-10/P0-13/P0-14 measurements are still owed.
3.  **Playback licence:** DECIDED 2026-11-18 — LGPL mpv build (O-09). A hard
    gate on CP-0 remains: confirm the actual artefact is LGPL-configured.
4.  **Media server:** defer unless headless or multi-device streaming is
    a confirmed requirement.
5.  **Download integration:** **REOPENED 2026-11-20 (ADR-0017)** — authorized-media
    acquisition via qBittorrent is in scope per the consolidated specification.
    Lawful use only; no bundled trackers/indexers; see ADR-0018/0019 and the
    conversion plan.
6.  **Metadata providers:** DECIDED 2026-11-18 — TMDB primary with local NFO
    offline fallback, network opt-in and off by default (ADR-0011, O-04).
7.  **Offline guarantee:** decide whether the complete app must work
    indefinitely without internet after installation. Local playback
    should work offline regardless.
8.  **Windows 10 support:** DECIDED 2026-11-18 — Windows 11 x64 only for v1
    (O-02). Do not claim Windows 10 or ARM64.
9.  **Remote access:** out of MVP.
10. **Project branding/name:** DECIDED 2026-11-18 — "MEDIA HAVEN" is the project
    and app name (O-01). The package/bundle ID is still needed before P7-01.

------------------------------------------------------------------------

# 19. Final engineering principle

Build the smallest reliable local media appliance first.

Do not confuse a large technology stack with a better product. The v1 proposal is a Tauri shell and host supervisor, a Docker-managed Rust backend with SQLite, an isolated media-inspection worker, and a playback engine whose Windows-host placement remains to be validated. The runtime distribution, resource cost, networking, egress, mounts and playback/licensing assumptions are unverified until CP-0 evidence is accepted. Add containers beyond the required backend/inspection boundary, a media server or download integrations only when a measured requirement justifies them.

The AI agent's job is to implement the approved plan, verify it,
document reality and hand over cleanly. It is not authorised to invent
product requirements, weaken security, install unapproved software, or
perform destructive actions just to make a demo appear complete.


------------------------------------------------------------------------

# 20. Consolidated project decisions and implementation priorities

This section resolves ambiguity in earlier exploratory wording. If an older
paragraph elsewhere in this document appears to conflict with this section,
this section is authoritative.

## 20.1 Product shape

MEDIA_HAVEN is one integrated media application. Movies, TV shows, episodes,
collections, downloads, queue management, playback, settings and diagnostics
are presented through one frontend and one navigation system. Horizontal media
shelves/carousels are the primary browsing pattern. Internal containers and
services must not create separate user-facing apps.

## 20.2 Docker direction

Docker Engine-compatible containers and Docker Compose are mandatory in v1 for the Rust backend and proposed isolated media-inspection worker. This is a required security boundary, not an optional service, and does not require containerising the Tauri UI or playback. The application remains one product; the container backend owns product rules, and the Windows frontend receives named product operations without Docker daemon access. The supported Windows runtime distribution, lifecycle, resource cost, network exposure, egress, mounts and persistence remain unverified until evidence is recorded and reviewed. Installation or host configuration requires explicit user consent. If the runtime is unavailable, show a clear recovery state; never use a host-side backend fallback.

## 20.3 Media lifecycle and storage correction

The desired product workflow may automatically prepare the next episode and
clean up an episode after it has been watched, but deletion must be safer than
a single playback-complete event. Use persistent Docker volumes or managed
folders outside disposable container layers. Keep source media separate and
read-only. Only app-managed downloads may be eligible for automated retention.

For the first release, automatic permanent deletion is OFF by default. The
user may enable an explicit retention policy after reviewing it. The default
queue depth is one upcoming episode, configurable to one or two; never fetch an
entire season automatically. A completed episode may be moved to managed trash
first. Permanent deletion occurs only after the configured retention period
and safety checks. Never delete unfinished, currently playing, user-imported
source or unverified files.

A completion decision must use multiple signals: playback position near the
end, watched-duration threshold, a normal end-of-file/player completion event
where available, and a short grace period. Seeking to the end, closing the app,
crashing, or removing an item from the queue must not trigger deletion. Require
first-time explicit consent, provide a preview and audit log, and make the
policy easy to disable.

## 20.4 Implementation order

1. Establish repository conventions and read-only CI/tooling setup; do not run the product backend outside its approved container boundary.
2. Complete CP-0 evidence for runtime distribution, host/container boundary and networking, resource cost, and playback/GPU/audio/licensing; obtain human review before implementation.
3. Build the unified frontend shell and Apple TV-inspired design system with mock data; validate directional navigation early.
4. Build backend domain, persistence and typed API contract, with product execution in the required Docker workspace; tests may use controlled test fixtures but must not become a production host fallback.
5. Implement user-selected read-only folder import, metadata cache and library browsing through the container backend after mount/path evidence is accepted.
6. Select and integrate playback through a documented proof of concept; host-side playback is a proposal pending measured codec/GPU/audio/licensing results; implement resume and completion tracking.
7. Implement durable queue and Up Next.
8. Complete the required Compose `core`/`probe` workspace, runtime lifecycle, health, security, resource and persistence verification. Do not treat the current download-client skeleton as this workspace.
9. Add a lawful download-provider adapter only after separate legal/security review and explicit configuration; otherwise defer it.
10. Add managed-storage policies, dry-run cleanup, retention and optional automated cleanup only after end-to-end safety tests.
11. Complete installer, update/rollback, diagnostics, performance and release gates.

Do not start by integrating multiple third-party media applications and then
trying to unify their interfaces. Build the single MEDIA_HAVEN experience first
and connect internal services behind stable interfaces.

## 20.5 Mandatory first implementation deliverables

Before feature work expands, create and maintain:

- `roadmap.md` with phases, task IDs, dependencies and acceptance criteria.
- `progress.md` with actual implementation state and test outcomes.
- An ADR for the Windows Docker runtime distribution and its install/update/lifecycle model.
- An ADR for the host/container API and control boundary.
- An ADR for the media playback engine, including any deliberate Windows-host exception.
- A threat model covering Docker control-plane access, volume mounts, hostile
  media, local APIs, credentials and deletion.
- A Compose security checklist and image pinning/update policy.
- A frontend interaction prototype proving shelf navigation, focus restoration
  and player overlay behaviour.

The project is not complete merely because containers start or the UI resembles
a streaming service. It is complete only when the integrated product is safe,
recoverable, testable and usable by a non-technical person.
