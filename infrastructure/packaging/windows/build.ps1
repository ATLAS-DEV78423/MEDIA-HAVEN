#Requires -Version 5.1
<#
.SYNOPSIS
    Reproducible, unsigned, per-user Windows release build for MEDIA HAVEN.

.DESCRIPTION
    Verifies the Tauri config is shaped for O-01/O-08, builds the frontend and
    the NSIS installer with the already-installed local Tauri CLI, then writes
    SHA-256 checksums and bundles the notices/SBOM beside the artifact.

    Installs nothing, fetches nothing, requires no administrator rights and
    publishes nothing. All tools must already be present; missing tools are a
    hard FAIL, never an implicit install.

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File infrastructure\packaging\windows\build.ps1

.NOTES
    Exit codes: 0 success; 2 config precondition failed; 3 required tool missing;
    4 build/output failure. Native tool exit codes are propagated as 4.
#>

[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Write-Fail {
    param([string]$Message, [int]$Code)
    Write-Host "FAIL: $Message" -ForegroundColor Red
    exit $Code
}

function Write-Info {
    param([string]$Message)
    Write-Host "  $Message"
}

function Get-Prop {
    param($Object, [string]$Name)
    if ($null -eq $Object) { return $null }
    $property = $Object.PSObject.Properties[$Name]
    if ($property) { return $property.Value }
    return $null
}

$packagingDir = $PSScriptRoot
$repoRoot = (Resolve-Path (Join-Path $packagingDir '..\..\..')).Path
$desktopDir = Join-Path $repoRoot 'apps\desktop'
$configPath = Join-Path $desktopDir 'src-tauri\tauri.conf.json'
$expectedId = 'com.mediahaven.desktop'

Write-Host 'MEDIA HAVEN release build - unsigned, per-user (O-08)'
Write-Info "Repo: $repoRoot"

# --- 1. Config preconditions (this script never edits config) -----------------
if (-not (Test-Path $configPath)) {
    Write-Fail "Missing $configPath" 2
}
$config = Get-Content -Raw -Encoding UTF8 $configPath | ConvertFrom-Json

$identifier = Get-Prop $config 'identifier'
if ($identifier -ne $expectedId) {
    Write-Fail ("tauri.conf.json identifier is '{0}'; expected '{1}'. Controller must change it; this script may not edit config." -f $identifier, $expectedId) 2
}

$bundle = Get-Prop $config 'bundle'
if (-not $bundle -or (Get-Prop $bundle 'active') -ne $true) {
    Write-Fail 'tauri.conf.json bundle.active must be true to emit an installer. Controller must change it.' 2
}

$targets = Get-Prop $bundle 'targets'
if ($null -ne $targets -and $targets -is [array] -and $targets -notcontains 'nsis') {
    Write-Fail 'bundle.targets does not include nsis. This build emits an NSIS per-user installer only.' 2
}

$windows = Get-Prop $bundle 'windows'
$nsis = Get-Prop $windows 'nsis'
$installMode = Get-Prop $nsis 'installMode'
if ($null -ne $installMode -and $installMode -ne 'currentUser') {
    Write-Fail ("bundle.windows.nsis.installMode is '{0}'; per-user requires 'currentUser'." -f $installMode) 2
}

$signing = Get-Prop $windows 'signing'
$thumbprint = Get-Prop $windows 'certificateThumbprint'
if ($null -ne $signing -or $null -ne $thumbprint) {
    Write-Fail 'Windows signing is configured but O-08 ships unsigned in v1. Remove signing config or get a new human ruling.' 2
}

# Version sources disagree in this checkout. The bundle version comes from
# tauri.conf.json; warn instead of silently claiming a single source.
$cargoToml = Get-Content -Raw -Encoding UTF8 (Join-Path $repoRoot 'Cargo.toml')
$npmJson = Get-Content -Raw -Encoding UTF8 (Join-Path $desktopDir 'package.json') | ConvertFrom-Json
$workspaceVersion = [regex]::Match($cargoToml, '(?m)^version\s*=\s*"([^"]+)"').Groups[1].Value
Write-Info ("Version sources: tauri.conf.json={0}, Cargo.toml workspace={1}, package.json={2}" -f (Get-Prop $config 'version'), $workspaceVersion, (Get-Prop $npmJson 'version'))
Write-Info 'Bundle version is taken from tauri.conf.json. Align the three files (controller) before publishing.'

# --- 2. Tools must already exist ---------------------------------------------
foreach ($name in @('node', 'npm', 'npx')) {
    if (-not (Get-Command $name -ErrorAction SilentlyContinue)) {
        Write-Fail "Required tool '$name' is not installed. Install it with human consent, then rerun." 3
    }
}
if (-not (Test-Path (Join-Path $desktopDir 'node_modules'))) {
    Write-Fail 'apps/desktop/node_modules missing. Run npm ci only with approved network access, then rerun.' 3
}

Push-Location $desktopDir
try {
    $cliVersion = & npx --no-install tauri --version 2>$null
    if ($LASTEXITCODE -ne 0) {
        Write-Fail 'Local @tauri-apps/cli is unavailable (npx --no-install tauri --version failed). No tool was installed.' 3
    }
    Write-Info "Tauri CLI: $cliVersion"
}
finally {
    Pop-Location
}

# --- 3. Build -----------------------------------------------------------------
Push-Location $desktopDir
try {
    Write-Host '== npm run build =='
    & npm run build
    if ($LASTEXITCODE -ne 0) { Write-Fail 'npm run build failed.' 4 }

    # beforeBuildCommand also runs npm run build; harmless duplicate, kept so the
    # build command is explicit and reproducible when beforeBuildCommand changes.
    Write-Host '== npx --no-install tauri build --bundles nsis =='
    & npx --no-install tauri build --bundles nsis
    if ($LASTEXITCODE -ne 0) { Write-Fail 'tauri build failed.' 4 }
}
finally {
    Pop-Location
}

# --- 4. Checksums + bundled compliance files ---------------------------------
# This is a Cargo workspace, so Tauri emits bundles into the workspace-root
# target/, not the per-crate src-tauri/target/. Honour CARGO_TARGET_DIR if set.
$workspaceRoot = Split-Path -Parent (Split-Path -Parent $desktopDir)
$envTarget = $env:CARGO_TARGET_DIR
if ($envTarget) {
    $targetRoot = if ([IO.Path]::IsPathRooted($envTarget)) { $envTarget } else { Join-Path $workspaceRoot $envTarget }
} else {
    $targetRoot = Join-Path $workspaceRoot 'target'
}
$bundleDir = Join-Path $targetRoot 'release\bundle\nsis'
if (-not (Test-Path $bundleDir)) {
    # Fall back to the per-crate layout for non-workspace checkouts.
    $fallback = Join-Path $desktopDir 'src-tauri\target\release\bundle\nsis'
    if (Test-Path $fallback) {
        $bundleDir = $fallback
    } else {
        Write-Fail "Expected NSIS output directory not found: $bundleDir" 4
    }
}

$artifacts = @(Get-ChildItem -Path $bundleDir -File -Recurse |
    Where-Object { $_.Extension -in @('.exe', '.msi', '.zip', '.sig') })
if ($artifacts.Count -eq 0) {
    Write-Fail "No installer artifacts found under $bundleDir" 4
}

$lines = foreach ($artifact in $artifacts) {
    $relative = $artifact.FullName.Substring($bundleDir.Length).TrimStart('\', '/')
    $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $artifact.FullName).Hash.ToLowerInvariant()
    '{0}  {1}' -f $hash, $relative
}
$sumPath = Join-Path $bundleDir 'SHA256SUMS.txt'
$lines | Set-Content -Encoding ascii -LiteralPath $sumPath
Write-Info "Wrote $sumPath"

Copy-Item -LiteralPath (Join-Path $packagingDir 'THIRD_PARTY_NOTICES.md') `
    -Destination (Join-Path $bundleDir 'THIRD_PARTY_NOTICES.md') -Force

$sbomSource = Join-Path $packagingDir 'sbom\bom.json'
if (Test-Path $sbomSource) {
    Copy-Item -LiteralPath $sbomSource -Destination (Join-Path $bundleDir 'bom.json') -Force
    Write-Info 'Bundled sbom/bom.json'
}
else {
    Write-Host 'WARN: SBOM NOT RUN - no sbom/bom.json to bundle. See infrastructure/packaging/windows/sbom/README.md.' -ForegroundColor Yellow
}

Write-Host 'PASS: unsigned per-user NSIS build complete. No publish/release action was performed.' -ForegroundColor Green
Write-Info "Artifacts: $bundleDir"
Get-Content -LiteralPath $sumPath | ForEach-Object { Write-Info $_ }
