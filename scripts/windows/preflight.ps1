#Requires -Version 5.1
<#
.SYNOPSIS
    Read-only environment preflight for MEDIA_HAVEN.

.DESCRIPTION
    Reports the facts the first-run setup needs. This script CHANGES NOTHING:
    it installs nothing, enables no Windows feature, edits no firewall rule,
    and requires no administrator rights.

    It is the executable half of docs/setup/first-run.md and is exercised by
    roadmap.md P1-06.

.PARAMETER Json
    Emit the result as JSON instead of a formatted table. Intended for the app.

.PARAMETER MediaPath
    Optional. A folder to report free space for (usually a candidate library
    or managed-storage root). Read-only: the folder is stat'ed, not listed.

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/windows/preflight.ps1

.EXAMPLE
    .\preflight.ps1 -Json -MediaPath 'D:\Media'

.NOTES
    Security notes:
      - Every check is a read. If you add a check that writes, it does not
        belong in this script.
      - Output contains OS and hardware facts only. It must never include
        personal file paths beyond the one the user explicitly passes in.
#>

[CmdletBinding()]
param(
    [switch]$Json,
    [string]$MediaPath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

$script:results = New-Object System.Collections.Generic.List[object]

function Add-Check {
    param(
        [Parameter(Mandatory)][string]$Name,
        [Parameter(Mandatory)][string]$Value,
        # ready | recommend | attention | info
        [Parameter(Mandatory)][ValidateSet('ready', 'recommend', 'attention', 'info')][string]$Status,
        [string]$Detail = ''
    )

    $script:results.Add([pscustomobject]@{
            Check  = $Name
            Value  = $Value
            Status = $Status
            Detail = $Detail
        })
}

function Test-IsAdministrator {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

# ---------------------------------------------------------------------------
# Operating system
# ---------------------------------------------------------------------------

try {
    $os = Get-CimInstance -ClassName Win32_OperatingSystem -ErrorAction Stop
    $build = [int]([System.Environment]::OSVersion.Version.Build)

    Add-Check -Name 'Windows version' `
        -Value "$($os.Caption) $($os.Version)" `
        -Status 'info' `
        -Detail "Build $build"

    if ($build -ge 22000) {
        Add-Check -Name 'Windows 11' -Value 'Detected' -Status 'ready'
    }
    elseif ($build -ge 19041) {
        Add-Check -Name 'Windows 10' -Value 'Detected' -Status 'recommend' `
            -Detail 'Windows 10 support is not claimed until tested (roadmap O-02).'
    }
    else {
        Add-Check -Name 'Windows version' -Value 'Too old' -Status 'attention' `
            -Detail 'Windows 11 x64 is the supported platform. Local playback may still work but is unsupported.'
    }
}
catch {
    Add-Check -Name 'Windows version' -Value 'Detection failed' -Status 'attention' -Detail $_.Exception.Message
}

# ---------------------------------------------------------------------------
# Architecture
# ---------------------------------------------------------------------------

try {
    $arch = $env:PROCESSOR_ARCHITECTURE
    $isArm = $arch -eq 'ARM64'

    Add-Check -Name 'Architecture' -Value $arch -Status $(if ($isArm) { 'attention' } else { 'ready' }) `
        -Detail $(if ($isArm) { 'ARM64 is not claimed for v1 (see docs/testing/test-matrix.md).' } else { 'x64 is the primary target.' })
}
catch {
    Add-Check -Name 'Architecture' -Value 'Unknown' -Status 'attention'
}

# ---------------------------------------------------------------------------
# Memory
# ---------------------------------------------------------------------------

try {
    $cs = Get-CimInstance -ClassName Win32_ComputerSystem -ErrorAction Stop
    $ramGb = [math]::Round($cs.TotalPhysicalMemory / 1GB, 1)

    if ($ramGb -ge 16) {
        $status = 'ready'; $detail = 'Full MVP experience expected.'
    }
    elseif ($ramGb -ge 8) {
        $status = 'ready'; $detail = 'Local playback and library; optional services only after measurement.'
    }
    elseif ($ramGb -ge 4) {
        $status = 'recommend'; $detail = 'Basic local playback only. Leave optional services disabled.'
    }
    else {
        $status = 'attention'; $detail = 'Below the minimum experiment tier. Playback may not be reliable.'
    }

    Add-Check -Name 'Installed RAM' -Value "$ramGb GB" -Status $status -Detail $detail
}
catch {
    Add-Check -Name 'Installed RAM' -Value 'Unknown' -Status 'info' -Detail $_.Exception.Message
}

# ---------------------------------------------------------------------------
# CPU
# ---------------------------------------------------------------------------

try {
    $cpu = Get-CimInstance -ClassName Win32_Processor -ErrorAction Stop | Select-Object -First 1
    Add-Check -Name 'CPU' -Value "$($cpu.Name.Trim())" -Status 'info' `
        -Detail "$($cpu.NumberOfCores) cores / $($cpu.NumberOfLogicalProcessors) threads"
}
catch {
    Add-Check -Name 'CPU' -Value 'Unknown' -Status 'info'
}

# ---------------------------------------------------------------------------
# GPU (relevant to the playback engine decision, ADR-0002)
# ---------------------------------------------------------------------------

try {
    $gpus = Get-CimInstance -ClassName Win32_VideoController -ErrorAction Stop |
        Where-Object { $_.Name } |
        Select-Object -ExpandProperty Name -Unique

    if ($gpus) {
        Add-Check -Name 'GPU' -Value ($gpus -join '; ') -Status 'info' `
            -Detail 'Hardware decode support is verified during the P0-03 playback spike.'
    }
    else {
        Add-Check -Name 'GPU' -Value 'None detected' -Status 'recommend' -Detail 'Software decoding only; expect higher CPU during playback.'
    }
}
catch {
    Add-Check -Name 'GPU' -Value 'Unknown' -Status 'info'
}

# ---------------------------------------------------------------------------
# Disk
# ---------------------------------------------------------------------------

try {
    $systemDrive = $env:SystemDrive
    $sys = Get-PSDrive -Name $systemDrive.TrimEnd(':') -ErrorAction Stop
    $freeGb = [math]::Round($sys.Free / 1GB, 1)

    Add-Check -Name "Free space ($systemDrive)" -Value "$freeGb GB" `
        -Status $(if ($freeGb -lt 5) { 'attention' } elseif ($freeGb -lt 20) { 'recommend' } else { 'ready' }) `
        -Detail 'App data, database and cache live here by default.'
}
catch {
    Add-Check -Name 'Free space' -Value 'Unknown' -Status 'info'
}

if ($MediaPath) {
    # Read-only: the folder is stat'ed, not enumerated. Nothing is listed and no
    # filenames are emitted, so this output is safe to share.
    if (Test-Path -LiteralPath $MediaPath -PathType Container) {
        try {
            $driveName = (Split-Path -Qualifier $MediaPath).TrimEnd(':')
            $d = Get-PSDrive -Name $driveName -ErrorAction Stop
            Add-Check -Name 'Media folder free space' -Value "$([math]::Round($d.Free / 1GB, 1)) GB" -Status 'info' `
                -Detail 'Reported for the folder supplied on the command line.'
        }
        catch {
            Add-Check -Name 'Media folder free space' -Value 'Unavailable' -Status 'info'
        }
    }
    else {
        Add-Check -Name 'Media folder' -Value 'Not found' -Status 'attention' `
            -Detail 'The supplied path is not an existing directory.'
    }
}

# ---------------------------------------------------------------------------
# Virtualisation capability (informational only — never enabled here)
# ---------------------------------------------------------------------------

try {
    $cs = Get-CimInstance -ClassName Win32_ComputerSystem -ErrorAction Stop
    $firmware = $cs.HypervisorPresent

    Add-Check -Name 'Virtualisation capability' -Value $(if ($firmware) { 'Hypervisor present' } else { 'Not detected' }) `
        -Status 'info' `
        -Detail 'Only relevant if optional container services are enabled. This script does not enable it.'
}
catch {
    Add-Check -Name 'Virtualisation capability' -Value 'Unknown' -Status 'info'
}

# ---------------------------------------------------------------------------
# Existing container runtime (read-only detection — never modified)
# ---------------------------------------------------------------------------

try {
    $docker = Get-Command docker -ErrorAction SilentlyContinue
    if ($docker) {
        $version = 'unknown'
        try { $version = (& docker --version 2>$null) } catch { }
        Add-Check -Name 'Container runtime' -Value 'Detected' -Status 'info' `
            -Detail "$version. An existing installation is never modified."
    }
    else {
        Add-Check -Name 'Container runtime' -Value 'Not installed' -Status 'info' `
            -Detail 'Optional. The app runs in local-library mode without it.'
    }
}
catch {
    Add-Check -Name 'Container runtime' -Value 'Unknown' -Status 'info'
}

# ---------------------------------------------------------------------------
# WebView2 (Tauri runtime requirement)
# ---------------------------------------------------------------------------

try {
    $webviewKeys = @(
        'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}',
        'HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}',
        'HKCU:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
    )

    $found = $null
    foreach ($key in $webviewKeys) {
        if (Test-Path -LiteralPath $key) {
            $found = (Get-ItemProperty -LiteralPath $key -ErrorAction SilentlyContinue).pv
            if ($found) { break }
        }
    }

    if ($found) {
        Add-Check -Name 'WebView2 runtime' -Value "Installed ($found)" -Status 'ready'
    }
    else {
        Add-Check -Name 'WebView2 runtime' -Value 'Not detected' -Status 'attention' `
            -Detail 'Required by the desktop shell. Install from Microsoft''s official WebView2 page before running the app.'
    }
}
catch {
    Add-Check -Name 'WebView2 runtime' -Value 'Unknown' -Status 'info'
}

# ---------------------------------------------------------------------------
# Privilege check (we must run unelevated)
# ---------------------------------------------------------------------------

if (Test-IsAdministrator) {
    Add-Check -Name 'Elevation' -Value 'Running as administrator' -Status 'recommend' `
        -Detail 'The app never requires administrator rights. This preflight does not need them either.'
}
else {
    Add-Check -Name 'Elevation' -Value 'Standard user' -Status 'ready' `
        -Detail 'Correct: the app runs as the normal user.'
}

# ---------------------------------------------------------------------------
# Output
# ---------------------------------------------------------------------------

if ($Json) {
    [pscustomobject]@{
        generatedAtUtc = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
        readOnly       = $true
        checks         = $script:results
    } | ConvertTo-Json -Depth 4
}
else {
    Write-Host ''
    Write-Host '  MEDIA_HAVEN preflight (read-only)' -ForegroundColor Cyan
    Write-Host '  This script changes nothing on your machine.' -ForegroundColor DarkGray
    Write-Host ''

    $script:results | ForEach-Object {
        $colour = switch ($_.Status) {
            'ready' { 'Green' }
            'recommend' { 'Yellow' }
            'attention' { 'Red' }
            default { 'Gray' }
        }
        $label = $_.Status.PadRight(9).ToUpperInvariant()
        Write-Host ("  [{0}] {1,-28} {2}" -f $label, $_.Check, $_.Value) -ForegroundColor $colour
        if ($_.Detail) {
            Write-Host ("            {0}" -f $_.Detail) -ForegroundColor DarkGray
        }
    }

    $attention = @($script:results | Where-Object { $_.Status -eq 'attention' }).Count
    $recommend = @($script:results | Where-Object { $_.Status -eq 'recommend' }).Count

    Write-Host ''
    if ($attention -gt 0) {
        Write-Host "  $attention item(s) need attention, $recommend recommendation(s)." -ForegroundColor Yellow
    }
    elseif ($recommend -gt 0) {
        Write-Host "  Ready, with $recommend recommendation(s)." -ForegroundColor Green
    }
    else {
        Write-Host '  Ready.' -ForegroundColor Green
    }
    Write-Host ''
    Write-Host '  Nothing was installed or changed. No administrator rights were used.' -ForegroundColor DarkGray
    Write-Host ''
}
