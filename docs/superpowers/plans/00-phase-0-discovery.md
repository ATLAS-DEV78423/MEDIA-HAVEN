# Phase 0 — Discovery and Technical Validation: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (or superpowers:subagent-driven-development) to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace every architectural assumption that could invalidate the stack with recorded evidence, ending in a passed CP-0.

**Architecture:** Spikes only. Code written here is throwaway and lives under `spikes/` (gitignored). The deliverables are measurements and accepted ADRs, not product code.

**Tech Stack:** Rust (stable), Tauri 2, Vite, React 18, TypeScript, PowerShell 5.1, Docker Engine / Docker Compose (detection and measurement only — the runtime distribution itself is still **Proposed**, ADR-0001 Part 2).

**Spec:** `docs/superpowers/specs/2026-11-15-media-haven-design.md`

## Global Constraints

- Windows 11 x64 is the only supported platform for this phase.
- No host change without explicit consent: no installs, no WSL2/Hyper-V enablement, no admin rights, no firewall edits.
- `spikes/` is gitignored. Do not promote spike code into `apps/` or `crates/` without re-implementing it under test.
- Every measurement is recorded in `progress.md` with the exact machine (CPU, RAM, GPU, Windows build).
- `NOT RUN` is never recorded as `PASS`.
- No copyrighted media is committed. `tests/test-media/` stays local.
- **A measurement is not a proposal.** CP-0 does not pass because a document says
  "recommended" — it passes because an observed value is recorded here.

---

## Task 1: Confirm the MVP boundary (P0-01)

**Files:**
- Create: `docs/decisions/0004-local-playback-first.md` (replace the placeholder)

**Interfaces:**
- Produces: ADR-0004 marked Accepted, referenced by `roadmap.md` P0-01 and CP-0.

- [ ] **Step 1: Draft the decision**

Replace the `## Decision` placeholder in `docs/decisions/0004-local-playback-first.md` with:

```markdown
## Decision

The first release is a local HDMI appliance. Playback, library browsing, queue
and settings operate on the machine connected to the TV. Remote, LAN and
headless streaming are out of scope for v1 and require a new spec, a new threat
model entry and a new ADR before any implementation.
```

- [ ] **Step 2: Present to the human partner**

Show the drafted ADR. Ask explicitly: "Accept ADR-0004 as written, or change the scope?"

- [ ] **Step 3: Record acceptance**

Only after an explicit yes, set the status line to `Accepted` and add the date. If the answer is no, revise and repeat Step 2. **Do not set `Accepted` yourself.**

- [ ] **Step 4: Verify**

Run: `grep -n "Status:" docs/decisions/0004-local-playback-first.md`
Expected: `Accepted`

- [ ] **Step 5: Update tracking**

Set `P0-01` to `[x]` in `roadmap.md` and record the acceptance in `progress.md` under "Decisions made".

---

## Task 2: Tauri 2 proof of concept (P0-02)

**Files:**
- Create: `spikes/tauri-poc/` (entire scaffold, gitignored)

**Interfaces:**
- Produces: measured cold-start time; confirmed pinned versions of `tauri`, `tauri-cli`, `vite`, `react`; a known-working `npm run tauri dev` command.

- [ ] **Step 1: Scaffold the spike**

Run:

```powershell
cd spikes
npm create tauri-app@latest tauri-poc -- --template react-ts --manager npm
cd tauri-poc
npm install
```

Expected: project created, `npm install` exits 0.

- [ ] **Step 2: Pin the toolchain**

Record exact versions:

```powershell
npm ls @tauri-apps/api @tauri-apps/cli react vite --depth=0
rustc --version; cargo --version
```

Expected: all versions printed. Copy them into `progress.md` under "Current implementation facts".

- [ ] **Step 3: Add one IPC command**

In `spikes/tauri-poc/src-tauri/src/lib.rs`, add:

```rust
#[tauri::command]
fn get_app_info() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "app_version": env!("CARGO_PKG_VERSION"),
    }))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_app_info])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Ensure `serde_json` is in `src-tauri/Cargo.toml`:

```toml
[dependencies]
serde_json = "1"
```

- [ ] **Step 4: Render the result**

In `spikes/tauri-poc/src/App.tsx`, replace the body with:

```tsx
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

function App() {
  const [info, setInfo] = useState<Record<string, string> | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<Record<string, string>>("get_app_info")
      .then(setInfo)
      .catch((e) => setError(String(e)));
  }, []);

  return (
    <main style={{ fontFamily: "system-ui", padding: 24 }}>
      <h1>MEDIA_HAVEN spike</h1>
      {error && <p role="alert">IPC failed: {error}</p>}
      {info && <pre>{JSON.stringify(info, null, 2)}</pre>}
    </main>
  );
}

export default App;
```

- [ ] **Step 5: Run and verify**

Run: `npm run tauri dev`

Expected: window opens titled with the app name, showing `os: "windows"`, `arch: "x86_64"` and a version. No console error.

- [ ] **Step 6: Measure cold start**

Close the app. Start it. Record wall-clock time from launch command to first paint, three times; record the median.

- [ ] **Step 7: Verify clean exit**

With the app running:

```powershell
Get-Process | Where-Object { $_.ProcessName -match 'tauri|msedgewebview2' } | Select-Object ProcessName, Id
```

Close the window normally, repeat the command.

Expected: no app-owned processes remain. If WebView2 helpers linger, record it — it affects `process-model.md`.

- [ ] **Step 8: Record results and update tracking**

Add a "P0-02 results" block to `progress.md`: versions, median cold start, process-leak finding. Mark `P0-02` `[x]` only when Steps 5–7 passed.

---

## Task 3: Playback engine comparison (P0-03)

**Files:**
- Create: `spikes/player-comparison/`
- Create: `docs/decisions/0002-playback-engine.md` (fill Decision + Evidence)
- Reference: `tests/test-media/README.md` (obtain the fixture set)

**Interfaces:**
- Produces: a completed codec matrix; a hardware-decode finding per GPU present; an accepted ADR-0002 naming one engine and rejecting the others.

- [ ] **Step 1: Assemble the fixture set**

Place freely licensed or generated files in `tests/test-media/` covering: MP4/H.264/AAC, MKV/H.264/AC3, MKV/HEVC/EAC3, WebM/VP9/Opus, MKV/AV1/FLAC, plus SRT and ASS subtitle files.

Verify: `Get-ChildItem tests/test-media -File | Select-Object Name, Length`
Expected: at least six media files plus two subtitles. **No personal media.**

- [ ] **Step 2: Build the baseline (HTML5 `<video>`)**

In `spikes/player-comparison/html5/`, serve a minimal page that plays a selected file in a `<video>` element with a subtitle track. Test each fixture.

Record per file: plays (yes/no), audio track present, subtitle rendering, error message.

- [ ] **Step 3: Build the native path candidate**

In `spikes/player-comparison/native/`, use Windows Media Foundation through a minimal Rust binary or a Tauri window hosting a media element. Test the same fixtures.

Record the same fields, plus hardware-decode status.

- [ ] **Step 4: Build the libmpv candidate**

In `spikes/player-comparison/mpv/`, embed libmpv (or drive an mpv child process for the spike). Test the same fixtures with `--hwdec=auto` and without it.

Record the same fields, plus ASS subtitle positioning behaviour.

- [ ] **Step 5: Measure resources**

For each candidate that plays a 1080p H.264 file, sample during a 10-minute run:

```powershell
1..10 | ForEach-Object {
  Get-Process -Name <candidate-process> -ErrorAction SilentlyContinue |
    Select-Object @{n='RAM_MB';e={[math]::Round($_.WorkingSet64/1MB,1)}}, @{n='CPU_s';e={[math]::Round($_.CPU,1)}}
  Start-Sleep -Seconds 60
}
```

Record peak and steady-state RAM and average CPU.

- [ ] **Step 6: Capture licence obligations**

For each candidate, record the licence name, whether redistribution requires source disclosure, and whether attribution is required. Copy the relevant clause text into the ADR — do not paraphrase from memory.

- [ ] **Step 7: Fill the matrix and write the recommendation**

In `docs/decisions/0002-playback-engine.md`:

- Complete every `[ ]` in the Evidence section with measured values.
- Write the Decision paragraph naming the engine.
- Add the rejected candidates with the specific reason each was rejected.
- Mark `UNTESTED` for any GPU family or codec not actually tested.

- [ ] **Step 8: Get acceptance**

Present the matrix and recommendation. Ask: "Accept ADR-0002 with `<engine>`?"

Only after an explicit yes, set `Status: Accepted`. If the human disagrees, record their choice and its rationale — the ADR must reflect the real decision.

- [ ] **Step 9: Update tracking**

Mark `P0-03` `[x]`, and record the matrix summary in `progress.md`.

---

## Task 4: Resource benchmark (P0-04)

**Files:**
- Create: `docs/testing/resource-baseline.md`
- Modify: `docs/testing/test-matrix.md` (hardware matrix status column)

**Interfaces:**
- Consumes: the winning engine from Task 3 and the shell from Task 2.
- Produces: measured numbers that confirm, amend or invalidate the tiers in `Development.md` §9.1.

- [ ] **Step 1: Measure idle**

Launch the app, wait 5 minutes without interaction, then sample:

```powershell
Get-Process -Name <app-process> | Select-Object ProcessName, Id,
  @{n='RAM_MB';e={[math]::Round($_.WorkingSet64/1MB,1)}}, CPU
```

Repeat 3 times, 2 minutes apart. Record all three values and the median.

- [ ] **Step 2: Measure startup**

Record launch-to-first-paint 5 times. Report median and worst case.

- [ ] **Step 3: Measure playback**

Play a 1080p file for 10 minutes. Sample RAM and CPU every 60 s. Record peak and steady state.

- [ ] **Step 4: Measure disk footprint**

```powershell
$p = "$env:LOCALAPPDATA\<AppName>"
if (Test-Path $p) { "{0:N1} MB" -f ((Get-ChildItem $p -Recurse -File | Measure-Object Length -Sum).Sum/1MB) }
```

Record app data size after a short library import.

- [ ] **Step 5: Write the baseline document**

Create `docs/testing/resource-baseline.md` with one row per measured profile, each stating CPU, RAM, GPU, Windows build, and every measurement above. Mark untested tiers `UNTESTED` with the reason.

- [ ] **Step 6: Reconcile with the tiers**

Compare against `Development.md` §9.1. For each tier, write one of: `Confirmed`, `Amended to <value>` (with evidence), or `Unproven` (no machine available).

- [ ] **Step 7: Get sign-off and update tracking**

Show the human partner the reconciliation. Mark `P0-04` `[x]` only after they accept the tier wording. Record in `progress.md`.

---

## Task 5: Container runtime validation (P0-05)

**Files:**
- Create: `docs/decisions/0001-windows-container-runtime.md` (fill Decision + Evidence)
- Create: `docs/setup/container-runtime.md`

**Interfaces:**
- Produces: the exact consent wording; measured idle cost; a behaviour matrix for absent/stopped/running/unhealthy.

- [ ] **Step 1: Document the clean-machine procedure** *(before installing anything)*

Write `docs/setup/container-runtime.md` listing every step, every Windows optional feature that will change, whether a reboot is needed, and every prompt the user will see.

- [ ] **Step 2: Capture the before-state**

```powershell
Get-WindowsOptionalFeature -Online |
  Where-Object { $_.FeatureName -match 'Hyper-V|VirtualMachinePlatform|Windows-Subsystem-Linux' } |
  Select-Object FeatureName, State | Export-Csv -NoTypeInformation docs/testing/host-state-before.csv
```

- [ ] **Step 3: Obtain explicit consent, then install**

Show the consent text. **Do not proceed without an explicit yes.** Then install Docker Desktop on the clean machine per Step 1.

- [ ] **Step 4: Measure idle cost**

With the runtime running and nothing else active, sample RAM for 5 minutes. Record the runtime's total footprint (process tree, not one process).

- [ ] **Step 5: Test the behaviour matrix**

Record observed behaviour for each: absent (uninstalled or service stopped), installed-stopped, installed-running, unhealthy (service failing). For each, state what the app can still do.

- [ ] **Step 6: Test uninstall cleanliness**

Capture the after-state with the same command as Step 2, into `host-state-after.csv`. Compare. Record leftover features, services, network adapters and disk residue.

- [ ] **Step 7: Capture the licence position**

Record the licence terms that apply to the intended user population, quoting the relevant text. If it depends on organisation size or revenue, state the threshold and mark it an open decision for the human partner.

- [ ] **Step 8: Write the ADR and get acceptance**

Fill `ADR-0001` Decision and Evidence. Present. Set `Accepted` only after explicit approval.

- [ ] **Step 9: Update tracking**

Mark `P0-05` `[x]`. Record measurements and the consent wording location in `progress.md`.

---

## Task 6: Select the supported runtime distribution (P0-06)

**Files:**
- Modify: `docs/decisions/0001-windows-container-runtime.md` (Part 2 status + Evidence)
- Modify: `roadmap.md` (Phase 6 status)

**Interfaces:**
- Consumes: Task 4, Task 5 and Task 10 (P0-10) numbers.
- Produces: a definitive recommended Windows runtime distribution for the mandatory
  boundary, with evidence and human review.

> **Docker's mandatory-v1 status is NOT reopened in this task.** It was decided by the
> product owner (ADR-0001 Part 1). The only question here is *which* distribution hosts
> the containers.

- [ ] **Step 1: Build the comparison**

Write a two-column comparison in the ADR: "Docker Desktop (GUI closed)" vs "Docker Engine in a dedicated WSL2 distro", using the measured idle RAM, disk and install friction from the measurement tasks, against the licence position, the isolation argument and the maintenance cost.

- [ ] **Step 2: Present and decide**

Ask the human partner to choose. Record their choice verbatim and the reasoning. If they choose Docker Desktop, record it honestly — including the licence cliff and the shared-daemon isolation cost — rather than re-arguing the recommendation.

- [ ] **Step 3: Apply the consequence**

Update ADR-0001 Part 2's status to Accepted with the human's name and the date. Update `roadmap.md` O-06 and O-07 accordingly, and record the decision in `progress.md`.

- [ ] **Step 4: Verify and update tracking**

Run: `grep -n "P0-06" roadmap.md`
Expected: the task shows `[x]` with the decision recorded.

---

## Task 7: Threat model review (P0-07)

**Files:**
- Modify: `docs/security/threat-model.md` (review log, residual risks)

**Interfaces:**
- Produces: a human-reviewed threat model, and confirmation that no `Development.md` §18 decision was silently resolved.

- [ ] **Step 1: Cross-check the open decisions**

For each of the 10 items in `Development.md` §18, confirm it appears either as an accepted ADR or as `[?]` in `roadmap.md`.

Run: `grep -nE "O-[0-9]{2}" roadmap.md`
Expected: every open decision listed.

- [ ] **Step 2: Challenge the model**

Walk each abuse case A1–A18 in `docs/security/threat-model.md` and ask: is the mitigation actually planned, or aspirational? Fix any that name a feature that will not exist.

- [ ] **Step 3: Get human review**

Show the threat model to the human partner. Add a row to the review log with the date and outcome.

- [ ] **Step 4: Update tracking**

Mark `P0-07` `[x]` only after the review-log row exists with a human's name or role.

---

## Task 8: Navigation prototype (P0-08)

**Files:**
- Create: `spikes/nav-prototype/` (React, mock data)
- Create: `docs/design/navigation-findings.md`

**Interfaces:**
- Produces: a proven focus model, recorded findings that inform `docs/design/navigation-spec.md`.

- [ ] **Step 1: Build a shelf prototype with mock data**

Three shelves of 8/3/11 cards (deliberately unequal lengths). No backend. Cards render a coloured block and a title.

- [ ] **Step 2: Implement directional focus**

Left/Right within a shelf; Up/Down between shelves preserving the nearest column; clamp when the target shelf is shorter.

- [ ] **Step 3: Implement an overlay**

Pressing Enter on a card opens a dialog. Escape closes it and restores focus to that card.

- [ ] **Step 4: Verify keyboard-only traversal**

Navigate all three shelves, open and close overlays, using only arrows + Enter + Escape.

Expected: focus always visible; no unreachable card; after closing the overlay focus is on the originating card.

- [ ] **Step 5: Verify no reflow on focus**

Record the bounding box of a neighbouring card before and after focusing its neighbour:

```js
// paste in the console before and after focusing
JSON.stringify(document.querySelectorAll('[data-card]')[1].getBoundingClientRect())
```

Expected: identical `x`, `y`, `width`, `height`. If it shifts, the implementation is animating layout, not transform.

- [ ] **Step 6: Verify reduced motion**

Toggle OS reduced motion, reload. Expected: no scale/overshoot animation; focus ring still visible.

- [ ] **Step 7: Record findings**

Create `docs/design/navigation-findings.md` with: what worked, what did not, the exact technique used for "preserve nearest column", and any change needed in `navigation-spec.md`.

- [ ] **Step 8: Update tracking**

Mark `P0-08` `[x]`. Demo the walkthrough to the human partner.

---

## Task 9: Write the Phase 1 plan (P0-09)

**Files:**
- Modify: `docs/superpowers/plans/01-phase-1-foundation.md`

**Interfaces:**
- Consumes: accepted ADRs from Tasks 1, 3, 5, 6.
- Produces: a Phase 1 plan that does not contradict any accepted decision.

- [ ] **Step 1: Re-read the accepted ADRs**

Run: `grep -l "Status: Accepted" docs/decisions/*.md`
Expected: at least ADR-0003 and whichever of 0001/0002/0004 were accepted.

- [ ] **Step 2: Reconcile the existing Phase 1 plan**

Open `docs/superpowers/plans/01-phase-1-foundation.md`. For each task, confirm the pinned versions, commands and file paths match what the spikes proved. Fix any drift.

- [ ] **Step 3: Record the reconciliation**

Add a "Derived from P0 evidence" note at the top of the Phase 1 plan listing the versions and ADRs it assumes.

- [ ] **Step 4: Update tracking**

Mark `P0-09` `[x]`.

---

## Task 10: Measure the cost of each runtime distribution (P0-10)

**Files:**
- Create: `docs/testing/runtime-cost.md`

**Interfaces:**
- Produces: measured idle/active RAM, CPU, disk and cold-start for both candidate
  distributions, plus the shared-engine comparison.

- [ ] **Step 1: Confirm consent**

The measurement needs a working engine for each candidate. If the machine does not
already have one, ask first; do not install without explicit consent.

- [ ] **Step 2: Measure each candidate**

For Docker Desktop (GUI closed) and for Docker Engine in a dedicated WSL2 distro:
sample the **whole process tree** for 5 minutes idle, then again with `core` and
`probe` running. Record RAM, CPU and disk. Repeat 3 times; report medians.

```powershell
Get-Process | Where-Object { $_.ProcessName -match 'docker|com\.docker|vmmem|wsl' } |
  Measure-Object WorkingSet64 -Sum |
  Select-Object @{n='RAM_MB';e={[math]::Round($_.Sum/1MB,1)}}
```

- [ ] **Step 3: Measure the shared-engine case**

With an existing shared engine already running other workloads, record the total VM
budget (`docker info` → `MemTotal`) and what our containers are actually given. This
is the R-16 evidence.

- [ ] **Step 4: Write the comparison**

Create `docs/testing/runtime-cost.md` and paste the measured numbers into ADR-0001's
comparison table, replacing the qualitative cells. Mark anything not measured as
UNTESTED.

- [ ] **Step 5: Update tracking**

Mark `P0-10` `[x]` and record the numbers in `progress.md`.

---

## Task 11: Attempt hardware video decode inside a container (P0-11)

**Files:**
- Modify: `docs/decisions/0002-playback-engine.md` (Evidence)

**Interfaces:**
- Produces: an observed record of what containerised hardware decode actually does
  on this machine.

- [ ] **Step 1: Record the machine's GPU**

```powershell
Get-CimInstance Win32_VideoController | Select-Object Name, DriverVersion
```

Expected: the exact GPU model. If it is Intel, `nvdec` is unavailable for hardware
reasons — record that as a hardware-availability observation, not as proof that
`nvdec` would fail on an NVIDIA machine.

- [ ] **Step 2: Attempt `vaapi`**

Run a container and try `hwdec=vaapi`. Record the exact error.

- [ ] **Step 3: Attempt `nvdec`** *(only if an NVIDIA GPU is present)*

Record whether the NVIDIA Container Toolkit is installed and what the result is. If
there is no NVIDIA GPU, record that fact and mark the result UNTESTED.

- [ ] **Step 4: Attempt `vulkan` video decode**

Record the result. This is the only cross-vendor candidate that could move decoding
into a container, so it gets an honest attempt.

- [ ] **Step 5: Record display and audio availability**

Note that the container has no display server and no Windows audio endpoint, and
whether audio could be routed out at all.

- [ ] **Step 6: Update the ADR and track**

Write the observations into ADR-0002's Evidence section, correcting any claim the
experiment contradicts. Mark `P0-11` `[x]`.

---

## Task 12: Record the native codec/GPU/subtitle/audio matrix (P0-12)

**Files:**
- Create: `docs/testing/codec-matrix.md`
- Modify: `docs/decisions/0002-playback-engine.md` (Evidence + Decision)

**Interfaces:**
- Produces: a measured codec matrix and a confirmed licence for the exact libmpv
  build used.

- [ ] **Step 1: Assemble the fixture set**

Cover MP4/H.264/AAC, MKV/H.264/AC3, MKV/HEVC/EAC3, WebM/VP9/Opus, MKV/AV1/FLAC, plus
SRT and ASS subtitles. All freely licensed or generated. No personal media.

- [ ] **Step 2: Run each fixture with the leading candidate**

libmpv, native Windows build, `--vo=gpu-next --gpu-context=d3d11 --hwdec=d3d11va`.

- [ ] **Step 3: Record hardware decode honestly**

Use GPU engine counters or the player's own hwdec reporting — not "it looked smooth".
Record the software-decode fallback explicitly for any format that cannot use the
GPU.

- [ ] **Step 4: Record subtitles and audio**

ASS positioning, audio-track switching, and whether passthrough engaged.

- [ ] **Step 5: Confirm the licence (O-09)**

Determine the licence of the **exact build** used — from its build configuration, not
from memory. mpv is GPLv2+ by default and LGPLv2.1+ only when built with
`-Dgpl=false` / `--enable-lgpl`. Record the finding and whether it is compatible with
the intended project licence. **This blocks CP-0.**

- [ ] **Step 6: Update the ADR and track**

Complete ADR-0002's Evidence section and write the Decision. Mark `P0-12` `[x]`.

---

## Task 13: Prove host reachability and LAN non-reachability (P0-13)

**Files:**
- Modify: `docs/architecture/docker-workspace.md` (§3.1)

**Interfaces:**
- Produces: the observed answer to "can anything but this Windows host reach the API?"

- [ ] **Step 1: Determine the WSL2 networking mode**

```powershell
Get-Content "$env:USERPROFILE\.wslconfig" -ErrorAction SilentlyContinue
```

If `networkingMode=mirrored` is set, record it as a **blocking** condition (R-18) and
stop — do not work around it.

- [ ] **Step 2: Start the workspace and reach the port from the host**

```powershell
Invoke-WebRequest -Uri "http://127.0.0.1:<port>/healthz" -UseBasicParsing
```

Record the observed result.

- [ ] **Step 3: Prove the LAN cannot reach it**

From a second machine on the same network, attempt the same URL. Record the observed
result.

- [ ] **Step 4: Check for port forwarding and stray listeners**

```powershell
netsh interface portproxy show all
Get-NetTCPConnection -State Listen | Where-Object { $_.LocalPort -in 2375,2376 }
```

- [ ] **Step 5: Confirm `probe` is gateway-free**

Inspect its network attachment and confirm it sits only on the `internal` network.

- [ ] **Step 6: Write the result into the workspace doc**

Replace §3.1's "not decided" text with the observed values. Mark `P0-13` `[x]`.

---

## Task 14: Baseline the total added footprint of the required workspace (P0-14)

**Files:**
- Create: `docs/testing/resource-baseline.md`

**Interfaces:**
- Produces: the measured cost of the mandatory boundary, and the reconciled hardware
  tiers.

- [ ] **Step 1: Measure the stack in four states**

WSL2 VM with engine only → `core` running → `core` + `probe` running → the above with
1080p playback active. Record RAM, CPU, disk and startup latency at each step.

- [ ] **Step 2: Repeat and report medians**

Three runs, median reported, variance noted.

- [ ] **Step 3: Reconcile the tiers**

Against `Development.md` §9.1, write one of `Confirmed`, `Amended to <value>` (with
evidence) or `Unproven` for each tier. The 4 GB tier is expected to be `Unproven` or
`Not a support target`.

- [ ] **Step 4: Update the workspace doc and track**

Paste the figures into `docs/architecture/docker-workspace.md` §5.1. Mark `P0-14`
`[x]`.

---

## Task 15: Pass CP-0

- [ ] **Step 1: Verify every evidence item**

Open `roadmap.md` CP-0 and check each box only where evidence exists in `progress.md`.

- [ ] **Step 2: Produce the checkpoint summary**

Write into `progress.md` under "Current state":

- Playback engine: `<name>` (ADR-0002), placement `<host/container>`, licence `<LGPL/GPL>`
- Runtime decision: `<distribution>` (ADR-0001 Part 2)
- Containerised decode result: `<observed>` (P0-11)
- Resource numbers: idle `<n>` MB, playback `<n>` MB on `<machine>` (P0-10, P0-14)
- Networking result: host reaches `<yes/no>`, LAN reaches `<no>`, mode `<NAT>` (P0-13)
- Navigation: keyboard walkthrough demonstrated on `<date>`

- [ ] **Step 3: Get human sign-off**

Show the summary. CP-0 passes only when the human partner confirms it.

If CP-0 cannot pass, record precisely which item failed and what would resolve it. **Do not start Phase 1.**
