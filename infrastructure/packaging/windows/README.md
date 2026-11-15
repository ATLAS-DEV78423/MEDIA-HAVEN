# infrastructure/packaging/windows

Windows installer and release packaging.

**Status: empty.** Created by `roadmap.md` P7-01, after CP-5 passes.

## What will live here

| Item | Purpose |
|---|---|
| Bundle configuration | Tauri bundler settings, per-user install where possible |
| Version stamping | Single source of truth for the app version |
| Checksum generation | Published alongside each artifact |
| SBOM generation | Attached to the release |
| Licence/notice collection | Third-party attributions bundled and shown in-app |
| Build script | Reproducible build invoked by CI and by a human |

## Constraints

- **Reproducible.** Two builds from the same commit must produce identical
  checksums, or the non-determinism must be documented with its cause (P7-01).
- **No admin by default.** The normal install path must not require elevation.
- **No host mutation.** The installer does not enable Windows features, change
  firewall rules or install runtimes. Anything optional is offered later, inside
  the app, with consent (`docs/setup/first-run.md`).
- **Data-safe uninstall.** Uninstall asks explicitly about app data and never
  touches user source media (`docs/operations/backup-recovery-uninstall.md`).
- **Signing is an open decision** (`roadmap.md` O-08) — an unsigned build will
  trip SmartScreen, and that must be stated rather than hidden.

## Blocking decisions before this directory is populated

- O-01: project name, branding, package ID (a neutral placeholder is in use).
- O-08: installer signing certificate.

Do not invent a product name to unblock packaging. Ask.
