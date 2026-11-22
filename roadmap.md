# Roadmap

> **Sequencing document for MEDIA_HAVEN.** Outcomes, dependencies, acceptance
> criteria and status only. It is not a feature wish list and not a transcript.
> Authoritative engineering contract: `Development.md`. Implementation state:
> `progress.md`.

## Status legend

- [ ] Not started
- [~] In progress
- [x] Complete
- [!] Blocked
- [?] Needs decision

## Checkpoint model

Every phase ends in a **checkpoint (CP)**. A checkpoint is not a status flag; it
is a hard gate with recorded evidence.

A checkpoint passes only when, for that phase:

1. Every task is `[x]` or explicitly moved to `Deferred / future` with a reason.
2. Every acceptance criterion has been **observed**, not assumed.
3. The phase's listed tests have actually been run, and the exact outcomes are
   recorded in `progress.md` (PASS / FAIL / NOT RUN — never a guess).
4. No item in `Risks and open decisions` that blocks the next phase is
   unresolved.
5. `progress.md` is updated and a human has read the checkpoint summary.

Checkpoints are cumulative. If a later change invalidates an earlier checkpoint
(for example, the playback engine is swapped), the earlier checkpoint's evidence
must be re-run and re-recorded. Do not carry forward stale PASS results.

### Checkpoint list

| CP | Name | Phase | Gate type |
|----|------|-------|-----------|
| CP-0 | Assumptions proven | 0 — Discovery | Blocking: stack viability |
| CP-1 | It builds, it logs, it fails safely | 1 — Foundation | Blocking: toolchain |
| CP-2 | The library is real | 2 — Core local library | Blocking: data correctness |
| CP-3 | Video plays and remembers | 3 — Playback | Blocking: core experience |
| CP-4 | Usable from the couch | 4 — TV experience | Blocking: product usability |
| CP-5 | Trustworthy under failure | 5 — Reliability and security | Blocking: data safety |
| CP-6 | Required workspace is contained | 6 — Required Docker workspace | Blocking: boundary, lifecycle and data safety |
| CP-7 | Shippable | 7 — Packaging and release | Blocking: release gates |

Phases 0–7 are required for the first release. Docker/Compose is mandatory in v1 for the backend and media-inspection boundary; other services remain optional and require separate approval.

### Execution order

Phase **numbers** are thematic (what kind of work); the **execution order** is set here
and is not always numeric.

| Order | Phase | Why here |
|---|---|---|
| 1 | Phase 0 — Discovery | Evidence before anything is built |
| 2 | Phase 1 — Foundation | Toolchain, CI, logging, config, preflight, the container boundary |
| 3 | **Phase 4 — TV experience (frontend)** | **The product owner chose frontend-first.** The interface is built against a frozen command contract with a development-only mock (ADR-0009), so it can be reviewed while it is still cheap to change |
| 4 | Phase 2 — Library (backend) | The library, scanner and database behind the contract the frontend already expects |
| 5 | Phase 3 — Playback (backend) | The engine, resume state and completion rules |
| 6 | Phase 5 — Reliability and security | Failure, storage policy, threat tests |
| 7 | Phase 6 — Workspace hardening | Hardens and verifies what P1-07/P1-08 established |
| 8 | Phase 7 — Packaging and release | Installer, update, uninstall |

**What this changes and what it does not.** The frontend is built first; the container
boundary, the command contract and the backend authority are unchanged. Acceptance
criteria that need real data stay **open** until Phases 2–3 land — they are recorded as
blocked, not passed. See ADR-0009.

---

## Current milestone

- **Phase:** No checkpoint has passed. Substantial code now exists across Phases 2
  (library, metadata), 3 (playback), 4 (frontend, keyboard navigation), 6
  (acquisition + fail-closed media gates) and 7 (an unsigned installer was built),
  but every checkpoint remains OPEN pending observed evidence and human review.
- **Goal:** Convert shipped-but-unverified work into checkpoint evidence. The
  newest code landed in the 2026-11-20/21 waves: authorized-media torrent
  acquisition, fail-closed probe + Defender gates, R-01 per-item source mapping,
  offline IMDb metadata, a Windows installer and full keyboard navigation. A TMDB
  artwork plan exists but is unexecuted (P6-07).
- **Exit criteria:** the next checkpoint reached passes only on observed evidence
  recorded in `progress.md`, including a human-readable checkpoint summary.
- **Owner/agent:** unassigned. Any agent picking this up must first read
  `Development.md`, this file and `progress.md`, then confirm scope with the
  human partner.
- **Note:** CP-0 is still OPEN. Zero of its evidence items are satisfied — every
  one requires a measurement or a human sign-off, not a document.
- **Note:** a `[~]` marker means code or partial evidence exists, NOT that the
  acceptance criteria are met. `[x]` is reserved for checkpoint-verified tasks.
- **Evidence:** `progress.md` records the latest gates at HEAD `342addf`
  (documents only). The last code-verification wave was `9b706fb` — see the
  2026-11-21 entry in `progress.md`. Live engine, hardware, IMDb/OMDb, TMDB and
  second-machine gates remain **NOT RUN**.

---

# Phase 0 — Discovery and technical validation

Purpose: buy down the risks that can invalidate the mandatory architecture — playback, low-memory behaviour, Windows host lifecycle, runtime distribution, and host/container networking and isolation.

### [ ] P0-01 — Confirm the MVP boundary

- **Goal:** A written, human-approved statement that the first release is a
  local HDMI appliance, not a streaming server.
- **Scope:** Restate MVP must-haves and non-goals from `Development.md` §1.4,
  §1.5 and §20 into a short decision record.
- **Out of scope:** Any implementation. Any new feature proposal.
- **Dependencies:** None.
- **Implementation areas:** `docs/decisions/`.
- **Acceptance criteria:**
  - [ ] An ADR records "local HDMI playback first; remote streaming deferred".
  - [ ] The ADR is marked Accepted by the human partner, not by the agent.
  - [ ] `Development.md` §18 open decision 9 (remote access) is referenced.
- **Tests:** None (documentation gate). Reviewed by human.
- **Risks:** Scope creep into headless/remote mode, which multiplies auth, TLS
  and network-exposure requirements.
- **Status:** Not started

### [ ] P0-02 — Tauri 2 + React + Rust proof of concept

- **Goal:** Prove the shell works on Windows 11 x64: window opens, React
  renders, one typed IPC command round-trips, app exits cleanly.
- **Scope:** Throwaway spike in `spikes/tauri-poc/`. One command
  (`get_app_info`) returning OS version and app version. No database, no
  library, no player.
- **Out of scope:** Production structure, CI, installer, persistence.
- **Dependencies:** Rust toolchain, Node 20+, WebView2 present.
- **Implementation areas:** `spikes/tauri-poc/`.
- **Acceptance criteria:**
  - [ ] `npm run tauri dev` opens a window titled with the app name.
  - [ ] The React view displays the value returned by `get_app_info`.
  - [ ] Cold start time to interactive window is measured and recorded.
  - [ ] Closing the window terminates all child processes (verified in Task
        Manager).
  - [ ] Pinned versions of `tauri`, `tauri-cli`, `vite`, `react` are recorded.
- **Tests:** Manual launch + process-list check. Record numbers in
  `progress.md`.
- **Risks:** WebView2 absence on stripped Windows images; Rust toolchain/MSVC
  build tools missing.
- **Status:** Not started

### [ ] P0-03 — Compare playback engine candidates

- **Goal:** Choose the playback engine on evidence, not preference.
- **Scope:** Evaluate at least three candidates against the same media set:
  a native Windows media path, a libmpv-based embed, and one HTML5/video-element
  baseline. Measure codec coverage, hardware decode on Intel/AMD/NVIDIA,
  subtitle rendering (SRT/ASS), audio-track switching, HDR behaviour, memory
  use during playback, licence terms, and Tauri integration cost.
- **Out of scope:** Building player UI, resume logic, or queue integration.
- **Dependencies:** P0-02.
- **Implementation areas:** `spikes/player-comparison/`, `docs/decisions/`.
- **Acceptance criteria:**
  - [ ] A comparison table exists with one column per candidate and measured
        cells (not "good/bad") for each criterion.
  - [ ] Codec results are stated per container/codec actually tested
        (e.g. H.264/MP4, HEVC/MKV, AV1, VP9/WebM, AAC, AC3, FLAC).
  - [ ] Hardware decode is verified per GPU vendor actually available, and
        honestly marked UNTESTED where hardware is absent.
  - [ ] Every candidate's licence is recorded, including whether shipping it
        requires attribution or limits redistribution.
  - [ ] An ADR recommends one engine, names the rejected ones and why, and is
        marked Accepted by the human partner.
- **Tests:** Manual playback matrix. Record pass/fail per codec, plus
  memory/CPU samples during a 10-minute playback run.
- **Risks:** Choosing an engine that cannot do hardware HEVC or ASS subtitles;
  licence incompatibility discovered after integration.
- **Status:** Not started

### [ ] P0-04 — Benchmark host, runtime and playback resource usage

- **Goal:** Establish which hardware profiles can support the required Docker backend boundary plus Windows UI and playback; do not infer 4/8/16 GB support.
- **Scope:** Consolidate the measurements from P0-10 (runtime distributions), P0-12 (playback cost) and P0-14 (workspace footprint) into one picture per hardware profile: cold start, idle RAM/CPU, disk footprint, and 1080p playback RAM/CPU, measured separately and combined. Mark unavailable evidence UNTESTED.
- **Out of scope:** Optimisation work or host/runtime installation without explicit consent.
- **Dependencies:** P0-02, P0-10, P0-12, P0-14.
- **Implementation areas:** `docs/testing/`.
- **Acceptance criteria:**
  - [ ] A results table records idle RAM/CPU, playback RAM/CPU and startup
        time per hardware profile tested.
  - [ ] Each row states the exact machine: CPU, RAM, GPU, Windows build.
  - [ ] Profiles that could not be tested are marked UNTESTED with a reason —
        not extrapolated.
  - [ ] `Development.md` §9.1 tiers are confirmed, amended, or marked
        unproven, with the human partner's sign-off.
- **Tests:** Repeat each measurement 3 times; report median and note variance.
- **Risks:** Publishing a hardware promise the product cannot keep.
- **Status:** Not started

### [ ] P0-05 — Validate Windows runtime distributions and boundary requirements

- **Goal:** Gather the documentary and licensing evidence needed to select a supported Docker Engine-compatible runtime distribution for the mandatory v1 backend boundary.
- **Scope:** Compare Docker Desktop (headless/GUI closed) with the proposed dedicated WSL2 Engine using primary documentation and an explicitly approved clean test environment. Document licensing, prerequisites, signing, start/stop/recovery, uninstall, idle cost, update/repair ownership and absent/stopped/unhealthy behavior. Define experiments for host-to-container access, LAN reachability, egress denial, read-only mounts, path translation and Compose health/lifecycle.
- **Out of scope:** Installing Docker on the user's current machine without explicit consent; product implementation; the *measurements* themselves, which are P0-10.
- **Dependencies:** None.
- **Implementation areas:** `docs/decisions/`, `docs/setup/`.
- **Acceptance criteria:**
  - [ ] A written procedure lists every host change the runtime requires and
        the exact consent wording the app will show.
  - [ ] Idle and active runtime costs are measured for candidates where approved; unavailable evidence is marked UNTESTED.
  - [ ] Missing/stopped/unhealthy runtime behavior is specified as a recoverable error state; backend-dependent features do not run outside Docker.
  - [ ] The licence position for the intended user population is stated
        explicitly, or flagged as an open decision requiring approval.
  - [ ] An ADR is accepted by the human partner.
- **Tests:** Install/disable/uninstall cycle on the clean machine, with the
  Windows feature state compared before and after.
- **Risks:** Silent WSL2/virtualisation enablement; misleading "free" claim for
  commercial users; broken uninstall leaving host residue.
- **Status:** Not started

### [ ] P0-06 — Select and document the supported runtime distribution

- **Goal:** Recommend a Windows runtime distribution for the already-mandated Docker/Compose backend boundary, based on P0-04/P0-05/P0-10 evidence.
- **Scope:** Compare supported candidates against licensing, resource cost, lifecycle, recovery, security and user consent. Update ADR-0001's runtime-distribution status only after evidence and human review; Docker's mandatory-v1 status is not reopened here.
- **Out of scope:** Removing Docker from v1, installing/configuring runtime on the user's machine, or implementing Compose.
- **Dependencies:** P0-04, P0-05, P0-10.
- **Implementation areas:** `docs/decisions/`, `docs/setup/`.
- **Acceptance criteria:**
  - [ ] Candidate comparison includes evidence and explicitly states remaining unknowns.
  - [ ] ADR-0001 separates the mandatory-boundary decision from the proposed/unselected runtime distribution.
  - [ ] The human partner approves the supported distribution and consent path before implementation.
- **Tests:** Documentation gate; no runtime test is called PASS unless executed.
- **Risks:** Selecting a distribution without verified licensing, lifecycle, networking or resource evidence.
- **Status:** Not started

### [ ] P0-07 — Threat model and ADR set for Phase 0 decisions

- **Goal:** A threat model covering the surfaces that later phases depend on.
- **Scope:** Docker control-plane access, volume mounts, hostile media, local
  APIs, credentials, destructive deletion, updater supply chain. Record
  accepted ADRs for P0-01/03/05/06.
- **Out of scope:** Implementing mitigations.
- **Dependencies:** P0-01, P0-03, P0-05, P0-06.
- **Implementation areas:** `docs/security/`, `docs/decisions/`.
- **Acceptance criteria:**
  - [ ] `docs/security/threat-model.md` lists assets, actors, entry points,
        abuse cases and the mitigation each maps to in `Development.md` §8.
  - [ ] Every ADR from Phase 0 is in `docs/decisions/` and indexed.
  - [ ] No open decision from `Development.md` §18 is silently resolved: each
        is either decided with evidence or listed as `[?]` here.
- **Tests:** None (documentation gate). Human review required.
- **Risks:** A threat model written to match the design instead of challenging
  it.
- **Status:** Not started

### [ ] P0-08 — Frontend interaction prototype contract

- **Goal:** Prove shelf navigation and focus behaviour are achievable before
  the design system is built.
- **Scope:** A static React prototype with mock data proving: horizontal shelves,
  directional focus within a shelf, vertical movement preserving column,
  visible focus at TV distance, focus restoration after an overlay closes, and
  a player overlay that fades on idle.
- **Out of scope:** Real data, backend calls, persistence, final visuals.
- **Dependencies:** P0-02.
- **Implementation areas:** `spikes/nav-prototype/`.
- **Acceptance criteria:**
  - [ ] A user can traverse the whole prototype with arrow keys, Enter and
        Escape only.
  - [ ] Focus is visible in every state, including after using the mouse.
  - [ ] Opening and closing an overlay restores focus to the exact card that
        opened it.
  - [ ] Focused card scaling does not reflow or shift neighbouring cards.
  - [ ] `prefers-reduced-motion` is honoured.
- **Tests:** Manual keyboard-only walkthrough; recorded as a step list in
  `progress.md`.
- **Risks:** Building the design system on a focus model that does not survive
  real shelves.
- **Status:** Not started

### [ ] P0-09 — Write the Phase 1 execution plan

- **Goal:** A bite-sized, dependency-ordered plan for Phase 1, derived from
  Phase 0 evidence.
- **Scope:** Task-level plan with files, tests and commands.
- **Dependencies:** P0-01 … P0-08.
- **Implementation areas:** `docs/superpowers/plans/`.
- **Acceptance criteria:**
  - [ ] Plan references the accepted ADRs and does not contradict them.
  - [ ] Every Phase 1 task below has a matching plan section.
  - [ ] Plan includes P1-07 (required Docker workspace) and P1-08 (host↔container
        boundary), which Phase 2 and later depend on.
- **Tests:** None.
- **Risks:** Planning Phase 1 against unproven assumptions.
- **Status:** Not started

### [ ] P0-10 — Measure the cost of each candidate runtime distribution

- **Goal:** Replace the qualitative RAM/CPU/disk claims in ADR-0001 with numbers.
- **Scope:** On this machine, measure idle and active footprint for (a) Docker
  Desktop with the GUI closed, and (b) Docker Engine in a dedicated WSL2
  distro. Measure the engine alone, then `core` + `probe` on top of it. Record
  the WSL2 VM memory ceiling actually in force. Repeat 3 times, report medians.
- **Out of scope:** Installing either runtime without explicit consent; any
  optimisation work.
- **Dependencies:** Approved test environment and runtime access.
- **Implementation areas:** `docs/testing/`.
- **Acceptance criteria:**
  - [ ] A table records idle RAM, idle CPU, disk footprint and cold-start
        time for each candidate, with the machine stated exactly.
  - [ ] The measured numbers replace the qualitative cells in ADR-0001's
        comparison table.
  - [ ] Unavailable measurements are marked UNTESTED with a reason, never
        extrapolated.
  - [ ] The effect of a **shared** engine (other workloads on the same daemon)
        is measured separately from an isolated one, and recorded.
- **Tests:** Repeat each measurement 3 times; report median and variance.
- **Risks:** Choosing a distribution from a desktop-environment impression
  rather than a measurement.
- **Status:** Not started

### [ ] P0-11 — Attempt hardware video decode inside a container

- **Goal:** Establish by experiment, not by reasoning, that containerised
  hardware video decode is not viable for our target hardware — so that
  ADR-0002's host-placement decision rests on an observed result.
- **Scope:** Inside a container on the reference machine, attempt `vaapi`,
  `nvdec` (with the NVIDIA Container Toolkit, if the machine has an NVIDIA
  GPU) and `vulkan` video decode. Record exactly what was tried, the error,
  and whether the reference machine even has the required GPU. Separately,
  record what the display and audio situation is inside the container.
- **Out of scope:** Product implementation; any change to the host GPU stack
  without consent.
- **Dependencies:** P0-10 (a working engine to run the experiment in).
- **Implementation areas:** `docs/decisions/`, `docs/testing/`.
- **Acceptance criteria:**
  - [ ] Each of `vaapi`, `nvdec` and `vulkan` has a recorded attempt: command
        run, error output, and conclusion.
  - [ ] The record states plainly that the reference machine has no NVIDIA
        GPU, so the `nvdec` outcome is a hardware-availability observation and
        **not** proof that `nvdec` would fail on an NVIDIA machine.
  - [ ] Display-server and audio-endpoint availability inside the container
        are recorded as observed facts.
  - [ ] ADR-0002's §"Why the container cannot do this reliably" is updated to
        match what was observed.
- **Tests:** Manual experiment, transcribed verbatim into the ADR.
- **Risks:** Over-claiming from a single machine; or under-claiming and
  discarding a viable path without evidence.
- **Status:** Not started

### [ ] P0-12 — Record the native-Windows codec, GPU, subtitle and audio matrix

- **Goal:** The engine recommendation rests on a measured matrix.
- **Scope:** Using the leading candidate (libmpv, native Windows build) on the
  reference machine, play a fixture set covering H.264 1080p and 4K, HEVC
  8-bit and 10-bit 4K, AV1 4K, VP9, plus AAC, AC3/EAC3 and FLAC audio, with
  SRT and ASS subtitles. Record hardware decode engaged (GPU engine counters,
  not "looks smooth"), CPU %, dropped frames, seek latency, audio format and
  passthrough result.
- **Out of scope:** Building player UI; the exact `--hwdec`/`--gpu-context`
  tuning for hardware we do not have.
- **Dependencies:** P0-02 (a shell to host the spike).
- **Implementation areas:** `docs/testing/`, `docs/decisions/`.
- **Acceptance criteria:**
  - [ ] A matrix exists with one row per fixture and a measured value in
        every cell (PASS / FAIL / UNTESTED — never a guess).
  - [ ] Hardware decode is verified per GPU actually present, and the
        reference machine's GPU is named.
  - [ ] Software-decode fallbacks are recorded explicitly, so the user-facing
        behaviour for those files is known in advance.
  - [ ] ASS positioning and styling are either supported or documented as not
        supported — no silent degradation.
  - [ ] The mpv licence of the exact build used (LGPL or GPL) is recorded from
        the build configuration, not from memory — this is O-09.
- **Tests:** Manual playback matrix, transcribed into `docs/testing/`.
- **Risks:** Recommending an engine that cannot do the formats users actually
  have; discovering the licence obligation after integration.
- **Status:** Not started

### [ ] P0-13 — Prove host reachability and LAN non-reachability

- **Goal:** The single published API port is reachable from the Windows host
  and unreachable from anywhere else. This is the assumption the whole
  host↔container boundary rests on.
- **Scope:** With the workspace running, measure: (a) can the Windows host
  reach the published port? (b) can a second machine on the LAN reach it?
  (c) is any `netsh interface portproxy` rule present? (d) is any listener on
  2375/2376? (e) is the WSL2 networking mode NAT or mirrored? (f) is
  `probe`'s internal network genuinely gateway-free?
- **Out of scope:** Changing the host's firewall or network configuration
  without consent; exposing anything for the test.
- **Dependencies:** P0-10 (a running engine and workspace).
- **Implementation areas:** `docs/architecture/docker-workspace.md`, §3.
- **Acceptance criteria:**
  - [ ] Each of the six checks has a recorded observed value, not an
        assumption.
  - [ ] If the host cannot reach the port, the documented fallbacks are
        tested and the one that works is recorded here.
  - [ ] A mirrored `.wslconfig` is recorded as a **blocking** condition
        (R-18), not worked around.
  - [ ] The result is written into `docs/architecture/docker-workspace.md` §3.1
        and the placeholder text is removed.
- **Tests:** Reachability test from the host; reachability test from a second
  machine on the LAN; `netsh` and `Get-NetTCPConnection` inspection.
- **Risks:** Assuming loopback-only isolation because the configuration says
  loopback.
- **Status:** Not started

### [ ] P0-14 — Baseline the total added footprint of the required workspace

- **Goal:** A defensible number for what the mandatory boundary costs, so
  hardware tiers and release claims are honest.
- **Scope:** Measure the full stack on the reference machine: WSL2 VM with
  engine only, then `core` running, then `core` + `probe` running, then with
  playback active. Record RAM, CPU, disk and startup latency at each step.
- **Out of scope:** Optimisation; any claim about hardware we did not test.
- **Dependencies:** P0-10, P0-13.
- **Implementation areas:** `docs/testing/`.
- **Acceptance criteria:**
  - [ ] One table records each step's RAM, CPU, disk and startup cost.
  - [ ] The total is compared against the `Development.md` §9.1 tiers, and each
        tier is marked Confirmed / Amended / Unproven.
  - [ ] `docs/architecture/docker-workspace.md` §5.1 is updated with the
        measured figures.
  - [ ] No minimum hardware specification is published from an unmeasured
        profile.
- **Tests:** Repeat each measurement 3 times; report median and variance.
- **Risks:** Publishing a hardware promise the product cannot keep.
- **Status:** Not started

### Checkpoint CP-0 — Assumptions proven — OPEN

- **Gate:** P0-01 … P0-14 complete.
- **Evidence required:**
  - [ ] Playback ADR accepted, with the codec matrix attached (P0-12).
  - [ ] The mandatory Docker boundary is confirmed in writing as human-reviewed;
        the runtime distribution recommendation is evidence-backed and
        human-reviewed (P0-10).
  - [ ] Runtime networking/isolation and lifecycle evidence recorded; untested
        properties are explicitly UNTESTED (P0-13).
  - [ ] The containerised hardware-decode question is settled by experiment,
        not by argument (P0-11).
  - [ ] Resource numbers recorded for a tested profile; unsupported or
        unavailable profiles remain unproven (P0-10, P0-14).
  - [ ] Threat model reviewed by a human, including the Docker/WSL2 surface
        and residual risks R-08…R-21.
  - [ ] Navigation prototype demonstrated keyboard-only.
  - [ ] Human-reviewed evidence confirms Docker/Compose is mandatory for the
        backend boundary; optional providers remain separate decisions.
  - [ ] The libmpv licence question (O-09) is answered, and it is compatible
        with the intended project licence.
- **If it fails:** The stack changes now, while nothing depends on it. Do not
  proceed to Phase 1 with an unresolved playback, runtime or licence decision.

**CP-0 does not pass merely because the documentation is written.** Every item above
requires an observed value recorded in `progress.md` — a measurement, a command output
or a human sign-off — not a proposal. A document that says "recommended" is not
evidence that the recommendation holds.

---

# Phase 1 — Foundation

Purpose: turn the proof of concept into a repository that builds, logs, fails
safely and can be verified by CI.

### [~] P1-01 — Create the workspace and repository conventions

- **Goal:** A Cargo workspace plus a frontend package that builds from a clean
  clone with one documented command.
- **Scope:** `apps/desktop` (Vite + React + TS), `apps/desktop/src-tauri`,
  `crates/*` workspace members, rustfmt/clippy config, `.editorconfig`,
  `.gitignore`, `rust-toolchain.toml`, README quick-start.
- **Out of scope:** Domain logic, database, UI screens.
- **Dependencies:** CP-0.
- **Implementation areas:** `apps/`, `crates/`, root config.
- **Acceptance criteria:**
  - [ ] `cargo build` succeeds for every workspace member.
  - [ ] `npm run build -w apps/desktop` succeeds from a clean `node_modules`.
  - [ ] Toolchain and package versions are pinned and committed as lockfiles.
  - [ ] `rust-toolchain.toml` pins the exact channel, so a contributor gets the
        same compiler.
- **Tests:** Clean-clone build on the target machine; `cargo build --locked`.
- **Risks:** Unpinned toolchains causing "works on my machine" drift.
- **Status:** Not started

### [~] P1-02 — Add CI for Rust and frontend

- **Goal:** Every push runs format, lint, tests and build for both halves.
- **Scope:** `.github/workflows/ci.yml` with separate Rust and Node jobs,
  lockfile-enforced installs, and caching.
- **Out of scope:** Release signing, deployment.
- **Dependencies:** P1-01.
- **Implementation areas:** `.github/workflows/`.
- **Acceptance criteria:**
  - [ ] CI fails on `cargo fmt --check` violation.
  - [ ] CI fails on a clippy warning at the configured lint level.
  - [ ] CI fails on a failing Rust test and on a failing frontend test.
  - [ ] CI fails on TypeScript type errors.
  - [ ] A green run is demonstrated on a branch, not asserted.
- **Tests:** Deliberately break one check per job and confirm red, then revert.
- **Risks:** A CI that only proves the happy path.
- **Status:** In progress — CI workflow and H1/H2/H3 tests exist and pass locally; a green GitHub Actions run has not been demonstrated.

### [ ] P1-03 — Implement structured logging and redaction

- **Goal:** Diagnosable logs that cannot leak user data.
- **Scope:** `tracing` with rolling file appender into
  `%LOCALAPPDATA%/MediaHaven/app/logs/`, log levels, rotation and retention, and
  a redaction layer that strips paths, filenames, tokens and URLs.
- **Out of scope:** The diagnostics bundle export (P5-06).
- **Dependencies:** P1-01.
- **Implementation areas:** `crates/diagnostics`, `platform-windows`.
- **Acceptance criteria:**
  - [ ] Logs are written to the app data directory, never the install dir.
  - [ ] A deliberately logged home path appears redacted in the output file.
  - [ ] Rotation bounds total log size; a test proves the bound is enforced.
  - [ ] Log level is configurable and defaults to no debug-level path output.
- **Tests:** Unit tests for the redactor (path, filename, token, URL cases) plus
  an integration test that writes past the rotation budget.
- **Risks:** Redaction that misses a case and leaks a filename into a bundle.
- **Status:** Not started

### [ ] P1-04 — Implement versioned configuration

- **Goal:** Settings persist, migrate and fail safe when corrupt.
- **Scope:** Serde config with an explicit `config_version`, validation on
  load, atomic write (temp file + rename), recovery from a backup copy when the
  primary is invalid, and a documented default set.
- **Out of scope:** The settings UI (P4-04). Secrets storage.
- **Dependencies:** P1-01.
- **Implementation areas:** `crates/app-core/application`, `persistence`.
- **Acceptance criteria:**
  - [ ] A corrupt config file is quarantined and defaults are loaded, with a
        user-visible warning — the app still starts.
  - [ ] A config from a previous version migrates without data loss.
  - [ ] An interrupted write cannot leave a truncated config: a test aborts
        mid-write and asserts the previous file is intact.
  - [ ] Unknown keys are preserved or explicitly rejected — behaviour is stated
        in the ADR, not implicit.
- **Tests:** Unit tests for validation, migration and atomic write; a test that
  feeds malformed JSON and asserts the quarantine path.
- **Risks:** Silent settings loss on upgrade; partially written config.
- **Status:** Not started

### [~] P1-05 — Implement SQLite migrations and backup

- **Goal:** A database that can be created, migrated and restored without data
  loss.
- **Scope:** `sqlx` (or the chosen library) with embedded migrations, a
  `schema_migrations` table, a pre-migration backup copy, foreign keys on,
  WAL mode, and migration tests against a representative prior-version fixture.
- **Out of scope:** The full entity set (Phase 2 defines real tables).
- **Dependencies:** P1-01.
- **Implementation areas:** `crates/persistence/sqlite`, `migrations/`.
- **Acceptance criteria:**
  - [ ] A new database is created from scratch with all migrations applied.
  - [ ] Migrating a seeded prior-version database preserves its rows and
        creates a backup file first.
  - [ ] A failed migration rolls back and leaves the app able to start in a
        recovery state.
  - [ ] `PRAGMA foreign_keys=ON` is asserted by a test that violates a
        constraint and expects failure.
  - [ ] Database lives under the app data dir, never the install dir.
- **Tests:** Migration tests on fixtures; a deliberate failing migration test.
- **Risks:** A migration that drops user data; app refusing to boot on a
  recoverable database.
- **Status:** Not started

### [ ] P1-06 — Build first-run preflight and setup flow

- **Goal:** A non-technical user is clearly guided through prerequisites for the mandatory Docker backend boundary without exposing terminal concepts or silently changing the host.
- **Scope:** Read-only preflight (OS edition/build/arch, RAM, free disk,
  virtualisation capability detection, port availability, existing runtime
  detection), a summary screen explaining what is needed and why, storage-root
  selection with free-space validation, and a clean abort path.
- **Out of scope:** Installing anything, enabling Windows features, requesting
  admin rights, downloading runtimes.
- **Dependencies:** P1-03, P1-04, P0-05 (runtime validation).
- **Implementation areas:** `scripts/windows/preflight.ps1`,
  `apps/desktop/src/pages/Setup`, `crates/platform-windows`.
- **Acceptance criteria:**
  - [ ] Preflight performs only reads: a before/after comparison of Windows
        feature state and installed software shows zero changes.
  - [ ] Every failed check produces a plain-language message with a next step,
        not an error code alone.
  - [ ] The user can cancel at any step and the app exits cleanly with no
        partial configuration written.
  - [ ] If the required runtime is absent or unhealthy, setup explains recovery and backend-dependent features remain unavailable; no host-side backend fallback exists.
  - [ ] No administrator prompt is triggered anywhere in the flow.
- **Tests:** Run on a machine with the runtime absent and on one with it
  present; record both. Verify no host mutation.
- **Risks:** A preflight probe that itself changes host state; a setup flow that
  dead-ends a 4 GB machine.
- **Status:** Not started

### [~] P1-07 — Stand up the required Docker runtime and the `core`/`probe` workspace

- **Goal:** The mandatory container boundary exists, running and measurable
  before any product feature is built on top of it.
- **Scope:** The host-side runtime adapter (detect the approved distribution,
  start/stop the engine inside the dedicated WSL2 distro, run
  `docker compose up -d --wait` through `wsl.exe`), the pinned `core` and
  `probe` images, the hardened `compose.yaml`, and the per-root read-only
  media mount rendering.
- **Out of scope:** Host-side backend fallback (forbidden), optional provider
  containers, and any attempt to install a runtime without consent.
- **Dependencies:** CP-0 (runtime distribution accepted), P1-01, P1-06.
- **Implementation areas:** `crates/runtime-adapter`,
  `infrastructure/containers/`, `crates/app-core` (container build).
- **Acceptance criteria:**
  - [ ] `core` starts, reports healthy, and serves the API on a loopback
        published port; `probe` starts with no published port and no gateway.
  - [ ] `docker inspect` shows non-root users, `cap_drop: ALL`,
        `no-new-privileges`, read-only root filesystems and the declared
        resource limits — asserted, not eyeballed.
  - [ ] `docker inspect` shows exactly the declared mounts and no more; a
        library root that was not added is not visible inside any container.
  - [ ] No container mounts the Docker control socket; no container is
        privileged; no image is referenced by a mutable tag.
  - [ ] A path-traversal attempt through the backend cannot modify the user's
        media, because the mount is read-only.
  - [ ] The workspace stops cleanly on app exit and leaves no orphan
        containers.
  - [ ] The supervisor refuses to start anything when the required runtime is
        absent, and reports a legible error instead.
- **Tests:** Static compose lint in CI; `docker inspect` assertions;
  recreate-and-verify test; a mount-scope test; an orphan-container check.
- **Risks:** Building product features on a boundary that was never
  exercised; a container that can reach more than it should.
- **Status:** Code complete, unproven — compose/Dockerfile/mount-override code exists; no live engine build or docker inspect evidence (needs consent).

### [~] P1-08 — Implement the host↔container boundary and the supervisor lifecycle

- **Goal:** The Windows host can talk to `core` securely, and the frontend
  cannot reach the Docker daemon or the API directly.
- **Scope:** The per-install token (Windows Credential Manager), the
  bearer+HMAC request scheme, the `/healthz` and `/readyz` endpoints, the
  generated host↔backend schema on both sides, the contract test that fails
  on schema drift, and the supervisor's start/stop/health/recovery behaviour.
- **Out of scope:** A generic "call the backend" passthrough command; any
  Docker access from the frontend.
- **Dependencies:** P1-07, P1-03 (redaction), P1-04 (config).
- **Implementation areas:** `crates/runtime-adapter`, `crates/app-core/ports`,
  `apps/desktop/src-tauri/src/commands`.
- **Acceptance criteria:**
  - [ ] The API refuses every request without a token, with a wrong token,
        with a replayed signature and with a tampered body — four tests, all
        failing closed.
  - [ ] `/healthz` is unauthenticated and returns nothing but a status;
        `/readyz` requires auth.
  - [ ] The frontend's capability surface is named commands only. A test
        asserts there is no generic HTTP or Docker passthrough command.
  - [ ] The container's environment holds the token and nothing else
        sensitive; the host reads it from Credential Manager, never from a
        file in the repository.
  - [ ] Both sides of the schema are generated from one source, and a
        contract test fails the build if they diverge.
  - [ ] Domain and application crates contain zero container-runtime and zero
        HTTP-transport dependencies (verified by a dependency check).
- **Tests:** Auth-matrix test; schema-drift test; dependency-graph assertion.
- **Risks:** A generic passthrough command quietly becoming a full-capability
  API for the frontend.
- **Status:** In progress — named commands, DPAPI token, supervisor, HMAC forwarder and auth/contract tests exist; live engine + token-forwarding validation NOT RUN.

### Checkpoint CP-1 — It builds, it logs, it fails safely — OPEN

- **Gate:** P1-01 … P1-08 complete.
- **Evidence required:**
  - [ ] CI red on a deliberately broken check, then green.
  - [ ] Corrupt-config recovery demonstrated.
  - [ ] Failed-migration recovery demonstrated.
  - [ ] Preflight before/after host-state comparison recorded as "no change".
  - [ ] The required `core`/`probe` workspace starts, passes its `docker
        inspect` assertions, and stops without orphans.
  - [ ] The auth matrix (no token / wrong token / replayed signature / tampered
        body) is demonstrated failing closed.
  - [ ] `progress.md` verification table all PASS or explicitly NOT RUN with a
        reason.
- **If it fails:** Fix foundations here. Every later phase inherits these
  guarantees, and retrofitting atomic writes, redaction or a container
  boundary is expensive.

---

# Phase 2 — Core local library

Purpose: the user's own media becomes a browsable, searchable library without
touching their files.

### [~] P2-01 — Add user-selected folder import

- **Goal:** A user picks folders; the app indexes them and never modifies them.
- **Scope:** Folder picker, persisted library roots, canonical path storage,
  validation that a root is a directory and readable, permission-error
  handling, removal from library without deleting files.
- **Out of scope:** Whole-drive or profile scanning. Moving/renaming sources.
- **Dependencies:** CP-1.
- **Implementation areas:** `crates/media-library`, `crates/app-core`,
  `apps/desktop/src/pages/Library`.
- **Acceptance criteria:**
  - [ ] Only explicitly selected folders are read; no default scan of Desktop,
        Documents, profile or all drives.
  - [ ] Removing a root deletes zero files — asserted by a test that counts
        files before and after.
  - [ ] An unreadable or disconnected root is reported as offline, not as a
        crash or as deleted media.
  - [ ] A root inside another root is detected and does not cause duplicate
        indexing.
- **Tests:** Integration tests against a temp fixture tree; a test with a
  read-only directory; a test that removes a root and asserts file count.
- **Risks:** Accidentally reading the whole user profile; duplicate items from
  nested roots.
- **Status:** Not started

> Backend completion note (2026-11-17): backend routes for root management exist (`/library_root_add`, `/library_root_remove`); the folder picker, nested-root detection and file-count assertion are frontend/test work and remain open.

### [~] P2-02 — Implement a safe scanner and metadata extraction

- **Goal:** Deterministic, interruptible, resumable scanning that treats every
  filename as hostile input.
- **Scope:** Extension allowlist, canonicalisation, symlink/reparse-point
  refusal, bounded concurrency, cancellation, rescan diffs, duplicate detection
  by size+hash without deletion, duration/technical metadata read, and traversal
  rejection.
- **Out of scope:** Online metadata fetching (P2-06). Thumbnailing UI.
- **Dependencies:** P2-01.
- **Implementation areas:** `crates/media-library`.
- **Acceptance criteria:**
  - [ ] A path containing `..` that resolves outside the root is rejected, with
        a test proving no read occurred.
  - [ ] A Windows reparse point/junction pointing outside the root is refused.
  - [ ] Scanning is cancellable mid-run and resumes without duplicating rows.
  - [ ] A file with an absurd or hostile filename (control characters, very
        long name, emoji) indexes without panic and renders safely.
  - [ ] Interrupted scans are recovered on next start, and the app reports the
        incomplete state honestly.
  - [ ] Duplicate detection identifies duplicates and deletes nothing.
  - [ ] A file that is locked or disappears mid-scan is skipped, not fatal.
- **Tests:** Unit tests for path policy and filename sanitiser; integration
  tests for cancellation, resume, duplicates and locked files.
- **Risks:** Path escape, resource exhaustion on huge trees, duplicate flood.
- **Status:** Not started

> Backend completion note (2026-11-17): closed by the 2026-11-17 backend pass — path-escape and hostile-filename tests (`crates/media-library/tests/roots.rs`), cancellation idempotency, duplicate detection, disappearing-file skip. Remaining: interrupted-scan recovery on next start; duration/technical metadata (needs ffprobe in the `probe` container — not the O-04 provider decision, which was accepted 2026-11-18; local NFO titles/year/synopsis/genres now come from `crates/metadata`).

### [~] P2-03 — Implement media database entities

- **Goal:** The data model from `Development.md` §5 exists with constraints and
  indexes that match real queries.
- **Scope:** `media_items`, `media_files`, `collections`, plus supporting
  entities needed by later phases; foreign keys; UTC timestamps; indexes for
  library, search and sort paths.
- **Out of scope:** Playback and queue tables (Phases 3–4) unless required by a
  foreign key — then create them with the minimum shape.
- **Dependencies:** P1-05, P2-02.
- **Implementation areas:** `crates/persistence/sqlite`, `migrations/`,
  `crates/app-core/domain`.
- **Acceptance criteria:**
  - [ ] Schema matches the entity list, with documented deviations recorded in
        an ADR.
  - [ ] Deleting a `media_item` with `media_files` is blocked or cascades — the
        chosen behaviour is explicit and tested.
  - [ ] Timestamps are stored in UTC and round-trip correctly across a DST
        boundary test.
  - [ ] Paths are stored platform-aware and compared normalised.
  - [ ] Query plans for library browse, search and sort use the intended indexes
        (assert via `EXPLAIN QUERY PLAN` in a test).
- **Tests:** Constraint tests, UTC/DST round-trip, index-usage test, cascade
  behaviour test.
- **Risks:** Missing indexes causing a slow library at 10k items.
- **Status:** Not started

> Backend completion note (2026-11-17): closed — schema matches entity list with deviations recorded in ADR-0010 (`collections` deferred), UTC round-trip test, index-usage test via EXPLAIN, and `media_replace_for_root` upsert+prune with cascade-safe rescan tests. Remaining: `collections` (deferred), normalised path comparison, explicit cascade test for `media_item` deletion.

### [~] P2-04 — Build Movies, Shows and Library views

- **Goal:** A user can browse their indexed media by type.
- **Scope:** Movies grid/shelf, Shows with seasons and episodes, Library view
  with roots, scan status, missing/offline files; empty, loading, offline,
  missing-artwork and error states.
- **Out of scope:** Playback. Details actions beyond navigation.
- **Dependencies:** P2-03.
- **Implementation areas:** `apps/desktop/src/pages/{Movies,Shows,Library}`,
  `components/{cards,shelves,feedback}`.
- **Acceptance criteria:**
  - [ ] Every listed state (empty, loading, offline, no artwork, error) has a
        rendered state, each verifiable by a fixture or a forced condition.
  - [ ] Large libraries (10k fixture items) render without eager-loading every
        image.
  - [ ] Missing artwork never breaks card dimensions; a placeholder fills the
        same box.
  - [ ] Sorting and filtering do not lose the user's position unexpectedly.
- **Tests:** Frontend tests per state; a performance check on the 10k fixture
  asserting bounded image requests.
- **Risks:** Janky scrolling; layout shift when artwork fails.
- **Status:** In progress — Movies/Shows/Library/Search screens built and browser-checked against the host-run backend; data-dependent criteria remain open.

### [~] P2-05 — Add search, sorting, filtering and empty/error states

- **Goal:** Finding a title in a large library is fast and local.
- **Scope:** Local-first search over indexed fields, sort and filter controls,
  debounced input, no-network guarantee, and an explicit note when an online
  search would be used (if ever enabled).
- **Out of scope:** Remote metadata search (P2-06, still optional).
- **Dependencies:** P2-03.
- **Implementation areas:** `apps/desktop/src/pages/Search`,
  `crates/media-library`.
- **Acceptance criteria:**
  - [ ] Search works with the network disconnected.
  - [ ] A 10k-item fixture returns results within the agreed budget (record the
        measured number; do not claim "fast").
  - [ ] Special characters in the query (`%`, `_`, `"`, `*`) do not error and
        do not alter query semantics.
  - [ ] Zero results shows an actionable empty state.
- **Tests:** Unit tests for the query builder including wildcard escaping;
  timing test on the fixture.
- **Risks:** SQL injection via search input; unindexed full scans.
- **Status:** In progress — local search implemented and timed at crate level; 10k-item frontend check and wildcard-input proof remain open.

### [~] P2-06 — Metadata and artwork cache with offline fallback

- **Goal:** Nice metadata when possible, useful library when not.
- **Scope:** Provider interface with at least one optional provider, local cache
  with size limits and cleanup, artwork placeholder path, manual correction,
  explicit disclosure of any network request, and full offline operation.
- **Out of scope:** Paid providers. Uploading library contents. Scraping.
- **Dependencies:** P2-03. Provider decided: ADR-0011 is **Accepted** (2026-11-18) — TMDB primary with local NFO offline fallback; network use is opt-in and off by default. Local NFO reading needs no provider and lands first.
- **Implementation areas:** `crates/metadata`, `apps/desktop/src/components/media`.
- **Acceptance criteria:**
  - [ ] With the network disabled, the library is fully browsable using cached
        and local data.
  - [ ] No filenames or library contents leave the machine unless the user opts
        in, and the opt-in text names exactly what is sent.
  - [ ] Cache respects a configured size cap; a test overshoots it and asserts
        eviction.
  - [ ] Metadata text and images are sanitised before display.
  - [ ] Provider failure degrades to placeholders, never to a broken view.
- **Tests:** Offline-mode integration test; cache eviction test; sanitiser unit
  tests with hostile strings and malformed images.
- **Risks:** Privacy leak via filenames; cache growth; terms-of-service
  violation.
- **Status:** In progress — local NFO reader landed 2026-11-18; the opt-in TMDB
  provider (text cache, sanitisation, `poster_path` parsing, network gate) landed
  in the torrent-acquisition wave, and offline IMDb ratings landed 2026-11-21
  (ADR-0022 **Proposed**). Still open: real TMDB/OMDb HTTPS transport and the
  hashed artwork cache — an **unexecuted** plan exists at
  `docs/superpowers/plans/2026-11-21-tmdb-artwork.md` (ADR-0023 Proposed). No live
  metadata request has been made; opt-in text and attribution must be verified
  before this task can be accepted.

### [~] P2-07 — Implement the local watchlist

- **Goal:** The user can save media to watch later, and it survives restarts
  and upgrades.
- **Scope:** A `watchlist_items` table, add/remove actions reachable from any
  media card and details surface, manual ordering, and a Watchlist screen with
  movies and episodes. Missing files are shown as unavailable rather than
  dropped.
- **Out of scope:** Cloud sync, accounts, sharing, automatic reordering, and
  any link between the watchlist and the play queue.
- **Dependencies:** P2-03 (database entities).
- **Implementation areas:** `crates/app-core`, `crates/persistence/sqlite`,
  `apps/desktop/src/pages/Watchlist`, `apps/desktop/src/components/cards`.
- **Acceptance criteria:**
  - [ ] Adding an item from a media card and from the details surface both
        work, and produce the same row.
  - [ ] Order is preserved exactly as the user arranged it, across a restart.
  - [ ] Removing an item deletes **zero** files — asserted by a file-count
        test.
  - [ ] An item whose file is missing is shown as unavailable and is **not**
        silently removed.
  - [ ] The watchlist is fully usable with the network disconnected.
  - [ ] Adding to the watchlist does not change the play queue, and vice
        versa — asserted by a test.
- **Tests:** Persistence round-trip; manual reorder across restart; file-count
  assertion on removal; missing-file handling; queue independence.
- **Risks:** Conflating the watchlist with the queue, which would make
  "play next" mutate unexpectedly.
- **Status:** In progress — backend table, routes, persistence and Watchlist screen exist; file-count, restart and queue-independence tests remain.

### Checkpoint CP-2 — The library is real — OPEN

- **Gate:** P2-01 … P2-07 complete.
- **Evidence required:**
  - [ ] 10k-item fixture browsed end to end with recorded timings.
  - [ ] Path-escape and reparse-point tests passing.
  - [ ] Offline library session demonstrated with the network off.
  - [ ] File-count before/after proof that removing a root deleted nothing.
  - [ ] Duplicate detection report on a known duplicate set.
  - [ ] Watchlist persistence demonstrated across a restart, with the
        queue-independence and file-count assertions from P2-07.

> Backend-side evidence now producible (2026-11-17): `cargo test -p media-library --test roots` (8 tests, path-escape + hostile names + cancel + duplicates), `cargo test -p media-persistence` (14 + 1 ignored), `cargo test -p media-persistence -- --ignored` (10k search under 200 ms observed at 1.39 s wall incl. setup). Remaining evidence (offline session, file-count before/after, restart persistence) is frontend/integration work.
- **If it fails:** Do not start playback. Browsing on an unreliable library
  makes playback bugs indistinguishable from indexing bugs.

---

# Phase 3 — Playback

Purpose: video actually plays, and the app remembers where the user was.

### [~] P3-01 — Integrate the selected player engine

- **Goal:** The engine chosen in P0-03 plays a real file inside the app shell.
- **Scope:** Adapter behind a `Player` port, lifecycle (open/play/pause/stop),
  error surfacing for unsupported codecs and missing files, and a documented
  integration note.
- **Out of scope:** Custom controls skin (P3-02). Queue (Phase 4).
- **Dependencies:** CP-2. (Playback ADR accepted provisionally 2026-11-18; engine is mpv via JSON IPC.)
- **Implementation areas:** `crates/playback`.
- **Acceptance criteria:**
  - [ ] Every codec proven in P0-03 plays; unsupported ones produce a clear
        message naming the codec and the file.
  - [ ] A missing file mid-session does not crash the app.
  - [ ] Player process/window is always terminated on app exit (verified).
  - [ ] Hardware decode is used where the spike proved it available, and the
        app reports whether it is active.
- **Tests:** Codec matrix replay as an automated or scripted suite; kill-file
  test; exit-leak test.
- **Risks:** Engine update changing codec support; orphaned player processes.
- **Status:** In progress — engine adapter wired into host + renderer with headless evidence (2026-11-20, ADR-0015); live playback/codec/hardware-decode NOT RUN (mpv absent, install needs consent).

### [ ] P3-02 — Implement playback controls and overlay

- **Goal:** Controls usable at TV distance, fading on idle.
- **Scope:** Play/pause, seek with real seek-completion feedback, elapsed and
  remaining time, volume, subtitle and audio track selection where supported,
  next/previous episode, title overlay on start, idle fade, reduced-motion
  respect.
- **Out of scope:** Autoplay of next episode (off by default until P3-06/P4).
- **Dependencies:** P3-01.
- **Implementation areas:** `apps/desktop/src/components/player`,
  `apps/desktop/src/pages/Player`.
- **Acceptance criteria:**
  - [ ] Every control is reachable and operable with arrow keys + Enter only.
  - [ ] Seek reports actual completion, not just an input value change.
  - [ ] Controls fade after the configured idle period and return on any input.
  - [ ] Focus is visible over bright and dark video; controls are legible at
        TV distance.
  - [ ] Back/Escape exits the player and restores focus to the originating card.
- **Tests:** Manual keyboard walkthrough; a seek test asserting the reported
  position matches the engine's actual position.
- **Risks:** Controls that need a mouse; focus lost behind the video surface.
- **Status:** Not started

### [~] P3-03 — Persist resume position

- **Goal:** Stop, crash or shut down mid-episode and continue where you left off.
- **Scope:** Checkpoints written on pause, stop, periodic interval, and app
  shutdown; transactional writes; recovery after an unclean exit.
- **Out of scope:** Completion rules (P3-04).
- **Dependencies:** P3-01, P1-05.
- **Implementation areas:** `crates/playback`, `crates/persistence`.
- **Acceptance criteria:**
  - [ ] Position survives: pause, stop, window close, and a simulated kill.
  - [ ] A kill during a write leaves a consistent prior checkpoint, not a
        corrupt row.
  - [ ] Resume prompts sensibly near the start (do not resume at 3 seconds) and
        near the end (do not resume into credits) — thresholds configured, not
        hard-coded magic.
  - [ ] Two items cannot overwrite each other's progress.
- **Tests:** Integration tests for each exit path, including the simulated kill;
  a concurrency test for two items.
- **Risks:** Losing progress on power loss; corrupt row on abrupt exit.
- **Status:** In progress — pure resume policy and checkpoint persistence exist; engine wiring and four exit-path tests remain.

### [~] P3-04 — Implement completion logic and tests

- **Goal:** "Watched" is a multi-signal backend decision, never one seek.
- **Scope:** Watched-duration threshold plus position-near-end plus a normal
  end-of-file event where available plus a short grace period. Distinguish
  user-stop from completion. Configurable thresholds.
- **Out of scope:** Deletion of any kind. Queue advance business rules (Phase 4).
- **Dependencies:** P3-03.
- **Implementation areas:** `crates/playback`, `crates/app-core/domain`.
- **Acceptance criteria:**
  - [ ] Seeking to the end does **not** mark complete — explicit test.
  - [ ] Closing the app mid-episode does **not** mark complete — explicit test.
  - [ ] Removing from queue does **not** mark complete.
  - [ ] A normal long watch through the end **does** mark complete.
  - [ ] Marking complete never deletes media — asserted by file-count test.
  - [ ] Thresholds are in configuration with documented defaults.
- **Tests:** A dedicated domain test suite covering each signal and each
  combination, including adversarial short durations.
- **Risks:** False completion triggering future cleanup. This task is the
  safety boundary for automated retention: treat a failing test as a release
  blocker.
- **Status:** In progress — domain completion rules exist with unit tests; persisted completion signal and full adversarial suite remain.

### [ ] P3-05 — Test subtitles, audio tracks and hardware decoding

- **Goal:** Documented, honest support matrix for real-world files.
- **Scope:** SRT/ASS rendering, embedded vs external subtitles, audio track
  switching, sync, and hardware decode verification per available GPU.
- **Out of scope:** Subtitle downloading or conversion services.
- **Dependencies:** P3-01.
- **Implementation areas:** `docs/testing/`, `tests/test-media/`.
- **Acceptance criteria:**
  - [ ] A support matrix exists listing codec, container, subtitle and audio
        combinations as PASS / FAIL / UNTESTED.
  - [ ] ASS positioning and styling are either supported or documented as not
        supported — no silent degradation.
  - [ ] Hardware decode result is recorded per GPU actually tested; untested
        GPUs are explicitly marked.
  - [ ] Unsupported combinations surface a user-readable message.
- **Tests:** Playback of the `tests/test-media` set; results transcribed into
  the matrix.
- **Risks:** Promising subtitle support that fails on ASS-heavy releases.
- **Status:** Not started

### [~] P3-06 — Implement the durable queue and Up Next

- **Goal:** A queue that survives a crash, a reboot and an upgrade, and that
  never deletes or advances media on its own.
- **Scope:** Durable queue state in SQLite, manual reorder/remove/pause/resume,
  distinct current/next/download states, recovery after restart, duplicate
  suppression, and a history of completed and removed entries.
- **Out of scope:** Automatic download of anything; autoplay of the next episode
  (off by default until the user chooses).
- **Dependencies:** P3-03, P1-05.
- **Implementation areas:** `crates/queue`, `crates/app-core/domain`.
- **Acceptance criteria:**
  - [ ] Queue order and state survive a forced kill and a reboot.
  - [ ] The queue cannot contain the same item twice unless explicitly allowed.
  - [ ] Removing an item from the queue never marks it watched and never deletes
        any file — asserted by a file-count test.
  - [ ] A crash mid-reorder leaves a consistent order, not a corrupt one.
  - [ ] Queue state lives in SQLite, not in frontend memory.
- **Tests:** Simulated-kill recovery; duplicate suppression; reorder crash
  consistency; file-count assertion on removal.
- **Risks:** A queue that advances or deletes on its own — the same class of
  defect as R-05.
- **Status:** In progress — queue screen, durable reorder, removal history and crash-consistency tests exist; Up Next and restart proof remain.

### Checkpoint CP-3 — Video plays and remembers — OPEN

- **Gate:** P3-01 … P3-06 complete.
- **Evidence required:**
  - [ ] Playback of at least one 1080p H.264 and one HEVC file, with recorded
        hardware-decode status.
  - [ ] Resume after a simulated kill, demonstrated.
  - [ ] The five adversarial completion tests passing, listed explicitly.
  - [ ] Orphan-process check after quitting mid-playback.

> **2026-11-20:** P3-01 is now `[~]` — the mpv adapter is wired into the Tauri
> host and renderer (ADR-0015 accepted). Headless evidence is in
> `docs/security/boundary-evidence.md` and `progress.md`, but no real file has
> been played: mpv is absent and installing it needs human consent. CP-3 stays
> **OPEN** — the 1080p/HEVC playback, hardware-decode, resume-after-kill and
> orphan-process items above are all still NOT RUN.
  - [ ] Support matrix published in `docs/testing/`.
- **If it fails:** Do not build the TV layer on top of a player that loses
  position or mis-detects completion. P3-04 failures block the entire storage
  policy phase.

---

# Phase 4 — TV experience

Purpose: the product becomes pleasant to use from a couch.

> **Runs third**, after Phase 1 and before Phase 2 — the product owner chose
> frontend-first (ADR-0009). P4-01 … P4-05 build the interface against the frozen
> contract and the development mock. P4-08 re-verifies it against the real backend.
> **Dependencies below list the phases needed to *finish* each task, not the phases
> needed to *start* it.**

### [~] P4-06 — Define the typed command and DTO contract

- **Goal:** One frozen, typed description of everything the frontend may see and
  call — written before either side exists.
- **Scope:** The media, collection, episode, playback-state, queue, watchlist and
  settings DTOs; the named command surface (`library_home`, `library_search`,
  `library_detail`, `playback_start`, `playback_stop`, `queue_*`, `watchlist_*`,
  `settings_*`); the error taxonomy the frontend renders; and the mapping from each
  command to the backend operation that will eventually satisfy it.
- **Out of scope:** The implementation of any command. No Docker access, no
  filesystem paths that the frontend can act on, no generic passthrough.
- **Dependencies:** P1-01 (workspace conventions).
- **Implementation areas:** `apps/desktop/src/types/`,
  `apps/desktop/src/services/commands.ts`.
- **Acceptance criteria:**
  - [ ] Every command has an explicit request type and an explicit response type.
    Unknown fields are rejected, not ignored.
  - [ ] No DTO exposes a raw filesystem path the UI could act on; any path that
        must cross the boundary is labelled display-only.
  - [ ] The command surface is named operations only — there is no
        `http_request`-equivalent.
  - [ ] Errors are a closed, typed set, each with a user-visible message and a
        stable code. The frontend never renders a raw backend error.
  - [ ] The contract is documented so the backend can be built against it, and
        the ADR index links it.
- **Tests:** A contract test that fails the build if a command is added without a
  request and response type, or if a type is changed in one place only.
- **Risks:** A contract that leaks backend internals into the UI, making the
  backend impossible to change later.
- **Status:** In progress — crates/contract is the single source for 24 commands and 31 DTOs; parity tests pass; contract not frozen at a checkpoint.

### [~] P4-07 — Build the development mock transport

- **Goal:** The interface can be built, demoed and tested before the backend
  exists — without that becoming a host-side backend.
- **Scope:** An in-memory fixture library behind the single transport seam, with
  realistic-but-fake data, deterministic ordering, and every state the UI must
  handle (loading, empty, offline, missing artwork, error, missing file).
- **Out of scope:** Any policy, filesystem access, database, or business rule. The
  mock returns fixture objects; it decides nothing.
- **Dependencies:** P4-06.
- **Implementation areas:** `apps/desktop/src/services/mock/`,
  `apps/desktop/src/services/transport.ts`.
- **Acceptance criteria:**
  - [ ] The mock is excluded from the production bundle, and the build **fails**
        if it is reachable in a production build.
  - [ ] The mock contains no filesystem access, no database and no policy — a
        test asserts the mock module imports nothing from the domain layer.
  - [ ] Fixture data is deterministic: the same library renders identically on
        every load, so screenshots and tests are stable.
  - [ ] Every state the UI must handle is reachable: empty library, offline,
        missing artwork, missing file, and error.
  - [ ] Swapping the mock for the real Tauri transport is a change to one file.
- **Tests:** A test that the fixture set is deterministic; a test that the
  production bundle excludes the mock.
- **Risks:** The mock quietly growing into a second, unpoliced backend. That
  would breach the mandatory boundary — see ADR-0009's revisit trigger.
- **Status:** Superseded — development mock was replaced by a host Tauri command surface and a dev-only signed HTTP client (A5/A7).

### [~] P4-08 — Integrate the frontend with the real backend

- **Goal:** Every screen verified against live data from the containerised
  backend.
- **Scope:** Replace the mock with the real transport, re-run every screen against
  real library data, re-run the data-dependent acceptance criteria that P4-01 …
  P4-07 could not close, and verify the loading/empty/error paths against the real
  failure modes.
- **Out of scope:** Changing the contract to suit the UI — that direction is
  closed. A contract change goes through P4-06.
- **Dependencies:** P1-08 (host↔container boundary), P4-02 … P4-07.
- **Implementation areas:** `apps/desktop/src/services/transport.ts`,
  `apps/desktop/src/`.
- **Acceptance criteria:**
  - [ ] The frontend runs against the containerised backend with no mock code
        in the bundle.
  - [ ] Every data-dependent acceptance criterion from P4-01 … P4-05 is now
        closed with observed evidence.
  - [ ] A real library of at least 10 000 items browses without eager-loading
        every image.
  - [ ] The real failure modes — backend down, token rejected, library root
        offline — produce the designed recovery states, not blank screens.
- **Tests:** The full frontend suite against the live backend; a 10 000-item
  performance check; a backend-down smoke test.
- **Risks:** The contract and the backend disagreeing in a way the mock never
  exposed. That is the point of this task.
- **Status:** In progress — frontend verified against the host-run backend; containerised backend, 10k fixture and failure-mode suite remain.

### [~] P4-01 — Implement design tokens and responsive layout

- **Goal:** One token source for colour, type, spacing, radii, elevation,
  motion and focus — no scattered hex values.
- **Scope:** `tokens.css` plus `global.css` and `motion.css`, dark cinematic
  palette with verified contrast, TV/laptop/narrow breakpoints, reduced-motion
  variants.
- **Out of scope:** Screen composition.
- **Dependencies:** P0-08 prototype learnings. Tokens and layout do not need
  a backend.
- **Implementation areas:** `apps/desktop/src/styles/`.
- **Acceptance criteria:**
  - [ ] No component contains a hard-coded colour or spacing literal outside
        tokens (enforced by a lint or a grep check in CI).
  - [ ] Normal text meets WCAG AA contrast against its actual surface —
        computed, not eyeballed.
  - [ ] Motion durations come from tokens; reduced-motion zeroes non-essential
        motion.
  - [ ] Focus ring thickness is a token and remains visible on light and dark
        artwork.
- **Tests:** Automated contrast check on the token pairs; a grep/lint gate for
  literals; visual check at 1080p and 4K.
- **Risks:** Contrast that passes on paper and fails over artwork; token sprawl.
- **Status:** In progress — token system and literal-check gate implemented; computed contrast ratios and TV-distance review NOT RUN.

### [~] P4-02 — Build Home hero and media shelves

- **Goal:** The screen that sells the product: data-driven, never hard-coded
  titles.
- **Scope:** `HeroFeature`, `MediaShelf`, `MediaCard`, `EpisodeCard`,
  `CollectionCard`, `ProgressBar`, `StatusPill`, `Artwork`, `SkeletonCard`;
  shelf order per `Development.md` §7.4.3; empty shelves hidden; designed
  onboarding empty state.
- **Out of scope:** A recommendation engine — MVP uses deterministic local rules
  only.
- **Dependencies:** P4-06, P4-07. The real library views (P2-04) are re-verified
  against live data in P4-08.
- **Implementation areas:** `apps/desktop/src/{pages/Home,components}`.
- **Acceptance criteria:**
  - [ ] Shelf order matches the spec, and shelves with no data do not render.
  - [ ] An empty library shows the designed onboarding state with an `Add Media
        Folder` action.
  - [ ] Hero text stays legible over a bright and a dark backdrop (verified with
        both fixtures).
  - [ ] Hero does not autoplay trailers or audio and does not rotate while the
        user is interacting with it.
  - [ ] Card focus scaling (1.03–1.06) causes no reflow of neighbours.
  - [ ] Continue Watching cards show progress and a resume action.
- **Tests:** Fixture-driven frontend tests for shelf order, empty states, and
  focus scaling without reflow.
- **Risks:** Layout shift on artwork load; hero gradient failing on pale art.
- **Status:** In progress — hero/shelves/cards built and browser-checked; fixture-driven shelf-order and empty-state tests remain.

### [~] P4-03 — Implement keyboard and remote focus navigation

- **Goal:** The whole app is operable without a mouse.
- **Scope:** `FocusManager` plus a tested directional navigation hook; shelf
  edge behaviour; vertical movement preserving column; overlay stack with
  Escape closing the topmost; focus restoration; game-controller and common
  remote key mapping where available.
- **Out of scope:** Customisable keybindings (deferred).
- **Dependencies:** P4-02.
- **Implementation areas:** `apps/desktop/src/hooks`, `components/navigation`.
- **Acceptance criteria:**
  - [ ] A full walkthrough — Home → shelf → card → details → player → back →
        settings → back — completes with arrow keys and Enter only.
  - [ ] Left at a shelf's first card does not jump to an unrelated control.
  - [ ] Vertical movement preserves the nearest column, verified on shelves of
        unequal length.
  - [ ] Escape closes the topmost overlay before navigating away from the page.
  - [ ] Focus returns to the exact originating element after every overlay and
        after player exit.
  - [ ] Focus remains visible when the last input was a mouse.
- **Tests:** A navigation test suite using synthetic key events covering each
  criterion, including unequal-length shelves.
- **Risks:** Focus traps, hidden focus behind overlays, unreachable controls.
  This is the highest-regression area in the app: every later phase must re-run
  the walkthrough.
- **Status:** In progress — spatial focus, a global Escape-to-back hook
  (dialog/player-aware), Up/Down escaping text inputs into spatial navigation and
  unit tests landed 2026-11-21; a browser walkthrough of
  profile → Home → card → Details → Escape → nav → Movies was observed with zero
  console errors (see `progress.md`). The every-screen automated walkthrough and
  CP-4 sign-off remain **NOT RUN**.

### [~] P4-04 — Build details, settings and setup screens

- **Goal:** Everything the user needs is in-app; no second dashboard, ever.
- **Scope:** Details view with season picker and episode list; Settings sections
  (General, Library folders, Playback, Storage, Optional services, Diagnostics,
  About/licences/updates); Setup wizard screens wired to P1-06 preflight.
- **Out of scope:** Exposing any container management UI.
- **Dependencies:** P4-03.
- **Implementation areas:** `apps/desktop/src/pages/{Settings,Setup}`,
  `components/media`.
- **Acceptance criteria:**
  - [ ] Movies distinguish "available" from "missing local file" visibly.
  - [ ] Episode list shows watched and progress states and offers next-episode
        play.
  - [ ] Settings changes validate and persist; invalid values are rejected with
        a message, not accepted silently.
  - [ ] Diagnostics view shows runtime, database, storage and player status
        without exposing ports, tokens or full personal paths.
  - [ ] About lists third-party licences.
- **Tests:** Persistence round-trip for settings; a test that an invalid value
  is rejected; navigation tests for each section.
- **Risks:** Settings that write but never apply, or apply without persisting.
- **Status:** In progress — details/settings/setup screens built; settings validation and diagnostics-view tests remain.

### [~] P4-05 — Validate readability and performance at TV distance

- **Goal:** Prove the interface works at 3 metres, not just at desk distance.
- **Scope:** Measure minimum text size, focus visibility, contrast in situ, and
  frame behaviour while scrolling shelves on the lowest supported hardware.
- **Out of scope:** Visual redesign. Perf optimisation beyond recording.
- **Dependencies:** P4-04, P4-06, P4-07. Readability and frame behaviour are
  measured against the mock and re-measured in P4-08.
- **Implementation areas:** `docs/testing/`.
- **Acceptance criteria:**
  - [ ] No essential text renders below 16 CSS px in TV mode.
  - [ ] Focus is identifiable from the measured viewing distance, recorded as a
        yes/no with the distance stated.
  - [ ] Shelf scrolling on the lowest supported hardware holds a smooth
        experience — measured, with dropped-frame counts recorded.
  - [ ] Large artwork animation does not cause sustained frame drops on that
        hardware.
- **Tests:** Recorded on-device measurements at 1080p and, if available, 4K.
- **Risks:** Shipping an interface that only looks right on the developer's
  monitor.
- **Status:** In progress — DOM geometry checked at two sizes; contrast ratios, dropped-frame counts and TV-distance review NOT RUN.

### Checkpoint CP-4 — Usable from the couch — OPEN

- **Gate:** P4-01 … P4-08 complete.
- **Evidence required:**
  - [ ] Keyboard-only walkthrough video or step log across every screen.
  - [ ] Before/after focus-restoration proof for each overlay.
  - [ ] Contrast report with computed ratios.
  - [ ] Dropped-frame numbers on the lowest supported hardware.
  - [ ] A non-technical tester completes setup + first playback unassisted —
        record where they hesitated.
  - [ ] The contract is frozen and the mock is excluded from the production
        bundle (P4-06, P4-07).
  - [ ] Every data-dependent criterion is closed against the **real** backend
        (P4-08), or explicitly carried forward to CP-5.
- **If it fails:** Do not proceed to reliability work; usability defects found
  later are far more expensive because they touch shared components.

---

# Phase 5 — Reliability, security and storage

Purpose: the app cannot silently lose the user's data or leak their machine.

### [ ] P5-01 — Implement storage usage and configurable limits

- **Goal:** Visible, enforced storage boundaries.
- **Scope:** Used/free reporting per managed category, maximum storage and
  per-category limits, warn at 80% and pause at 90% (configurable), resume when
  space returns, orphaned temp detection.
- **Out of scope:** Any automated deletion.
- **Dependencies:** CP-4.
- **Implementation areas:** `crates/storage-policy`.
- **Acceptance criteria:**
  - [ ] The 80% warning and 90% pause thresholds fire at the configured values —
        tested by filling a fixture volume to those marks.
  - [ ] Crossing a limit never deletes anything.
  - [ ] Reported free space matches the OS within a documented tolerance.
  - [ ] Orphaned temp files are listed, not removed.
- **Tests:** Threshold tests against a size-capped fixture directory; a
  free-space agreement test.
- **Risks:** Wrong free-space math causing premature pause; limits that cannot
  be lowered later.
- **Status:** Not started

### [ ] P5-02 — Implement non-destructive cleanup preview

- **Goal:** The user sees exactly what would be deleted before anything is.
- **Scope:** Dry-run preview listing paths and item counts, explicit
  confirmation, move-to-managed-trash staging, retention period, audit log
  entry per action, retention disabled by default.
- **Out of scope:** Permanent deletion by default. Automated retention.
- **Dependencies:** P5-01, P3-04.
- **Implementation areas:** `crates/storage-policy`, `apps/desktop/src/pages/Settings`.
- **Acceptance criteria:**
  - [ ] Preview reports exact paths and counts and changes nothing — verified by
        file-count and mtime comparison.
  - [ ] Nothing is deleted without explicit confirmation; a cancelled
        confirmation performs zero operations.
  - [ ] Source/user-imported media is never eligible.
  - [ ] Unfinished, currently playing and unverified items are never eligible —
        one test per exclusion.
  - [ ] Every destructive action writes an audit event with actor, time, path
        and outcome.
  - [ ] Cleanup cannot escape the managed root: traversal and reparse-point
        attempts are rejected (tested).
- **Tests:** The exclusion matrix as automated tests; audit-log assertions;
  boundary-escape tests.
- **Risks:** Data loss. Treat any failing exclusion test as a release blocker
  and a stop-the-line event.
- **Status:** Not started

### [ ] P5-03 — Add crash and restart recovery

- **Goal:** Power loss or a crash leaves a recoverable app, not a broken one.
- **Scope:** Incomplete-scan recovery, queue/playback reconciliation, safe mode
  that starts the UI without optional services, bounded retries with backoff, a
  recovery command that never deletes data.
- **Out of scope:** Automatic repair of user media files.
- **Dependencies:** P5-01.
- **Implementation areas:** `crates/app-core`, `apps/desktop/src/app`.
- **Acceptance criteria:**
  - [ ] Killing the process during a scan, a write and a playback checkpoint
        each recover to a consistent state on next start (three tests).
  - [ ] Safe mode starts the app with optional services disabled and says so.
  - [ ] A failing optional service is retried with backoff and then stops —
        never restarted indefinitely.
  - [ ] Recovery never deletes rows or files.
- **Tests:** Simulated-kill integration tests for each interruption point; a
  retry-storm test asserting the retry count is bounded.
- **Risks:** Retry storms consuming CPU; a "recovery" path that deletes
  evidence.
- **Status:** Not started

### [ ] P5-04 — Run path traversal and filesystem boundary tests

- **Goal:** Demonstrated resistance to filesystem escape.
- **Scope:** A security test suite covering `..` traversal, absolute paths,
  UNC paths, junctions and symlinks, case-sensitivity tricks, Unicode
  normalisation, long paths, wildcard injection, and cleanup rooted outside
  managed storage.
- **Out of scope:** Kernel-level or driver-level attacks.
- **Dependencies:** P5-02.
- **Implementation areas:** `tests/security/`.
- **Acceptance criteria:**
  - [ ] Every escape vector has a test that asserts rejection **and** asserts no
        read/write occurred outside the root.
  - [ ] A canonicalisation-bypass attempt via a junction is rejected.
  - [ ] Tests run in CI and fail the build on regression.
- **Tests:** `tests/security/` suite; CI job.
- **Risks:** A partial fix that passes one vector and leaves another open.
- **Status:** Not started

### [ ] P5-05 — Review local network exposure and process permissions

- **Goal:** The app listens on nothing it does not need, and holds no excess
  privilege.
- **Scope:** Enumerate every listening socket during normal operation and during
  each optional feature; confirm loopback-only binding where required; confirm
  no admin rights are held; confirm no unnecessary capabilities; check firewall
  rules unchanged.
- **Out of scope:** Remote access features (out of MVP).
- **Dependencies:** CP-4.
- **Implementation areas:** `docs/security/`, `crates/platform-windows`.
- **Acceptance criteria:**
  - [ ] A documented socket inventory exists with expected vs observed state.
  - [ ] No port is bound to a non-loopback interface in any MVP mode.
  - [ ] The process runs without elevated privileges; verified by process
        inspection.
  - [ ] Firewall rules are unchanged before and after a full session.
  - [ ] Any unavoidable listener uses a per-install random credential, not a
        default one.
- **Tests:** `netstat`/`Get-NetTCPConnection` capture before/during/after;
  privilege check; firewall rule diff.
- **Risks:** An internal service accidentally binding `0.0.0.0` and exposing the
  household network.
- **Status:** Not started

### [ ] P5-06 — Build diagnostic export with redaction

- **Goal:** Support-quality diagnostics that are safe to share.
- **Scope:** One-click bundle (logs, config shape, DB schema version, health
  snapshot, optional runtime status), redaction of tokens, passwords, private
  URLs and personal paths, and consent-based inclusion of filenames.
- **Out of scope:** Automatic upload of any kind.
- **Dependencies:** P1-03, P4-04.
- **Implementation areas:** `crates/diagnostics`.
- **Acceptance criteria:**
  - [ ] A seeded bundle containing a token, a home path and a filename contains
        none of them after export (test asserts absence, not presence of a
        filter).
  - [ ] Bundle creation is entirely local; no network request is made.
  - [ ] Filenames appear only when the user opts in, and the opt-in text says so.
  - [ ] Bundle creation works with optional services disabled.
- **Tests:** Redaction regression test with a fixture bundle; a network-silence
  check.
- **Risks:** A diagnostic bundle becoming a privacy incident.
- **Status:** Not started

### [ ] P5-07 — Run low-disk, offline and power-interruption tests

- **Goal:** The failure modes users actually hit are survivable.
- **Scope:** Full-disk writes, disconnected network for the whole session,
  abrupt power interruption during scan/write/cleanup, and removable-drive
  removal mid-playback.
- **Out of scope:** Hardware fault injection beyond these scenarios.
- **Dependencies:** P5-03, P5-04.
- **Implementation areas:** `docs/testing/`.
- **Acceptance criteria:**
  - [ ] Full disk produces a clear message and no partial state corruption.
  - [ ] A complete offline session performs every local feature.
  - [ ] Power interruption during each of the three operations recovers cleanly.
  - [ ] Removing the drive mid-playback stops playback gracefully and never
        deletes the item.
- **Tests:** Recorded procedure per scenario with outcome and any data loss
  observed.
- **Risks:** Undiscovered corruption paths that only appear on real hardware.
- **Status:** Not started

### Checkpoint CP-5 — Trustworthy under failure — OPEN

- **Gate:** P5-01 … P5-07 complete.
- **Evidence required:**
  - [ ] Full security suite green in CI.
  - [ ] Socket inventory showing loopback-only.
  - [ ] Redaction proof for a seeded bundle.
  - [ ] Cleanup exclusion matrix green, with the audit log shown.
  - [ ] Power-interruption results table, one row per scenario.
  - [ ] Zero known data-loss defects open.
- **If it fails:** Do not package. A release gate in `Development.md` §17 cannot
  be waived by an agent.

---

# Phase 6 — Workspace hardening, optional providers and lifecycle verification

Purpose: harden and prove the required Docker/Compose boundary that Phase 1 brought
up, and add optional providers only if they are separately approved.

> **Sequencing note.** The required workspace itself is stood up in **P1-07** and wired
> to the host in **P1-08**, because Phase 2 and later are backend work and the backend
> *is* the `core` container. Building the boundary in Phase 6 would mean shipping
> product features on a boundary that had never been exercised. Phase 6 therefore
> hardens and verifies what P1-07/P1-08 established, and adds optional providers.

**Precondition:** CP-0 has human-reviewed the mandatory boundary, runtime distribution
evidence, playback evidence, host/container networking and resource profile. If any of
these is unresolved, stop before implementation.

### [ ] P6-01 — Audit and harden the required workspace's security controls

- **Goal:** Every control claimed in `docs/architecture/docker-workspace.md` is
  demonstrated, not asserted.
- **Scope:** A full pass over the running workspace: identity and privilege,
  image pinning, network exposure, mount scope, resource limits, health checks,
  restart policy, log rotation and secret redaction. Fix what fails.
- **Out of scope:** Adding services; changing the container topology.
- **Dependencies:** CP-5, and P1-07/P1-08 (the workspace exists).
- **Implementation areas:** `infrastructure/containers/`, `docs/security/`.
- **Acceptance criteria:**
  - [ ] Every line of `docs/security/compose-security-checklist.md` is checked
        against the running workspace and the outcome recorded.
  - [ ] `docker inspect` output for both services is captured and archived as
        evidence in `progress.md`.
  - [ ] A deliberate write to an undeclared path fails; a deliberate write to a
        declared path succeeds.
  - [ ] `probe` is confirmed to have no route off its internal network.
  - [ ] AppArmor is recorded as **absent** under WSL2 (R-17) and the
        compensating control is named — not listed as if active.
  - [ ] The `\wsl$` volume exposure is re-confirmed and the threat model still
        describes reality (R-15).
- **Tests:** The checklist as an executable script; adversarial write tests;
  `docker inspect` capture.
- **Risks:** A control that exists on paper only.
- **Status:** Not started

### [~] P6-02 — Optional lawful download provider adapter

- **Goal:** A provider interface with no bundled content sources and no
  unlawful defaults.
- **Scope:** Provider trait, explicit user configuration, directory boundaries,
  per-task and total limits, pause/resume/cancel/status, untrusted-input
  handling, loopback-only control interface, per-install credentials. Enabled
  only after a separate legal and security review.
- **Out of scope:** Bundled trackers, indexers, search services or content
  catalogues of any kind.
- **Dependencies:** P1-07, P6-01, and an accepted legal/security review (O-05).
- **Implementation areas:** `crates/downloads`.
- **Acceptance criteria:**
  - [ ] The adapter contains no hard-coded content source, tracker or indexer.
  - [ ] Feature is disabled by default and requires explicit configuration.
  - [ ] Downloads cannot write outside the managed download directory (tested).
  - [ ] Downloaded files are never executed.
  - [ ] Control interface binds loopback only with per-install credentials.
  - [ ] Credentials never appear in logs, source or plaintext config.
  - [ ] If a VPN-gated mode exists, a tunnel drop stops traffic — fail closed,
        never fall back silently.
- **Tests:** Boundary tests, credential-redaction test, fail-closed test.
- **Risks:** Legal exposure; a leaked control API; a misconfiguration turning
  the app into a piracy front-end. This task requires human legal sign-off
  before implementation begins.
- **Status:** [~] Reopened and implemented under ADR-0017/0018/0019 (human-accepted 2026-11-20) via the torrent-acquisition conversion plan, Tasks 1–7. Built: qBittorrent Docker sandbox on an egress `acquire` network with no published port, loopback-only WebUI client with secret-store credentials, named + signed host-internal acquisition routes, user-supplied magnet/`.torrent` Downloads UI, rolling two-episode buffer with 20 GiB cap, and coordinated cleanup of app-managed downloads only. Full gate green (fmt/clippy clean; 84 Rust passed/1 ignored, parity 24; 18 Vitest; frontend checks clean). Acceptance evidence: `docs/testing/acquisition-matrix.md`. **NOT RUN:** live authorized-torrent acquisition, real-media appearance/playback, live prefetch, live qB deletion, real retained-byte cap, and second-machine LAN non-reachability (no lawful source / human evidence gate). No checkpoint passed.

### [ ] P6-03 — Verify workspace shutdown, limits, recovery and persistence

- **Goal:** The required workspace runs reliably without data loss or unbounded
  resource use.
- **Scope:** Shutdown cleanliness, measured overhead against the P0-14
  baseline, recovery after a hard kill, data persistence across an image
  update, and uninstall behaviour.
- **Out of scope:** New features.
- **Dependencies:** P6-01.
- **Implementation areas:** `docs/operations/`, `docs/testing/`.
- **Acceptance criteria:**
  - [ ] Measured idle RAM overhead with services enabled is recorded and
        compared against the P0-14 baseline.
  - [ ] Killing the app leaves no orphaned containers or volumes (verified).
  - [ ] Recreating and upgrading the stack preserves database and config.
  - [ ] Uninstall removes the workspace but not user media — file-count proof.
  - [ ] A hard kill during a database write leaves a recoverable state, not a
        corrupt one.
- **Tests:** Shutdown/orphan checks, overhead measurements, upgrade-persistence
  test, uninstall test, kill-during-write test.
- **Risks:** Orphaned containers consuming memory after the app closes;
  a container upgrade that loses watch history.
- **Status:** Not started

### [~] P6-07 — Keyless IMDb posters

- **Goal:** Real movie/show posters from IMDb's keyless public suggestion JSON —
  cached locally, attributed exactly "Posters: IMDb", and fully offline once
  cached. No API key and no Python runtime.
- **Scope:** Keyless provider and URL allowlist (Task 1), core wiring through
  the existing hashed cache and loopback `GET /artwork/:key` route (Task 2),
  and the frontend attribution/privacy text. All existing artwork controls are
  preserved unchanged.
- **Out of scope:** Any scraper, indexer, catalogue or bundled content source;
  runtime HTML scraping; any path, token, host or API key in a renderer DTO;
  a full-library live IMDb seed.
- **Dependencies:** ADR-0025 (**Accepted (2026-11-21, human-approved)**)
  supersedes the artwork clauses of ADR-0011 and ADR-0023 only. IMDb ratings
  remain opt-in via offline dataset files; OMDb stays opt-in with a key; local
  NFO stays the offline fallback. ADR-0007 still governs the API token.
- **Implementation areas:** `crates/metadata`, `crates/core`, `apps/desktop/src`.
- **Acceptance criteria:**
  - [ ] Keyless: only a title and year go to IMDb's public JSON endpoint; no
        key, no Python, no HTML scraping. Image URLs must pass the
        `https://m.media-amazon.com/images/M/` allowlist before any request.
  - [ ] Hashed cache: 64-lowercase-hex key, 2 MiB per image, 64 MiB total,
        oldest-first eviction; 12-item-per-request cap; key validated before
        the filesystem join.
  - [ ] Attribution: the exact string "Posters: IMDb" is rendered where poster
        art is shown.
  - [ ] Offline fallback: absent, malformed or failed artwork degrades to the
        bundled placeholder; no view breaks.
  - [ ] No path, key or host in DTO: the renderer receives only a relative
        `/artwork/<64-hex>` URL and prefixes `apiBase`.
- **Tests:** Fake-fetch Rust tests with a pre-seeded JPEG cache byte file,
  loopback route tests, frontend `artFromDto` tests and all six frontend gates
  are PASS. A **full-library live IMDb seed is NOT RUN** (no live endpoint call
  was made; offline fakes only).
- **Risks:** The suggestion endpoint is unofficial and may change or be
  rate-limited; failures degrade to bundled art. The trust surface is bounded
  by the host allowlist, 12-item cap, 64-hex key validation, loopback bind and
  host-free DTO URLs. ADR-0025 is **Accepted (2026-11-21, human-approved)**;
  no ADR was self-accepted.
- **Status:** [~] Code complete for Tasks 1–2 with local gates green. No key,
  no Python. A full-library live IMDb seed is **NOT RUN**. ADR-0025 is
  **Accepted (2026-11-21, human-approved)**. No checkpoint passed.

### [~] P6-08 — Operator-supplied TMDB token

- **Goal:** Let the operator paste a TMDB read access token in Settings; store
  it in the existing Windows DPAPI secret store, inject it into `core` at
  startup, and use TMDB as the primary artwork source with keyless IMDb posters
  as fallback.
- **Scope:** Generic DPAPI secret helpers and named commands
  `provider_secret_set/has/clear` (only `kind == "tmdb"`), supervisor injection
  of `MEDIAHAVEN_TMDB_API_KEY`, `core` TMDB → IMDb → bundled fallback, and the
  renderer Settings field plus both exact attribution lines.
- **Out of scope:** Any site list, indexer, catalogue, Cardigann definition or
  Jackett/Prowlarr code; a new dependency or runtime; a new opt-in switch; any
  agent-run live TMDB call. Jackett/Prowlarr is a separate, later plan.
- **Dependencies:** ADR-0026 (**Accepted (2026-11-22, human)**) supersedes the
  opt-in clause of ADR-0023 for artwork only; ADR-0007 still governs the secret
  store, and ADR-0025's keyless IMDb posters remain the fallback.
- **Implementation areas:** `apps/desktop/src-tauri/src`, `crates/core`,
  `apps/desktop/src`.
- **Acceptance criteria:**
  - [ ] The token is optional and reversible: blank/missing means zero TMDB
        requests and keyless IMDb posters still work; Clear removes it.
  - [ ] The value is stored only in a distinct DPAPI blob; it never appears in
        a DTO, log, error string, the repo, or the renderer after save. The
        password field is cleared immediately on a successful save and Settings
        shows only `Configured` / `Not configured`. In production the value
        never reaches localStorage/sessionStorage; browser dev uses a
        session-only shim.
  - [ ] A new token reaches `core` only after an app restart, and the Settings
        UI states this.
  - [ ] Both exact attribution lines render where artwork is shown:
        "This product uses the TMDB API but is not endorsed or certified by
        TMDB." and "Posters: IMDb".
  - [ ] All existing artwork limits are unchanged: 2 MiB / 64 MiB hashed cache,
        12-item batch cap, 64-hex route validation, relative DTO URL, `onError`
        fallback.
  - [ ] Frontend gates typecheck / lint / test / check:tokens / build /
        check:bundle are PASS; no new npm dependency; Playwright config and
        `vite.config.ts` untouched.
- **Tests:** Fake-fetch Rust tests, loopback route tests and frontend tests
  (Settings save/clear/no-leak, dev shim, both attribution lines) are PASS.
  **A live token run is NOT RUN** — no operator token was supplied, no live
  TMDB request was made and no live TMDB poster was observed.
- **Risks:** TMDB may rate-limit or change its API; failure degrades to IMDb,
  then bundled art. The trust surface is the DPAPI identity, the named-command
  allowlist, the restart gate and the host-free DTO. ADR-0026 is **Accepted
  (2026-11-22, human)**; no ADR was self-accepted. No checkpoint passed.
- **Status:** [~] Code complete for Tasks 1–3 with local gates green. A live
  token run is **NOT RUN**. No new dependency, no push/merge/install.

### [~] P6-09 — TMDB discovery catalogue (display-only)

- **Goal:** Browse TMDB's popular/trending catalogue from the Movies, TV Shows
  and Anime pages using the operator's existing token, with real posters and
  no acquisition path.
- **Scope:** Named command `discover_catalog(kind, page)`, display-only cards,
  exact TMDB attribution, and a per-visit My library / Discover toggle. Anime
  is approximated with TMDB's Animation genre + Japanese original language
  filter; coverage is incomplete and stated in the UI.
- **Out of scope:** Any site list, indexer, catalogue file, Cardigann
  definition, auto-download, playback or local-library mapping; no new
  dependency, runtime or opt-in switch; no Playwright/vite config change.
- **Dependencies:** P6-08's operator token and ADR-0026; ADR-0027 (Accepted
  2026-11-22) records the display-only decision.
- **Implementation areas:** `crates/contract`, `crates/metadata`,
  `crates/app-core`, `crates/core`, `apps/desktop/src-tauri`, `apps/desktop/src`.
- **Acceptance criteria:**
  - [ ] `discover_catalog` is registered across all named-command surfaces
        (route parity now 25 commands); no token returns `available: false`.
  - [ ] Cards are display-only: no detail link, add, download or play action.
  - [ ] Poster URLs are relative `/artwork/<64-hex>` refs, prefixed with
        `apiBase` exactly once, with a guarded `onError`; attribution is exact.
  - [ ] `available: false` shows a quiet "add a TMDB token in Settings"
        message with a Settings link, no cards and no request loop.
  - [ ] Anime Discover states it uses TMDB's Japanese-animation filter and may
        miss anime; the local-mode Anime tag filter is unchanged.
  - [ ] Frontend gates typecheck / lint / test / check:tokens / build /
        check:bundle PASS; no new npm dependency.
- **Tests:** Fake-fetch metadata tests, loopback route tests and frontend
  component tests are PASS. A live TMDB catalogue run with a real token is
  **NOT RUN** — no live request was made and no live poster was observed.
- **Risks:** Anime coverage is an approximation. TMDB may rate-limit or change
  its API, degrading Discover to an empty section. No checkpoint passed.
- **Status:** [~] Code complete for Tasks 1–3 with local gates green. Live
  catalogue with token **NOT RUN**. No new dependency, no push/merge/install.

### [~] P6-10 — App startup on mount and Prowlarr inputs

- **Goal:** Opening the installed app starts its own backend, and Settings lets
  the operator supply a Prowlarr address and API key alongside the TMDB token.
- **Scope:** One host-only `runtime_start` Tauri command called on UI mount; a
  compiled-in compose definition staged at an absolute temp path; a non-blocking
  start that returns only fixed status strings. Extend the DPAPI secret allowlist
  to `tmdb` / `prowlarr_url` / `prowlarr_key` via one `account_for` helper,
  forward the two Prowlarr inputs through `WSLENV` into `core.environment`, and
  add matching Settings fields with Save/Clear and configured status.
- **Out of scope:** Any Prowlarr/Jackett orchestration, site list, indexer,
  catalogue or auto-download; a new dependency/runtime; a new renderer command
  (the contract stays at 25); any live WSL/Docker start in tests.
- **Dependencies:** ADR-0005 (named commands), ADR-0007/0014 (DPAPI secrets),
  ADR-0026 (TMDB token), ADR-0028 (**Accepted (2026-11-22, human instruction)**).
- **Implementation areas:** `apps/desktop/src-tauri/src`, `apps/desktop/src`,
  `infrastructure/containers/compose.yaml`.
- **Acceptance criteria:**
  - [ ] `runtime_start` returns only `ready` / `setup-required` / `failed`; no
        path, distro, env value or engine detail reaches the renderer. It does
        not block on `wait_ready`; readiness is detected by the UI's API calls.
  - [ ] The staged compose path is absolute and the file exists; no path is in
        the command's output.
  - [ ] All three kinds are accepted; unknown kinds are rejected with the fixed
        `unsupported provider` string before any store access. No value appears
        in a DTO, log, error string, source or the renderer after save.
  - [ ] Prowlarr variables are forwarded only when present, with `WSLENV`
        entries; compose declares both with `:-` defaults. Blank inputs leave
        the two-entry base environment byte-for-byte unchanged.
  - [ ] Settings shows text address / password key fields, Save/Clear and
        `Configured` / `Not configured` only; drafts clear on success. Browser
        dev uses a per-kind session-only shim tree-shaken from production.
  - [ ] Frontend gates typecheck / lint / test / check:tokens / build /
        check:bundle PASS; Rust fmt / clippy / lib test / all-targets check PASS;
        no new dependency.
- **Tests:** Rust RED then GREEN (52 lib tests), frontend targeted RED then GREEN
  (8 tests), full Rust and frontend gates green. **A live WSL/Docker start and
  a real Prowlarr connection are NOT RUN** — no engine consent or operator
  instance was supplied.
- **Risks:** A saved value reaches `core` only after an app restart (Settings
  says so). Engine output could echo a forwarded variable; redaction now masks
  all four provider assignments as defence in depth. No checkpoint passed.
- **Status:** [~] Code complete with local gates green. Live startup with a real
  runtime and Prowlarr instance **NOT RUN**. No new dependency, no
  push/merge/install.

### [~] P6-11 — Seeded sources and search from Details

- **Goal:** Ship the provider definitions inside the app, install them into the
  operator's own Prowlarr with one click, and let the user shortlist sources
  from a title's Details page and send a chosen one to the downloader.
- **Scope:** Five attributed provider definitions embedded with `include_str!`;
  host-internal `POST /host/indexers_seed` and `POST /host/source_search` routes
  reusing one injectable Prowlarr HTTP seam; two named Tauri host commands
  (`indexers_seed`, `source_search`); a `SourceResults` panel on Details with a
  "Find sources" button. Search parses Torznab XML with simple string scanning
  and returns at most ten path-free candidates. No new dependency.
- **Out of scope:** Bundled trackers, site lists, catalogues or unlawful
  acquisition guidance; auto-download, auto-selection or playback from search;
  a 26th renderer command (the contract stays at 25); live indexer calls in tests.
- **Dependencies:** ADR-0018 (qB boundary), ADR-0021 (R-01 source mapping),
  ADR-0024 (executable guard), ADR-0028 (Prowlarr inputs) and ADR-0029
  (**Accepted (2026-11-22, human instruction)**).
- **Implementation areas:** `crates/core/src/{indexers.rs,acquisition.rs,lib.rs}`,
  `crates/core/resources/indexers/`, `apps/desktop/src-tauri/src`,
  `apps/desktop/src`.
- **Acceptance criteria:**
  - [ ] Five definitions bundled verbatim with `ATTRIBUTION.md` naming both
        upstream repos; no tracker, indexer catalogue or site list is bundled.
  - [ ] `indexers_seed` adds the three Prowlarr-native definitions, skips the
        two custom ones, and returns counts/fixed reasons only — no URL or key.
  - [ ] `source_search` queries enabled indexers, filters by normalized title
        prefix plus year/season/episode when supplied, sorts by seeders desc,
        caps at ten, and returns only `{id, title, size, seeders, magnet}`.
  - [ ] Blank config, transport/parse failure and one failing indexer degrade
        without leaking a path, address, key or raw response text.
  - [ ] Details "Find sources" shows candidates only on request; choosing one
        calls `download_add` then `source_set` with fixed status text. No
        auto-selection and no playback path from the panel.
  - [ ] Frontend gates typecheck / lint / test / check:tokens / build /
        check:bundle PASS; Rust fmt / clippy / lib test / all-targets check PASS;
        contract stays at 25 commands; no new dependency.
- **Tests:** Rust RED then GREEN (29 lib tests) with fake Torznab XML; frontend
  `SourceResults` RED then GREEN (3 tests), full suite 19 files / 56 tests. All
  gates green. **Live Prowlarr seeding and live Torznab search are NOT RUN** —
  no operator indexer instance was supplied; no live request was made.
- **Risks:** Search quality depends on the operator's indexer health and title
  conventions; S/E detection uses string scanning and may miss unusual release
  names. A chosen source is a candidate until the existing executable/ffprobe/
  Defender gates run. No checkpoint passed.
- **Status:** [~] Code complete with local gates green. Live seeding and live
  search **NOT RUN**. No new dependency, no push/merge/install.

### Checkpoint CP-6 — Required workspace is contained — OPEN

- **Gate:** P6-01 … P6-03 complete; the mandatory backend boundary is verified.
- **Evidence required:**
  - [ ] Compose static security check green, and the full checklist executed
        against the running workspace with archived `docker inspect` output.
  - [ ] Host-state diff showing no unconsented change.
  - [ ] Orphan-container check after hard kill.
  - [ ] Overhead numbers recorded next to the P0-14 baseline.
  - [ ] Legal/security sign-off recorded for the provider adapter, or the
        optional adapter is explicitly deferred.
  - [ ] Required backend/inspection workspace is part of the v1 product and its
        persistent-state recovery is proven.
- **If it fails:** Do not ship v1. The required backend boundary is a release gate;
  defer optional providers, not the workspace.

---

# Phase 7 — Packaging and release

Purpose: a non-technical user installs, updates and uninstalls safely.

### [ ] P7-01 — Build a reproducible Windows installer

- **Goal:** One signed, reproducible artifact.
- **Scope:** Tauri bundler configuration, versioning, checksums, SBOM
  generation, and a documented build command.
- **Out of scope:** Auto-update delivery (P7-02).
- **Dependencies:** CP-5.
- **Implementation areas:** `infrastructure/packaging/windows`,
  `.github/workflows/release.yml`.
- **Acceptance criteria:**
  - [ ] Two builds from the same commit produce identical checksums, or the
        non-determinism is documented with its cause.
  - [ ] The installer installs per-user where possible and does not require
        admin rights for the normal path.
  - [ ] An SBOM is produced and attached to the release.
  - [ ] Third-party licences are bundled and visible in-app.
- **Tests:** Rebuild-and-compare; install on a clean VM.
- **Risks:** Signing cost and certificate handling; per-machine install forcing
  admin.
- **Status:** In progress — an unsigned per-user NSIS installer was **really
  built and verified** 2026-11-21: `MEDIA_HAVEN_0.1.0_x64-setup.exe` (~3.60 MiB)
  plus `SHA256SUMS.txt` and `THIRD_PARTY_NOTICES.md`; `build.ps1` gate exit 0.
  Package ID `com.mediahaven.desktop` and version 0.1.0 come from one source.
  Remaining: builds are **non-deterministic** across runs (documented, not
  fixed — half of the rebuild criterion is met by disclosure); **SBOM NOT RUN**
  (no tool installed without consent); clean-VM install unverified. Signing is
  deliberately out of scope for v1 (O-08, ADR-0016). No CP-7 pass.

### [ ] P7-02 — Test install, upgrade, rollback and uninstall

- **Goal:** The full lifecycle is safe for user data.
- **Scope:** Fresh install, upgrade over the previous version, failed-upgrade
  rollback, and uninstall with a clear choice about app data.
- **Out of scope:** Silent/mandatory updates.
- **Dependencies:** P7-01.
- **Implementation areas:** `docs/operations/`.
- **Acceptance criteria:**
  - [ ] Upgrade preserves database, config, watch history and queue — verified
        row counts and a playback-resume spot check.
  - [ ] A failed upgrade rolls back to the previous working version without data
        loss.
  - [ ] Uninstall asks explicitly about app data; both answers behave as stated,
        and neither deletes user source media.
  - [ ] Uninstall leaves no services, scheduled tasks or firewall rules behind.
- **Tests:** Lifecycle matrix on a clean VM, each step recorded.
- **Risks:** An uninstall that deletes user media; an upgrade that loses watch
  history.
- **Status:** Not started

### [ ] P7-03 — Verify signatures, checksums and dependency licences

- **Goal:** Supply-chain claims are true.
- **Scope:** Signature verification instructions, checksum publication,
  dependency and secret scanning in CI, and a licence inventory with
  attribution.
- **Out of scope:** Purchasing a certificate without approval.
- **Dependencies:** P7-01.
- **Implementation areas:** `.github/workflows/security.yml`, `docs/security/`.
- **Acceptance criteria:**
  - [ ] Dependency and secret scanning jobs run and fail the build on a
        high-severity finding.
  - [ ] Published artifacts have verifiable checksums and, if signed, a working
        verification procedure.
  - [ ] Licence inventory matches the actual dependency graph.
  - [ ] No secret appears in the repository or in build logs.
- **Tests:** Scanners run on a seeded violation and fail as expected.
- **Risks:** Unsigned binary triggering SmartScreen warnings; undisclosed
  copyleft dependency.
- **Status:** Not started

### [ ] P7-04 — Run the full Windows test matrix

- **Goal:** Honest, documented compatibility claims.
- **Scope:** Windows 10 (only if supported) and 11, x64 (and ARM64 only if
  claimed), on the hardware tiers from `Development.md` §9.1.
- **Out of scope:** Unsupported OS claims.
- **Dependencies:** P7-02.
- **Implementation areas:** `docs/testing/`.
- **Acceptance criteria:**
  - [ ] A results matrix covers install, first-run, library import, playback and
        uninstall per configuration tested.
  - [ ] Untested combinations are marked UNTESTED, never implied supported.
  - [ ] The published minimum requirements derive from measured results only.
- **Tests:** Recorded runs on each available configuration.
- **Risks:** Claiming support for hardware that stutters on playback.
- **Status:** Not started

### [ ] P7-05 — Publish setup guide, troubleshooting and release notes

- **Goal:** A non-technical user succeeds without support.
- **Scope:** Setup guide, troubleshooting with real error messages, privacy
  statement, uninstall instructions, known limitations, and release notes.
- **Out of scope:** Marketing site.
- **Dependencies:** P7-04.
- **Implementation areas:** `README.md`, `docs/setup/`, `docs/operations/`,
  `CHANGELOG.md`.
- **Acceptance criteria:**
  - [ ] Every documented step was executed as written during P7-04.
  - [ ] Troubleshooting entries quote actual messages the app emits.
  - [ ] The privacy statement matches observed network behaviour — the socket
        and request inventory is attached as evidence.
  - [ ] Known limitations are published, including unsupported codecs and
        untested hardware.
- **Tests:** A non-technical tester follows the guide unassisted; record where
  they get stuck.
- **Risks:** Documentation describing intended rather than actual behaviour.
- **Status:** Not started

### Checkpoint CP-7 — Shippable — OPEN

- **Gate:** All of `Development.md` §17 release gates pass.
- **Evidence required:**
  - [ ] Product, reliability, security, performance and documentation gates each
        checked with evidence.
  - [ ] Install/upgrade/rollback/uninstall matrix green.
  - [ ] `roadmap.md` and `progress.md` current at the release commit.
  - [ ] A non-technical user completes install → import → play → uninstall.
  - [ ] Zero open data-loss defects.
- **If it fails:** Do not publish. **This is the final product definition:** a
  user installs MEDIA_HAVEN on a spare Windows laptop, plugs it into a TV, adds
  their own folders, and browses and plays their library with a remote — fully
  offline, free, without an AI service, and without losing a single file.

---

## Deferred / future

Explicitly out of scope for the first release. Do not implement without a new
approved requirement and a spec update.

- Remote/headless streaming and multi-device access (reopens auth, TLS,
  patching, threat model).
- ~~Whole download workstream / O-05~~ **REOPENED 2026-11-20 by ADR-0017** —
  torrent acquisition of authorized media is now in scope; see
  `docs/superpowers/plans/2026-11-20-torrent-acquisition-conversion.md`.
- Mobile companion application.
- Multi-user profiles and cloud sync.
- Transcoding farm, Kubernetes, distributed anything.
- AI recommendations, AI media agents, any AI runtime dependency.
- Paid metadata providers or mandatory accounts.
- Automatic access to arbitrary network shares.
- Automatic permanent deletion of source media.
- Bundled trackers, indexers, content catalogues or piracy tooling of any kind.
- Customisable keybindings and themes beyond the token set.
- A separate media-server dashboard as a user-facing surface.

---

## Risks and open decisions

| ID | Item | Type | Impact | Next action | Trigger / deadline |
|----|------|------|--------|-------------|--------------------|
| R-01 | Playback engine unproven on target codecs | Risk | Stack-level; blocks Phase 3 | P0-12 codec matrix | Before CP-0 |
| R-02 | 4 GB profile stutters on playback | Risk | Changes minimum requirements | P0-14 measurement | Before CP-0 |
| R-03 | Runtime distribution licensing, networking or resource cost unsuitable | Risk | Changes supported runtime and hardware claims, not the mandatory boundary | P0-10 evidence + ADR-0001 | Before CP-0 |
| R-04 | Focus navigation regressions across phases | Risk | Product usability | Re-run walkthrough at every CP | Each checkpoint |
| R-05 | Completion logic misfires and later deletes media | Risk | **Data loss** | P3-04 adversarial suite | Before any retention ships |
| R-06 | Path escape via junction/reparse point | Risk | Security | P5-04 suite in CI | Before CP-5 |
| R-07 | Diagnostic bundle leaks a filename or path | Risk | Privacy | P5-06 redaction test | Before CP-5 |
| R-08 | WSL2/VM escape or guest-kernel exploit | Risk | Security; boundary | Keep the kernel updated; `probe` treated as hostile | Accepted — ADR-0001 |
| R-09 | No kernel-enforced storage quota on volumes | Risk | Resource exhaustion | Application-level budgets + eviction | Accepted — documented |
| R-10 | Same-user local process reads the API token | Risk | Local privilege | Loopback-only + token + HMAC | Accepted — out of scope |
| R-11 | Docker daemon is root inside the VM | Risk | Local privilege | VM is the boundary; no socket exposure | Accepted |
| R-12 | Malicious or compromised container image | Risk | Supply chain | Digest pinning; no `latest`; deliberate updates | Open — signing deferred |
| R-13 | WSL2 NAT networking assumption is wrong | Risk | **LAN exposure of the API** | P0-13 measures host reachability and LAN non-reachability | **Open — blocks CP-0** |
| R-14 | Playback forced into a container | Risk | Core experience | ADR-0002 (host placement) | Closed by design |
| R-15 | Named volumes are readable/writable from Windows via `\\wsl$` | Risk | Integrity/confidentiality of app data | Volume holds no secret; no other container can reach it | Accepted — documented |
| R-16 | Workspace shares a VM budget/daemon with unrelated workloads | Risk | Availability; blast radius | Dedicated WSL2 distro + dedicated daemon (ADR-0001) | Accepted — mitigated by design |
| R-17 | No AppArmor confinement under WSL2 | Risk | Security | Seccomp + cgroupns; `probe` networkless | Accepted — documented |
| R-18 | Mirrored WSL2 networking would expose published ports to the LAN | Risk | **LAN exposure** | Preflight rejects mirrored mode; NAT required | Accepted — documented |
| R-19 | Third-party decoder parses hostile media on the host | Risk | Code execution | ADR-0002 exception; OS mitigations; sandboxed renderer | Accepted |
| R-20 | Watch history readable by the same OS user | Risk | Privacy | Per-user app data and per-user distro | Accepted |
| R-21 | No HDR guarantee | Risk | Experience | Deferred with the playback ADR | P0-12 result |
| R-22 | Executable file written to the download root by a torrent | Risk | **Code execution** | Pre-download name guard (ADR-0024): add paused, reject executable-bearing/empty/unreadable file lists, remove task with `deleteFiles=false` before any resume; existing post-download probe/Defender gates remain mandatory | Guard implemented + fake-driven tests pass; live qB rejection **NOT RUN** (no lawful source) |
| O-01 | Project name / branding / package ID | **Decided 2026-11-18 / 2026-11-20** — MEDIA HAVEN is the name; package ID `com.mediahaven.desktop` (ADR-0016) | Blocks installer identity | Apply at P7-01 | Before P7-01 |
| O-02 | Windows 10 support claim | **Decided 2026-11-18** — Windows 11 x64 only for v1 | Support surface | Development.md and installer must state it | Before P7-04 |
| O-03 | Offline-indefinitely guarantee | **Decided 2026-11-20** — fully offline after install (ADR-0016); local playback/browsing never need a network | Metadata feature behaviour | Enforce opt-in egress in P2-06 | Before CP-2 |
| O-04 | Metadata provider selection and terms | **Accepted 2026-11-18** — TMDB primary, local NFO fallback (ADR-0011) | Legal/feature scope | Opt-in egress, attribution, secret-store key | Before P2-06 |
| O-05 | Download provider legal/security review | **Reversed and accepted 2026-11-20 (ADR-0017/0018/0019)** — authorized-media acquisition via qBittorrent is in scope; conversion Tasks 1–7 complete and ADR-0018/0019 controls verified by tests + boundary evidence (`docs/testing/acquisition-matrix.md`, `docs/security/boundary-evidence.md`) | Live lawful-source acquisition, real-media playback/cleanup and second-machine LAN non-reachability remain **NOT RUN** (no lawful source / human evidence gate); no checkpoint passed | Evidence recorded in `docs/testing/acquisition-matrix.md`; human CP review still owed | Design closed 2026-11-20; live gates open |
| O-06 | Windows Docker runtime distribution | **Accepted 2026-11-18** — dedicated WSL2 distro + Docker Engine (ADR-0001 Part 2) | Required backend packaging and lifecycle | P0-10/P0-13/P0-14 numbers still owed | Before CP-0 |
| O-07 | Whether to reuse an existing Docker Desktop engine instead of a dedicated distro | **Decided 2026-11-18** — no reuse; the product ships its own dedicated distro | Isolation vs install friction | Detection still reported to the user, never silent | Before CP-0 |
| O-08 | Installer signing certificate | **Decided 2026-11-20** — ship unsigned in v1, publish the SmartScreen caveat; a signed build needs a future human budget decision (ADR-0016) | Trust/SmartScreen | Document caveat at P7-03 | Before P7-01 |
| O-09 | libmpv LGPL vs GPL build | **Decided 2026-11-18** — LGPL build (ADR-0002 acceptance record) | **Project licence compatibility** | Confirm the shipped artefact really is LGPL-configured | Before CP-0 |

**Rule:** an agent may propose options for any `O-*` item and must record the
proposal in `docs/decisions/`. An agent may not mark one accepted. Only the
human partner accepts product, legal or irreversible decisions.
