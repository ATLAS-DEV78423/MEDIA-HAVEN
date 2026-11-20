# Documentation Reconciliation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring every project document into agreement with the code and the observed test evidence at HEAD `7d48507`, so status is honest and agents stop re-deriving stale facts.

**Architecture:** Five tasks with strictly non-overlapping write surfaces. Tasks 1–4 each own a disjoint set of Markdown files and run in parallel; Task 5 is a read-only code check that writes only its report into the SDD workspace. A controller review follows every task. No production code, config, dependency, or git history is changed.

**Tech Stack:** Markdown, PowerShell, `git diff --check`, `grep`, the existing `npm`/`cargo` verification commands (already observed green at HEAD).

**Spec:** `AGENTS.md`, `Development.md`, `roadmap.md`, `progress.md`, and the four 2026-11-19 audit reports recorded in the SDD workspace ledger.

## Global Constraints

- Source-of-truth order: current human instruction > `Development.md` > `roadmap.md` > `progress.md` > existing code/tests.
- Only the human partner may move an ADR to **Accepted** or pass a checkpoint. An agent records `Proposed` / `NOT RUN` honestly. Never mark CP-0…CP-7 as passed.
- `PASS` / `FAIL` / `NOT RUN` must describe an observed run. No doc may claim a test result that was not run in this repository at this commit.
- Do **not** commit, push, install anything, start Docker, change Windows features, or touch host configuration. `AGENTS.md`: "Committing at all, unless the current instruction permits it."
- Do not touch Playwright configuration, Playwright plugin state, `vite.config.ts`, or any production code in order to make a check pass.
- Preserve history: where an earlier section is wrong, replace the stale claim but add a short dated correction line noting what it used to say. Do not delete dated historical sections.
- No credentials, tokens, personal paths, or private endpoints in any document.
- Use the existing docs structure. Do not create new top-level documentation files.

---

### Task 1: Reconcile `roadmap.md`

**Files:**
- Modify: `roadmap.md` only.

**Interfaces:**
- Consumes: the verified HEAD facts and task-completion mapping in the controller brief.
- Produces: a roadmap whose current milestone, task markers, Status lines, checkpoint headings and gate lists match observed reality. No other file is touched by this task.

- [ ] **Step 1: Replace the "Current milestone" block (currently lines 73–84) with:**

```markdown
## Current milestone

- **Phase:** No checkpoint has passed. Work is concurrent across Phase 1 foundation
  (WS-A host boundary), Phase 2 library, Phase 3 playback and Phase 4 frontend —
  the product owner chose frontend-first (ADR-0009).
- **Goal:** Close the container boundary (A6 → A8 → A9, which unblocks P1-07/P1-08)
  and build real playback (B2/B3, which unblocks P3-01/P3-02). Both remain open.
- **Exit criteria:** the next checkpoint reached passes only on observed evidence
  recorded in `progress.md`, including a human-readable checkpoint summary.
- **Owner/agent:** unassigned. Any agent picking this up must first read
  `Development.md`, this file and `progress.md`, then confirm scope with the
  human partner.
- **Note:** CP-0 is still OPEN. Zero of its evidence items are satisfied — every
  one requires a measurement or a human sign-off, not a document.
- **Note:** a `[~]` marker means code or partial evidence exists, NOT that the
  acceptance criteria are met. `[x]` is reserved for checkpoint-verified tasks.
- **Evidence:** `progress.md` 2026-11-19 entry records the live verification at
  HEAD `7d48507` (Rust/frontend gates green; no engine or hardware evidence run).
```

- [ ] **Step 2: Change the eight checkpoint headings from `### ✅ Checkpoint CP-N — name` to `### Checkpoint CP-N — name — OPEN`** (CP-0 at line 411, CP-1 646, CP-2 857, CP-3 1012, CP-4 1251, CP-5 1429, CP-6 1537, CP-7 1660). The `✅` marker asserts a pass that has not happened.

- [ ] **Step 3: Apply this status-marker mapping.** Change the checkbox and the `Status:` line for each task listed. Leave every other task untouched.

| Task | Checkbox | New `Status:` line (verbatim) |
|---|---|---|
| P1-02 | `[~]` | `In progress — CI workflow and H1/H2/H3 tests exist and pass locally; a green GitHub Actions run has not been demonstrated.` |
| P1-07 | `[~]` | `Code complete, unproven — compose/Dockerfile/mount-override code exists; no live engine build or docker inspect evidence (needs consent).` |
| P1-08 | `[~]` | `In progress — named commands, DPAPI token, supervisor, HMAC forwarder and auth/contract tests exist; live engine + token-forwarding validation NOT RUN.` |
| P2-04 | `[~]` | `In progress — Movies/Shows/Library/Search screens built and browser-checked against the host-run backend; data-dependent criteria remain open.` |
| P2-05 | `[~]` | `In progress — local search implemented and timed at crate level; 10k-item frontend check and wildcard-input proof remain open.` |
| P2-06 | `[~]` | `In progress — local NFO reader landed 2026-11-18; TMDB opt-in provider, cache, egress flag and UI toggle remain.` |
| P2-07 | `[~]` | `In progress — backend table, routes, persistence and Watchlist screen exist; file-count, restart and queue-independence tests remain.` |
| P3-03 | `[~]` | `In progress — pure resume policy and checkpoint persistence exist; engine wiring and four exit-path tests remain.` |
| P3-04 | `[~]` | `In progress — domain completion rules exist with unit tests; persisted completion signal and full adversarial suite remain.` |
| P3-06 | `[~]` | `In progress — queue screen, durable reorder, removal history and crash-consistency tests exist; Up Next and restart proof remain.` |
| P4-01 | `[~]` | `In progress — token system and literal-check gate implemented; computed contrast ratios and TV-distance review NOT RUN.` |
| P4-02 | `[~]` | `In progress — hero/shelves/cards built and browser-checked; fixture-driven shelf-order and empty-state tests remain.` |
| P4-03 | `[~]` | `In progress — spatial focus implementation and unit tests exist; full keyboard-only walkthrough across every screen NOT RUN.` |
| P4-04 | `[~]` | `In progress — details/settings/setup screens built; settings validation and diagnostics-view tests remain.` |
| P4-05 | `[~]` | `In progress — DOM geometry checked at two sizes; contrast ratios, dropped-frame counts and TV-distance review NOT RUN.` |
| P4-06 | `[~]` | `In progress — crates/contract is the single source for 24 commands and 31 DTOs; parity tests pass; contract not frozen at a checkpoint.` |
| P4-07 | `[~]` | `Superseded — development mock was replaced by a host Tauri command surface and a dev-only signed HTTP client (A5/A7).` |
| P4-08 | `[~]` | `In progress — frontend verified against the host-run backend; containerised backend, 10k fixture and failure-mode suite remain.` |
| P6-02 | `[ ]` | `Deferred — the whole download workstream is out of v1 by human decision O-05 (2026-11-18). Do not implement without a new approved requirement.` |

- [ ] **Step 4: Fix two self-referential dependencies.** At line 559 change `P1-06 (\`preflight.ps1\`)` to `P0-05 (runtime validation)`. At line 1133 change `P4-01, P0-08 prototype learnings` to `P0-08 prototype learnings`.

- [ ] **Step 5: Fix two gate lists.** At line 859 change `P2-01 … P2-06 complete` to `P2-01 … P2-07 complete`. At line 1014 change `P3-01 … P3-05 complete` to `P3-01 … P3-06 complete`.

- [ ] **Step 6: Verify.** Run `git diff --check` and `git diff -- roadmap.md`. Grep for `✅ Checkpoint` (expect 0 matches) and for `no backend` / `no Rust code` (expect 0 matches in this file).

- [ ] **Step 7: Report.** Write the task report to the SDD workspace `task-1-report.md`: files changed, exact edits applied, verification commands and their exit codes, concerns. Do not commit.

---

### Task 2: Reconcile `progress.md`

**Files:**
- Modify: `progress.md` only.

**Interfaces:**
- Consumes: the verified HEAD facts and live test results in the controller brief.
- Produces: a `progress.md` whose header, current state, verification table, next actions and handover match reality, plus one new dated 2026-11-19 section. Other docs are owned by other tasks.

- [ ] **Step 1: Replace the `## Last updated` block (lines 3–5) with:**

```markdown
## Last updated

2026-11-19 (documentation reconciliation + live verification at HEAD `7d48507`;
earlier sections of this file are retained as history and are explicitly
superseded where they disagree with the 2026-11-19 entry at the end).
```

- [ ] **Step 2: Replace the `## Current state` block (lines 7–32) with:**

```markdown
## Current state

The repository holds the approved engineering contract, the checkpoint-driven
roadmap, the documentation baseline, a working React frontend, a working Rust
backend, a Tauri host and a CI workflow. No checkpoint has passed: CP-0 evidence
is still unmeasured and every later checkpoint is open.

Built and verified at HEAD `7d48507`: React TV UI wired to the host through named
Tauri commands; Rust workspace of seven crates including `app-core`, `core`,
`media-library`, `metadata`, `persistence`, `contract` and `mediahaven-desktop`;
SQLite migrations, scanner, watchlist, durable queue, settings/profiles; DPAPI
per-install token, WSL supervisor and HMAC-signed command forwarder; CI with
contract-drift and dependency-graph assertions.

Written but NOT RUN on a live engine: the `core`/`probe` images, the rendered
Compose override, mount scope, token forwarding into `core`, boundary evidence
and all container-based checkpoints. These need explicit Docker/WSL2 consent.

Playback is still simulated in the renderer (`SIMULATION_RATE = 4`); there is no
`crates/playback`, no mpv adapter and no stream route. TMDB provider, artwork
cache, diagnostics bundle, safe mode, traversal CI suite, storage policy and
installer are not built.

Superseded: earlier text in this file claimed "the backend does not yet exist",
"No Rust workspace, no database" and "src-tauri is still empty". Those claims
were true only for the 2026-11-15 baseline and were corrected on 2026-11-18 and
again on 2026-11-19.
```

- [ ] **Step 3: Replace the `## Active task` block (lines 34–47) with a single active task: "Documentation reconciliation + code check (2026-11-19); no product feature is active until ADR-0015 is accepted and engine consent is given." Include the branch `master`, HEAD `7d48507`, and the commit list of the six newest commits exactly as printed by `git log -6 --oneline`.

- [ ] **Step 4: Replace the `Known working commands` block (lines 139–148) with the actual commands:**

```markdown
## Known working commands

- Frontend dev server: `cd apps/desktop && npm run dev` (port 5173)
- Frontend: `npm run typecheck`, `npm run lint`, `npm test`, `npm run check:tokens`,
  `npm run check:bundle`, `npm run build`
- Rust: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`
- Dependency graph (read-only): `bash tests/dependency_graph.sh`
- Compose parse (no daemon): `docker compose config --quiet` — exits 1 with four
  "required variable" errors when the token/digest env is absent (fail-closed)
- Preflight (read-only): `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/windows/preflight.ps1`
- Dev backend: `MEDIAHAVEN_DEV=1 MEDIAHAVEN_SEED=1 MEDIAHAVEN_DATA_DIR="$PWD/.mediahaven-dev" cargo run -p mediahaven-core`
- NOT available yet: any live `docker build` / `docker compose up` / installer command.
```

- [ ] **Step 5: Replace the `## Verification` table (lines 150–173) with a table recording the 2026-11-19 live results:** Rust formatting **PASS**, clippy **PASS**, `cargo test --workspace` **PASS** (107 tests passed, 1 ignored — the pre-existing 10k search budget test), frontend typecheck **PASS**, lint **PASS**, Vitest **PASS** (11/11 across `App.test.tsx`, `transport.test.ts`, `QueuePage.test.tsx`), token check **PASS** (61 files, 0 violations), build **PASS** (91 modules), git tree clean at start of run. Add rows marking still-NOT-RUN: engine-backed evidence, hardware playback matrix, TV-distance review, GitHub Actions green run, installer lifecycle.

- [ ] **Step 6: Replace `## Blockers and failures`, `## Next actions` and `## Agent handover`** (lines 175–186, 204–217, 253–280) so they state: blockers are engine consent, ADR-0015 acceptance, O-08/package ID and remaining ADR acceptances; next agent action is A6 engine build → A8 boundary evidence → A9 mount proof, or playback B2/B3 after ADR-0015 acceptance; handover records HEAD, clean tree, all-green local gates, and the explicit NOT RUN list.

- [ ] **Step 7: Append a new dated section at the end of the file:**

```markdown
## 2026-11-19 — documentation reconciliation and live verification

Four parallel read-only audits (code state, document state, governance/consent,
test/evidence gaps) were run, plus a live verification pass at HEAD `7d48507`.

Observed live results (exit code 0 for every command): `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`
(107 passed, 1 ignored), `npm run typecheck`, `npm run lint`, `npx vitest run`
(11/11), `npm run check:tokens` (61 files, 0 violations), `npm run build`
(91 modules). `git status --short` was empty when the run began.

Rulings made by the controller: no worktree was created for this docs-only change
because merging it back would need consent the current instruction does not give;
parallel agents were used because each owns disjoint files, which satisfies the
conflict-avoidance purpose of the one-agent-per-task rule; no ADR was self-accepted
and no checkpoint was marked passed.

NOT RUN and still blocking: Docker/WSL2 engine evidence, live token forwarding,
hardware playback matrix, TV-distance review, GitHub Actions run, installer matrix.
```

- [ ] **Step 8: Verify.** Run `git diff --check`. Grep for `no Rust code exists`, `src-tauri/` is still empty`, `backend does not yet exist` (expect each either absent or only inside a dated superseded-correction sentence).

- [ ] **Step 9: Report.** Write `task-2-report.md` in the SDD workspace with files changed, verification output and concerns. Do not commit.

---

### Task 3: Reconcile the completion plan, ADR index and `Development.md` §18

**Files:**
- Modify: `docs/superpowers/plans/2026-11-18-backend-completion.md`
- Modify: `docs/decisions/README.md`
- Modify: `Development.md`

**Interfaces:**
- Consumes: the verified task-completion mapping in the controller brief.
- Produces: a plan whose status and "where the build is" table are true; an ADR index whose "still open" paragraph matches the index rows; a §18 that marks settled decisions as settled without changing §20's authority.

- [ ] **Step 1: In the completion plan, replace line 10** (`**Status:** draft for human review — no task in this plan has been executed.`) with:

```markdown
**Status:** active plan. Implemented in code: A1–A5, A7, A9 (render logic), C1–C2,
D1–D2, H2–H3. Written but NOT RUN on a live engine: A4 live cycle, A6 build/digests,
A8 boundary evidence, A9 mount-scope proof, C1 token forwarding. Not started:
B2–B6 (except pure domain logic in B4/B5), D3–D4, F1–F6, G1–G4. See `progress.md`
2026-11-19 entry for observed evidence.
```

- [ ] **Step 2: In the completion plan, replace the "Where the build actually is (observed 2026-11-18)" table (lines 36–48) with:**

```markdown
| Area | Observed state at HEAD `7d48507` (2026-11-19) |
|---|---|
| Rust workspace | Seven crates: `app-core`, `core`, `media-library`, `metadata`, `persistence`, `contract`, `mediahaven-desktop`. `cargo test --workspace` green. |
| API | 26 routes: `/healthz` public + 25 token/HMAC protected. Unknown fields rejected; 11 auth-matrix tests pass. |
| Persistence | SQLite migrations, roots, media, watchlist, durable queue + history, settings, profiles. |
| Scanner | Real filesystem scan, stable ids, path-escape refusal, duplicates reported; NFO sidecar metadata stored. |
| UI | React 19 + Vite, wired to the host through 24 named Tauri commands; dev-only signed HTTP client for browser work. |
| Playback | `playback_start` returns metadata only; renderer runs a simulated 4× clock. No `crates/playback`, no stream route. |
| Queue | Screen, route, nav, add/remove/reorder, durable reorder and removal history implemented and tested. |
| Downloads | `downloads_list` is read-only. Whole workstream deferred out of v1 (O-05). |
| Tauri host | Window, DPAPI token, WSL supervisor, mount override renderer, command surface and signed forwarder implemented. |
| Docker | Dockerfiles, compose and override renderer written but never built or run; no digests, no `docker inspect` evidence. |
| CI | Workflow + contract-drift + dependency-graph + H2/H3 tests present and green locally; no GitHub Actions run recorded. |
```

- [ ] **Step 3: In the completion plan, update these task headers in place:** A3 → `### A3 — Per-install token provisioning — COMPLETE (2026-11-18)`, A5 → `### A5 — Named command surface — COMPLETE (2026-11-18)`, A7 → `### A7 — Renderer transport swap — COMPLETE (2026-11-18)`, A9 → `### A9 — Media root mount rendering — IMPLEMENTED IN CODE, live validation NOT RUN (2026-11-18)`. Add one line directly under A9's header: `> Label note: "C1" in `progress.md` means token injection into core; "C1" in this plan means the Queue screen. They are different tasks.` Do not mark A4 or A6 COMPLETE.

- [ ] **Step 4: In `docs/decisions/README.md`, replace lines 33–38 with:**

```markdown
**Still open, each needing a human answer before code depends on it:** ADR-0004
(local playback boundary, P0-01), ADR-0005 (transport/auth, P0-13), ADR-0008
(deployment boundary, P0-07), ADR-0010 (library deviations), ADR-0015 (playback
descriptor, dated 2026-11-19), and the O-03 offline guarantee. ADR-0001 Part 2
and ADR-0002 are **Accepted (provisional)** and become fully binding only when
their P0-10/P0-11/P0-12/P0-13/P0-14 evidence is observed and human-reviewed.
ADR-0006 and ADR-0007 are **Accepted (provisional)**, with ADR-0007's token
mechanism amended by the Accepted ADR-0014. ADR-0011 (TMDB primary, local NFO
offline fallback, opt-in network) was accepted by the human partner on
2026-11-18 and is no longer open.
```

- [ ] **Step 5: In `Development.md` §18, replace items 1, 2, 3, 6, 8 and 10 with the following, leaving items 4, 5, 7 and 9 unchanged:**

```markdown
1.  **Playback engine:** DECIDED 2026-11-18 — mpv via JSON IPC, LGPL build,
    host-side placement (ADR-0002, O-09). The shipped artefact must still be
    confirmed LGPL-configured (P0-12) before release.
2.  **Windows Docker runtime/provider:** Docker Engine-compatible runtime and
    Compose are mandatory (ADR-0001 Part 1). Distribution DECIDED 2026-11-18 —
    dedicated WSL2 distro, no reuse of an existing engine (ADR-0001 Part 2,
    O-06/O-07). P0-10/P0-13/P0-14 measurements are still owed.
3.  **Playback licence:** DECIDED 2026-11-18 — LGPL mpv build (O-09). A hard
    gate on CP-0 remains: confirm the actual artefact is LGPL-configured.
6.  **Metadata providers:** DECIDED 2026-11-18 — TMDB primary with local NFO
    offline fallback, network opt-in and off by default (ADR-0011, O-04).
8.  **Windows 10 support:** DECIDED 2026-11-18 — Windows 11 x64 only for v1
    (O-02). Do not claim Windows 10 or ARM64.
10. **Project branding/name:** DECIDED 2026-11-18 — "MEDIA HAVEN" is the project
    and app name (O-01). The package/bundle ID is still needed before P7-01.
```

- [ ] **Step 6: Verify.** Run `git diff --check`. Grep the three files for `no task in this plan has been executed`, `No workflow`, `no UI`, `not yet finalised` (expect 0 matches).

- [ ] **Step 7: Report.** Write `task-3-report.md` in the SDD workspace. Do not commit.

---

### Task 4: Reconcile the user-facing and test-status docs

**Files:**
- Modify: `README.md`
- Modify: `tests/README.md`
- Modify: `infrastructure/packaging/windows/README.md`
- Modify: `docs/testing/test-matrix.md`

**Interfaces:**
- Consumes: the verified HEAD facts and live test results in the controller brief.
- Produces: docs that describe the runnable app and the actual test layout, without removing the Playwright e2e reference in `tests/README.md`.

- [ ] **Step 1: In `README.md`, replace lines 17–19 with:**

```markdown
> **Status: functional prototype, no checkpoint passed.** The React UI, Rust
> backend, Tauri host and local tests run today. Real playback, the container
> boundary, reliability/security work and packaging are still open. See
> `progress.md` for the 2026-11-19 evidence and the exact NOT RUN list.
```

- [ ] **Step 2: In `README.md`, change the heading `## Planned stack` to `## Stack`, and replace lines 43–47 with:**

```markdown
The runtime distribution is decided as a dedicated WSL2 distro with Docker
Engine (ADR-0001 Part 2, accepted 2026-11-18); the shipped images and boundary
evidence are not yet built. The host/container transport (ADR-0005) and the
playback descriptor (ADR-0015) remain Proposed and must not be treated as
binding. If the required runtime is unavailable, backend-dependent functions
remain unavailable; there is no host-side backend fallback.
```

- [ ] **Step 3: In `README.md`, replace lines 62–73 with a contributor quick start listing the commands verified green on 2026-11-19:** `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and from `apps/desktop`: `npm run typecheck`, `npm run lint`, `npm test`, `npm run check:tokens`, `npm run check:bundle`, `npm run build`, `npm run dev`. Keep the read-only preflight command. State that no Docker command is runnable without explicit consent.

- [ ] **Step 4: In `tests/README.md`, replace line 6 (`**Status: empty.** Suites are created by the roadmap tasks that need them.`) with:**

```markdown
**Status: partially populated.** Cross-cutting suites currently present:
`tests/dependency_graph.sh` (H3 assertion script), `tests/test-media/README.md`
(gitignored fixtures placeholder), plus crate-level suites that carry the
boundary work: `crates/core/tests/auth.rs`, `crates/contract/tests/{parity,route_parity}.rs`,
`crates/media-library/tests/{roots,scanner}.rs`, `crates/metadata/tests/nfo.rs`,
`crates/persistence/tests/{migrations,repository}.rs`, `crates/app-core/tests/domain.rs`
and `apps/desktop/src/**/*.test.tsx`. `tests/security/`, `tests/performance/` and
`tests/e2e/` are not yet created.
```

- [ ] **Step 5: In `tests/README.md`, replace the `## Running (once the harness exists)` block (lines 29–36) with:**

````markdown
## Running

```bash
cargo test --workspace                    # observed PASS 2026-11-19 (107 passed, 1 ignored)
bash tests/dependency_graph.sh            # read-only assertion script, H3
cd apps/desktop && npx vitest run         # observed PASS 2026-11-19 (11/11)
cd apps/desktop && npm run check:bundle   # dev-token/signing markers absent from dist/
npx playwright test                       # e2e harness — NOT RUN, suite not yet built
```

Results must be recorded in `progress.md` as PASS / FAIL / **NOT RUN**.
`NOT RUN` is never reported as passing. See `docs/testing/test-matrix.md`.
````

- [ ] **Step 6: In `infrastructure/packaging/windows/README.md`, replace lines 31–36 with:**

```markdown
## Blocking decisions before this directory is populated

- O-01 residual: the project name is settled as MEDIA HAVEN (2026-11-18); the
  application/package ID is still needed before P7-01.
- O-08: installer signing certificate — signed vs unsigned is a human budget
  decision and is still open.

Do not invent a package ID or a signing decision to unblock packaging. Ask.
```

- [ ] **Step 7: In `docs/testing/test-matrix.md`, insert directly after line 8:**

```markdown
**Observed on 2026-11-19 at HEAD `7d48507`:** Rust unit/integration suites PASS
(107 tests, 1 ignored), frontend component suites PASS (11/11), token/bundle
checks PASS, CI workflow present but no GitHub Actions run recorded. The
Security, E2E and Performance layers below are specified but **NOT RUN** as
cross-cutting suites; the path-escape and auth cases currently live in
`crates/media-library/tests/roots.rs` and `crates/core/tests/auth.rs`.
```

- [ ] **Step 8: Verify.** Run `git diff --check`. Grep for `Status: pre-implementation`, `Status: empty`, `Nothing is runnable yet`, `neutral placeholder` (expect 0 matches). Confirm the string `npx playwright test` is still present exactly once in `tests/README.md`.

- [ ] **Step 9: Report.** Write `task-4-report.md` in the SDD workspace. Do not commit.

---

### Task 5: Code check (read-only)

**Files:**
- Create: `.superpowers/sdd/2026-11-19-docs-reconciliation/code-check-report.md` only.
- Modify: none.

**Interfaces:**
- Consumes: HEAD `7d48507` and the already-observed live verification results.
- Produces: a written code check with findings and a clear PASS/NOT RUN statement. No production code is edited by this task.

- [ ] **Step 1: Re-run read-only checks** and record exact exit codes: `git status --short`, `git diff --check`, `cargo fmt --all -- --check`, `bash tests/dependency_graph.sh`. Do not start Docker, installs, or a dev server.

- [ ] **Step 2: Inspect the six newest commits** (`git log -6 --oneline`, then `git show --stat` on each) for code-quality and safety issues: the queue durability change, ADR-0015, H2 route-parser change, CI workflow, queue screen and mount override renderer. Look specifically for: a passthrough command, a raw path or token in a renderer-facing DTO, an unvalidated filesystem path, a test that cannot fail, a change to `vite.config.ts` made to satisfy a test.

- [ ] **Step 3: Write the report** with sections: `Checks run (command → exit code)`, `Commit inventory`, `Findings (Critical / Important / Minor / none)`, `Playwright plugin status (untouched)`, `What was NOT checked`. State explicitly that this is inspection, not a claim that new tests were run, and that the live test suite was already run separately at this HEAD.

- [ ] **Step 4: Verify** the report file exists and `git status --short` is still clean apart from the four documents owned by Tasks 1–4.

- [ ] **Step 5: Report.** Write `task-5-report.md` in the SDD workspace with the report path and a one-line verdict. Do not commit.
