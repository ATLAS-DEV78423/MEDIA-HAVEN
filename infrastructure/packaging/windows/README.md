# infrastructure/packaging/windows

Windows installer and release packaging for MEDIA HAVEN.

**Status: artifact built and published as a GitHub pre-release.** A fresh
unsigned per-user NSIS installer was rebuilt from HEAD `7a062c4` on 2026-11-22
and published at
https://github.com/ATLAS-DEV78423/MEDIA-HAVEN/releases/tag/v0.1.0-beta with
`SHA256SUMS.txt`, `THIRD_PARTY_NOTICES.md` and `ALL-IN-ONE.md`. SBOM and
clean-VM install remain NOT RUN. The older note that config preconditions were
unmet was superseded by the packaging fixes recorded in `docs/operations/release.md`.

## Files

| File | Purpose |
|---|---|
| `build.ps1` | Reproducible unsigned, per-user NSIS build + SHA-256 checksums. Installs/fetches/publishes nothing. |
| `collect_licenses.py` | Offline, deterministic licence inventory from `Cargo.lock` + `package-lock.json`. |
| `THIRD_PARTY_NOTICES.md` | Generated notice (536 Rust crates, 343 npm packages; 177 Rust licences unresolved offline). Regenerate with the script; do not hand-edit. |
| `sbom/README.md` | SBOM **NOT RUN** - tool probe and exact commands for an approved operator run. |
| `../../.github/workflows/release.yml` | Runs after CI succeeds, builds, checksums and uploads artifacts. Never publishes a release. |
| `../../docs/operations/release.md` | Build/checksum/verify runbook, SmartScreen caveat, known limitations. |

## Reproducible build

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File infrastructure\packaging\windows\build.ps1
```

The script verifies the Tauri config first and **fails** (exit 2) if it is not
shaped for the decided packaging contract. It then runs `npm run build` and
`npx --no-install tauri build --bundles nsis`, writes `SHA256SUMS.txt`, and
copies `THIRD_PARTY_NOTICES.md` (and `sbom/bom.json` if present) beside the
installer. No admin rights, no host mutation, no publishing.

## Controller action required (Task 3 has no write access to these files)

`apps/desktop/src-tauri/tauri.conf.json` currently blocks a build:

| Setting | Current | Required |
|---|---|---|
| `identifier` | `app.mediahaven.desktop` | `com.mediahaven.desktop` (O-01) |
| `bundle.active` | `false` | `true` |
| `bundle.targets` | `all` | `["nsis"]` (NSIS defaults to `currentUser` in Tauri CLI 2.12.5 - verified from local `config.schema.json`) |
| version | `0.1.0` | single source; `package.json` is `0.0.0` and `Cargo.toml` is `0.1.0` - align before publishing |

Task 3 did not edit these files; it gates on them instead of guessing.

## Constraints honoured

- **Reproducible.** The licence notice has no date stamp (verified byte-identical
  across two runs). Installer reproducibility itself is **UNVERIFIED** - no build
  was run, so two builds have not been compared.
- **No admin by default.** NSIS `installMode` defaults to `currentUser`; the
  script rejects any other explicit value.
- **No host mutation.** No Windows feature, firewall, runtime or service change.
- **Data-safe uninstall.** Uninstaller configuration is unchanged; app data is
  separate from source media (`docs/operations/backup-recovery-uninstall.md`).
- **Unsigned (O-08).** SmartScreen warning is expected and published in
  `docs/operations/release.md`. No signing certificate is configured.
- **SBOM NOT RUN**, third-party notices partial (177 Rust licences unresolved
  offline), no live-media or hardware proof, no CP sign-offs. See the release
  runbook.
