# Release runbook - Windows (v1, unsigned)

Status: **installer built and checksummed locally; not published.** A real
unsigned NSIS installer was produced on 2026-11-21 (recorded in `progress.md`):
`MEDIA_HAVEN_0.1.0_x64-setup.exe` (~3.60 MiB) with `SHA256SUMS.txt` and
`THIRD_PARTY_NOTICES.md`. The build output is gitignored, so it is not in the
repository — run the command below and verify the checksum of the artifact you
produce. Nothing has been published as a GitHub Release. SBOM and a clean-VM
install remain NOT RUN; see "Not run / known limitations".

## Prerequisites (already installed; nothing is auto-installed)

- Windows 11 x64 (v1 support claim, O-02).
- Node >= 22, npm, and `apps/desktop/node_modules` (`npm ci` requires approved
  network access). Local Tauri CLI is `@tauri-apps/cli` 2.12.5.
- Rust toolchain (CI pins 1.99.0). WebView2 tooling is assumed present on
  Windows 11; **not verified** here.
- No administrator rights are needed for the normal per-user install.

## Build preconditions (now satisfied)

The controller applied the packaging fixes recorded in `progress.md`, and a real
build succeeded. `apps/desktop/src-tauri/tauri.conf.json` now has `identifier`
`com.mediahaven.desktop` (O-01), `bundle.active: true`, `bundle.targets:
["nsis"]` and version `0.1.0`; `package-lock` version and the `build.ps1` output
path bug were also fixed. `build.ps1` still refuses a build that requests signing
or a non-`currentUser` install mode, by design.

NSIS defaults to `currentUser` (per-user, no admin) in Tauri CLI 2.12.5 - this
was verified against the CLI's local `config.schema.json`. The build script
rejects any explicit non-`currentUser` install mode and any signing config.

## Exact build command

```powershell
# From the repository root. Does not install, fetch, elevate or publish.
powershell -NoProfile -ExecutionPolicy Bypass -File infrastructure\packaging\windows\build.ps1
```

It runs `npm run build`, then `npx --no-install tauri build --bundles nsis`
(no network fetch of the CLI), then writes `SHA256SUMS.txt` and copies
`THIRD_PARTY_NOTICES.md` (plus `sbom/bom.json` if present) next to the installer
in `apps/desktop/src-tauri/target/release/bundle/nsis/`.

`beforeBuildCommand` also runs `npm run build`, so the frontend build happens
twice; this is harmless and kept so the explicit command is reproducible.

In CI, `.github/workflows/release.yml` performs the same build **after the
existing `CI` workflow succeeds on master**, then uploads artifacts. It does
not create a GitHub Release.

## Unsigned build - SmartScreen caveat (O-08)

v1 ships **unsigned**. Windows SmartScreen will show "Windows protected your
PC" / "unknown publisher" on first run. This is expected, not a defect. Publish
this text with every download:

> MEDIA HAVEN v1 is not code-signed. Windows may warn "Windows protected your
> PC" because the publisher is unverified. Verify the SHA-256 checksum below
> against the published `SHA256SUMS.txt` before running the installer. More
> Info > Run anyway is required on first launch. A signed build needs a future
> human budget decision and certificate (reopens O-08).

Do not hide the warning, and do not add a signing certificate without a new
ruling.

## Checksum procedure

Publisher: `SHA256SUMS.txt` sits beside the installer and is uploaded by CI.

Consumer, on the downloaded file:

```powershell
# In the folder holding the installer and SHA256SUMS.txt
Get-FileHash -Algorithm SHA256 .\MEDIA_HAVEN_0.1.0_x64-setup.exe
# Compare the (lowercase) hash with the line for that filename in SHA256SUMS.txt.
# Alternative: certutil -hashfile .\MEDIA_HAVEN_0.1.0_x64-setup.exe SHA256
```

`SHA256SUMS.txt` lines are `<64 lowercase hex>  <relative filename>` (two
spaces), the standard `sha256sum` layout. Do not accept a checksum relayed only
through the same channel as the binary.

## Compliance artifacts

- `infrastructure/packaging/windows/THIRD_PARTY_NOTICES.md` - declared licences
  for 536 Rust crates and 343 npm packages. Regenerate with
  `python infrastructure/packaging/windows/collect_licenses.py` (offline,
  deterministic). 177 Rust licences are `unresolved offline` because those
  crate manifests are not in the local Cargo cache; resolving them needs an
  approved fetch.
- IMDb attribution ("Ratings and age ratings: IMDb / OMDb") is rendered in the
  UI where IMDb data appears. Verify the notice text against the shipped build
  before release; TMDB artwork attribution is not yet implemented (the artwork
  transport is an unexecuted plan, P6-07).
- `infrastructure/packaging/windows/sbom/README.md` - SBOM **NOT RUN**; contains
  the exact operator commands (syft / cargo-cyclonedx / cyclonedx-npm) for an
  approved run. The notice is not a SBOM.

## Not run / known limitations

- **Installer build: PASS (local, 2026-11-21).** A real artifact and its
  `SHA256SUMS.txt` were produced; `build.ps1` gate exited 0. The output is
  gitignored and has not been published.
- **SBOM: NOT RUN** (all tools missing - see `sbom/README.md`).
- **Reproducibility: UNVERIFIED.** The hash changed between builds; the cause is
  documented rather than fixed, which meets only the disclosure half of the
  rebuild criterion.
- **No live media, no real download source, no hardware proof.** Codec, GPU,
  audio, subtitle and low-end-tier claims are unproven (CP gates outstanding).
- **No CP sign-offs.** Product/reliability/security/performance/documentation
  gates in `Development.md` §17 are not passed by this runbook.
- **Installer integrity/update path** and **uninstall data behaviour** were not
  exercised against a built installer.
- **Full licence texts** are not inlined; they remain in the dependency sources
  and must be included in any source distribution.

Publishing any release before these are closed is not authorised by this task.
