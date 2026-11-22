# Changelog

All notable changes to this project are documented here. Format loosely follows
[Keep a Changelog](https://keepachangelog.com/); versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added (2026-11-22)
- **Published pre-release `v0.1.0`** at
  https://github.com/ATLAS-DEV78423/MEDIA-HAVEN/releases/tag/v0.1.0, tagged at
  `7a062c4`. A fresh unsigned per-user NSIS installer was rebuilt from current
  HEAD with `build.ps1` (exit 0), its SHA-256 verified, and published with
  `SHA256SUMS.txt`, `THIRD_PARTY_NOTICES.md` and `ALL-IN-ONE.md`. The release is
  explicitly marked **pre-release**: no checkpoint has passed, the installer is
  unsigned (SmartScreen warning expected), SBOM and clean-VM install remain
  NOT RUN, and the player is still a labelled preview.

### Shipped features (2026-11-20 / 2026-11-21)

Code-complete and covered by the gates recorded in `progress.md`. Behaviour that
needs an operator, lawfully sourced media, a real playback engine or
second-machine hardware remains **NOT RUN**; no checkpoint has passed.

- **Authorized-media torrent acquisition** (ADR-0017/0018/0019): a qBittorrent
  sandbox on an egress-only Docker network with no published port, loopback-only
  control with secret-store credentials, user-supplied magnet/`.torrent` entry,
  a rolling two-episode buffer with a 20 GiB cap, and cleanup of app-managed
  media only. No tracker, indexer or catalogue is bundled.
- **Fail-closed downloaded-media gates**: the file-inspection probe and a Windows
  Defender gate (`MpCmdRun`) are wired into `detail()` and `playback_start`. A
  downloaded item is playable only when both verifications are Clean and it is
  tied to a `media_id`; a `NULL` verification never becomes an allow.
- **R-01 per-item source mapping** (ADR-0021): an operator attaches a
  magnet/`.torrent` source to a library item on its Details screen; `item_sources`
  (migration 0007), the `source_set` command chain and a fail-closed verification
  guard. No discovery or bundled sources.
- **Offline IMDb metadata** (ADR-0022, **Proposed**): opt-in via
  `MEDIAHAVEN_IMDB_DIR`, reading operator-supplied `title.basics`, `ratings` and
  `age_ratings` TSVs; signed `/host/imdb_lookup`, a named renderer command, and
  card + Details display with attribution. Zero file reads when off. OMDb HTTP
  remains an unwired seam and needs its own ADR.
- **Unsigned per-user Windows installer** built and verified (v0.1.0, x64, NSIS,
  ~3.60 MiB) with checksums and third-party notices. The SmartScreen caveat
  applies (O-08) and builds are non-deterministic across runs (documented, not
  fixed). SBOM generation is **NOT RUN**.
- **Full arrow/Enter/Escape keyboard navigation**: a global Escape-to-back hook,
  Up/Down escaping text inputs into spatial navigation, and unit tests. A browser
  walkthrough was observed (see `progress.md`); the every-screen automated
  walkthrough remains open.
- **TMDB artwork implementation plan** added
  (`docs/superpowers/plans/2026-11-21-tmdb-artwork.md`). This is a **plan, not
  implemented code**: ADR-0023 is Proposed pending human acceptance, the HTTPS
  transport and hashed image cache are unwritten, and no live TMDB call has been
  made.
- **ADR-0024 (Proposed, minimal-change variant)** — host-side Torznab search
  returning a ranked shortlist that the operator confirms through the **existing**
  `source_set` command. No container, network, schema or gate change; no code or
  network call. Would supersede ADR-0021's no-discovery clause and amend ADR-0018
  §5 if accepted. O-10 tracks the open decision. A TMDB catalog is a separate
  follow-up. miauflix code was **not** used (PolyForm Noncommercial 1.0 licence).

### Changed
- **Docker/Compose is now a mandatory v1 dependency.** The backend and the
  media-inspection workload run in containers; there is no host-side backend
  fallback. This is reflected consistently across `Development.md`, the ADRs, the
  threat model, the Compose configuration, the roadmap and the operations docs.
- The Windows runtime distribution is now **Proposed**: Docker Engine (rootful) in a
  dedicated WSL2 distro, over Docker Desktop (ADR-0001 Part 2). Rationale: Docker
  Desktop requires a paid subscription for organisations above 250 employees or $10M
  revenue and for government entities regardless of size; it adds a privileged
  host-integration layer; and it shares one daemon and one VM budget with every
  other project on the machine.
- **Playback placement is decided:** the playback engine runs natively on Windows, an
  intentional exception to "everything backend-ish lives in Docker", because the
  display, GPU, audio endpoint and vsync exist only there and WSL2 offers no
  cross-vendor hardware video-decode path (ADR-0002).
- The roadmap now brings the required workspace forward into Phase 1 (P1-07
  workspace, P1-08 host↔container boundary) because Phase 2 onward is backend work
  and the backend *is* the `core` container. Phase 6 was consolidated from five tasks
  to three (hardening, optional provider, lifecycle verification).
- Phase 0 gained five tasks — P0-10 … P0-14 — covering runtime-distribution cost,
  the containerised GPU-decode experiment, the native codec matrix, host/LAN
  reachability and the workspace footprint baseline.
- `Development.md` §9.1 no longer treats 4 GB as a candidate tier: the required
  runtime raises the memory floor, and Docker Desktop's own WSL2 backend lists 8 GB
  of system RAM as a requirement.

### Added
- `docs/architecture/docker-workspace.md` — the required Compose workspace:
  container inventory, network boundary, the five filesystem classes, resource
  limits, least-privilege detail, lifecycle, and residual risks.
- ADR-0008 — the modular monolith deployed across the Windows host and the Docker
  backend.
- ADR-0007 (secrets management), ADR-0006 (filesystem and volume policy),
  ADR-0005 (host↔container boundary).
- `infrastructure/containers/compose.yaml` — the required `core`/`probe` workspace:
  non-root with distinct UIDs, `cap_drop: ALL`, `no-new-privileges`, read-only root
  filesystems, declared tmpfs and named volumes, CPU/memory/PID limits, a
  loopback-only published port, and an `internal: true` network with no gateway for
  `probe`. Images are digest-pinned and supplied as required variables, so Compose
  fails closed rather than pulling a mutable tag.
- Threat-model coverage for the container runtime and the WSL2 boundary: actors,
  entry points, abuse cases A19–A25, trust boundaries 6–7, residual risks R-08 … R-21.

### Fixed
- The Compose file no longer rejects `docker compose config` under Compose v2 —
  `pids_limit` was moved inside `deploy.resources.limits`.
- WSL2 **mirrored** networking mode was removed as a fallback for host→container
  reachability. It is designed to make the instance LAN-reachable and would have
  converted a loopback-only API publication into a LAN-reachable one.
- The threat model no longer says hostile media is parsed containerised "when Phase 6
  exists" — the required workspace is in v1 and the claim was wrong.
- The operations documentation no longer places the database in `%LOCALAPPDATA%`; it
  lives in the named volume `core-data` (ADR-0006).
- ADR-0003 is annotated with the fact that making Docker mandatory fired its own
  revisit trigger; its decision was not rewritten.

### Notes
- The earlier statement that "no application code exists yet" is **superseded** by
  the shipped features above: a React frontend, a seven-crate Rust backend, a Tauri
  host, SQLite persistence and a CI workflow all exist. See `progress.md` for the
  current state.
- The Compose file resolves cleanly and its static security posture is verified, but
  it has **not** been run against a running engine (needs explicit Docker/WSL2
  consent).
- The runtime distribution (ADR-0001 Part 2) and playback engine (ADR-0002) are
  **Accepted (provisional)** pending P0 measurement evidence; O-09 (libmpv LGPL
  build) still needs confirmation of the shipped artefact. ADR-0022 (IMDb) and the
  plan-derived ADR-0023 (TMDB artwork transport) are **Proposed** and require human
  acceptance.
- **CP-0 is not passed.** Every CP-0 criterion requires an observed measurement or a
  human sign-off; a document that says "recommended" is not evidence.
- Project name (MEDIA HAVEN) and package ID (`com.mediahaven.desktop`) are decided by
  ADR-0016; the earlier "not finalised" note is superseded.

### Added (earlier in this cycle)
- `Development.md` — the consolidated product and engineering specification.
- `roadmap.md` — 8 phases and 8 checkpoints (CP-0 … CP-7) with acceptance criteria,
  a deferred list, and a risk/open-decision register.
- `progress.md` — implementation state and handover protocol.
- `AGENTS.md` — rules every contributing agent must follow.
- Documentation baseline: `docs/architecture/` (overview, process model),
  `docs/decisions/` (ADR index plus ADR-0001 runtime, ADR-0002 playback,
  ADR-0003 modular monolith, ADR-0004 local-playback placeholder),
  `docs/design/` (design system, navigation spec),
  `docs/security/` (threat model, Compose checklist),
  `docs/testing/` (test matrix), `docs/setup/` (first run),
  `docs/operations/` (backup, recovery, uninstall).
- `scripts/windows/preflight.ps1` — read-only environment checks.
- `README.md`, `SECURITY.md`, `CONTRIBUTING.md`, `LICENSE`, `.editorconfig`,
  `.gitignore`.
