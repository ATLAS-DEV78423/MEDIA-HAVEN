# SBOM - STATUS: NOT RUN

**No SBOM was generated.** Every SBOM-capable tool was checked with
`Get-Command` on 2026-11-20/21 and none is installed. Nothing was installed as
part of this task (no consent to install tooling).

## Tool probe (observed)

| Command | Result |
|---|---|
| `Get-Command cargo-sbom` | MISSING |
| `Get-Command cargo-cyclonedx` | MISSING |
| `Get-Command syft` | MISSING |
| `Get-Command cyclonedx` | MISSING |
| `cargo --list` | no sbom/cyclonedx subcommand |
| `Get-Command @cyclonedx/cyclonedx-npm` | not applicable (npm package) |
| `apps/desktop/node_modules/@cyclonedx` | absent (local install not present) |

`cargo metadata --offline --format-version 1` also fails: it attempts to
download `android_system_properties v0.1.6`. Any network fetch needs human
consent under the repository contract, so no fetch was attempted.

## Commands a human must run (with consent)

Pick **one** tool. Verify its flags with `--help` on the installed version
before relying on the output; these are the intended invocations, not commands
that were executed by this task.

**Option A - Syft (covers Rust + npm from source tree):**

```powershell
# Requires an approved install of syft first (e.g. winget/scoop, with consent).
syft dir:. -o cyclonedx-json=infrastructure/packaging/windows/sbom/bom.json
Get-FileHash -Algorithm SHA256 infrastructure/packaging/windows/sbom/bom.json
```

**Option B - cargo-cyclonedx for the Rust workspace:**

```powershell
# Requires cargo install cargo-cyclonedx --locked (approved network + install).
cargo cyclonedx --manifest-path Cargo.toml --format json `
  --output infrastructure/packaging/windows/sbom/bom-rust.json
```

**Option C - CycloneDX npm for the frontend:**

```powershell
# Requires approved network/install; npx fetches if the package is not local.
Push-Location apps/desktop
npx --yes @cyclonedx/cyclonedx-npm `
  --output-file ../../infrastructure/packaging/windows/sbom/bom-npm.json
Pop-Location
```

If A and one of B/C are used together, keep both files and list both in the
release checksum file; do not merge formats by hand.

## What exists instead

`../THIRD_PARTY_NOTICES.md` is a **licence inventory**, not a SBOM. It lists
component names, versions and declared licences from `Cargo.lock` and
`apps/desktop/package-lock.json`. It is not a CycloneDX/SPDX document, has no
dependency graph, and does not satisfy a SBOM requirement. Do not rename or
pass it off as one.

## Gate status

SBOM gate: **NOT RUN** - blocks a complete release until one of the commands
above is executed with consent and the resulting file is checksummed and
attached.

`infrastructure/packaging/windows/build.ps1` copies `sbom/bom.json` into the
artifact directory **if it exists**; otherwise it prints an explicit SBOM
warning. It never fabricates an SBOM.
