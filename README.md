# MEDIA_HAVEN

A free, local-first media appliance for a spare Windows laptop plugged into a TV.

MEDIA_HAVEN turns an older Windows 11 machine into a living-room media player:
plug in an HDMI cable, open the app, point it at the folders you already own, and
browse and play them through a full-screen, remote-friendly interface.

- **Local-first.** Your files stay where they are. Nothing uploads your library.
- **Free.** No subscription, account, credit card or paid API.
- **Offline.** Playback and browsing work with the network disconnected.
- **No AI runtime.** After setup, the app never needs an AI service.
- **Non-destructive.** The app never deletes your source media.
- **Isolated.** The backend and media inspection run in a Docker workspace with
  read-only mounts, non-root containers and a single loopback-only API port.

> **Status: functional prototype, no checkpoint passed.** The React UI, Rust
> backend, Tauri host and local tests run today. Authorized-media acquisition,
> fail-closed media gates, offline IMDb metadata, keyboard navigation and an
> unsigned Windows installer have since been built. Real playback, the running
> container boundary, live provider requests and clean-VM install remain
> **NOT RUN**. See `progress.md` for the 2026-11-21 evidence and exact NOT RUN
> list.

## Read these first

| Document | Purpose |
|---|---|
| `all-in-one-final.txt` | Single-file overview: how it works, standout features, requirements and installation |
| `Development.md` | The authoritative product and engineering contract |
| `roadmap.md` | Phases, tasks, checkpoints and acceptance criteria |
| `progress.md` | What actually works today, and the handover |
| `AGENTS.md` | Rules for human and AI contributors |
| `docs/` | Architecture, ADRs, design, security, testing, operations |

## Stack

Docker/Compose-hosted Rust backend (`core` + `probe`) · Tauri 2 Windows shell and
supervisor · React + TypeScript (Vite) · SQLite.

**Docker is a mandatory v1 dependency.** The backend and the untrusted-media
inspection worker run in a hardened Compose workspace — non-root, read-only root
filesystems, dropped capabilities, resource limits, read-only media mounts and a
single loopback-only authenticated API. Containers are an internal detail: there is
still exactly one MEDIA_HAVEN application, and the user never sees a container
dashboard or types a Docker command.

The runtime distribution is decided as a dedicated WSL2 distro with Docker
Engine (ADR-0001 Part 2, accepted provisional 2026-11-18); the shipped images and
boundary evidence are not yet built. The host/container transport (ADR-0005) and
playback descriptor (ADR-0015) were accepted via the blanket human acceptance
(ADR-0016, 2026-11-20). ADR-0022 (IMDb) and ADR-0023 (TMDB artwork transport) are
**Proposed** and must not be treated as binding. If the required runtime is
unavailable, backend-dependent functions remain unavailable; there is no
host-side backend fallback.

## Repository layout

```
apps/desktop/      React UI + Tauri shell
crates/            Rust workspace (domain, persistence, library, playback, ...)
infrastructure/    Container and packaging configuration
scripts/windows/   Preflight, build, test, verify, diagnostics
docs/              Architecture, decisions, design, security, testing, operations
tests/             Integration, e2e, performance, security, fixtures
roadmap.md         Sequencing and checkpoints
progress.md        Current state
```

## Quick start (contributors)

The read-only preflight reports Windows version, architecture, RAM, free disk,
virtualisation capability, Docker presence and port availability. It changes
nothing on your machine — no installs, no feature toggles, no admin prompt:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/windows/preflight.ps1
```

The following were observed green on 2026-11-19:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

From `apps/desktop`:

```bash
npm run typecheck
npm run lint
npm test
npm run check:tokens
npm run check:bundle
npm run build
npm run dev
```

No Docker command is runnable without explicit consent. See `progress.md` for the
2026-11-19 evidence and the exact NOT RUN list.

## Tooling for AI agents

Design and UI skills live in `.agents/skills/` (symlinked into
`.claude/skills/`). A fresh clone does not carry them, because they are large
and environment-specific. To restore the expected set:

```bash
npx skills add anthropics/skills --skill frontend-design
npx impeccable install
npx skills add https://github.com/Leonxlnx/taste-skill
npx skills add nutlope/hallmark
npx skills add nextlevelbuilder/ui-ux-pro-max-skill
npx skills add google-labs-code/stitch-skills --skill stitch::extract-design-md
npx skills add shadcn/ui
npx skills add Dammyjay93/interface-design
npx skills add jakubkrehel/skills --skill better-typography
git clone https://github.com/dominikmartn/hue ~/.claude/skills/hue
```

Impeccable installs design-detector hooks into `.claude/settings.local.json`
and `.codex/hooks.json`. Leave them enabled; they run a fast check on UI edits
and a deeper pass on Stop.

## Licence

See `LICENSE`. Third-party attributions are listed in `docs/decisions/` and will
be bundled with the installer from Phase 7.

## Contributing and security

- `CONTRIBUTING.md` — workflow, standards, commit conventions.
- `SECURITY.md` — how to report a vulnerability. Do not open a public issue.
