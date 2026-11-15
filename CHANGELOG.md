# Changelog

All notable changes to this project are documented here. Format loosely follows
[Keep a Changelog](https://keepachangelog.com/); versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

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
- No application code exists yet. Nothing is runnable beyond the preflight script.
- The Compose file resolves cleanly and its static security posture is verified, but
  it has **not** been run against a running engine.
- The runtime distribution, the playback engine and its licence (O-09), and the
  host↔container transport remain **Proposed** and require human acceptance.
- **CP-0 is not passed.** Every CP-0 criterion requires an observed measurement or a
  human sign-off; a document that says "recommended" is not evidence.
- Project name, branding and package ID are not finalised.

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
