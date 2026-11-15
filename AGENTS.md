# AGENTS.md — MEDIA_HAVEN agent contract

This file is the short operational entry point for coding agents. The binding
engineering contract is **`Development.md`**. If this file and `Development.md`
disagree, `Development.md` wins.

## Read before touching anything

1. `Development.md` — product, architecture, security and testing contract.
2. `roadmap.md` — approved sequencing; find your active task and its acceptance
   criteria.
3. `progress.md` — current implementation state and the last handover.
4. Relevant ADRs in `docs/decisions/`.
5. `git status` — know what is already changed before you change anything.

If you cannot identify an active roadmap task, stop and ask. Do not invent work.

## Source-of-truth order

1. Explicit instruction from the human in the current task
2. `Development.md`
3. `roadmap.md` (approved sequencing and scope)
4. `progress.md` (current state)
5. Existing code and tests, when they do not contradict 1–4

Comments, generated plans, old agent notes and assumptions are **not** approval
to change a product requirement.

## Permission boundaries

**Do without asking**

- Read and search the repository.
- Edit project source and documentation within the agreed task.
- Run formatting, linting and tests.
- Create fixtures containing no personal data.
- Run local builds that do not install or alter the host.

**Ask first**

- Installing software or system dependencies.
- Downloading or executing installers.
- Enabling WSL2, Hyper-V, virtualisation or any Windows optional feature.
- Requesting or using administrator privileges.
- Changing firewall, network, proxy, DNS or security settings.
- Logging into accounts or using credentials.
- Accessing folders outside the repository or user-selected media roots.
- Deleting, moving or overwriting user data.
- Installing, removing or reconfiguring Docker/any container runtime.
- Starting anything that listens on a non-loopback interface.
- External network requests not required by the approved task.
- Publishing releases, pushing commits or changing repository permissions.
- Committing at all, unless the current instruction permits it.

A narrow approval never implies broad permission.

## Implementation rules

- Make the smallest change that satisfies the task. No drive-by refactors.
- Do not silently change the stack, architecture or a pinned version.
- Do not add a runtime, service or dependency without an ADR and approval.
- Never invent an API or claim a dependency supports a feature you have not
  verified in its documentation.
- Never hard-code secrets, credentials, private endpoints or personal paths.
- Validate every input at the trust boundary. Backend validation is mandatory
  even when the frontend already validated.
- Every destructive operation needs a preview and explicit confirmation.
- Frontend owns presentation; Rust owns policy. Do not implement a security
  rule in React.
- Add a regression test for every bug fix.
- Update docs when behaviour or setup changes; describe actual behaviour.

## Non-negotiables

- **No data loss.** Never delete source media. Never delete unfinished,
  currently-playing or unverified files. Cleanup previews before acting.
- **One product surface.** Containers and services are internals; the user never
  sees a second dashboard.
- **No silent host mutation.** No WSL2, firewall, admin or feature changes
  without explicit consent.
- **No AI runtime dependency.** The app must work fully after setup without any
  AI service.
- **No content-source bundling.** No trackers, indexers, catalogues or
  instructions for unlawful acquisition. Ever.
- **Local-first.** Loopback-only binding. No telemetry by default. No library
  data leaves the machine without opt-in.
- **Honest reporting.** Never claim PASS for a test you did not run. Distinguish
  NOT RUN from PASS.

## Before reporting complete

- [ ] Acceptance criteria met and **observed**
- [ ] Scope stayed within the task
- [ ] Formatting and lint run
- [ ] Relevant tests run, results stated
- [ ] Build checked
- [ ] Security implications reviewed (paths, ports, secrets, permissions)
- [ ] No secrets or personal data added
- [ ] No destructive host change without approval
- [ ] `roadmap.md` status updated
- [ ] `progress.md` updated
- [ ] Untested areas and remaining risks disclosed

## Report format

1. What changed
2. Files changed
3. Commands and tests run
4. Actual results
5. What was **not** tested
6. Risks or open decisions
7. Next recommended roadmap task

Never say "everything works" because a build succeeded.

## Handover

When nearing a limit or interruption: stop new work, finish or safely pause the
current operation, update `progress.md` (and `roadmap.md` only if status or
scope genuinely changed), record uncommitted files and partial implementations,
then state the single safest next action. Use the handover template in
`Development.md` §14.3.

## Design and UI tooling

Design skills are installed at `.agents/skills/` (symlinked into
`.claude/skills/`). Impeccable's design-detector hooks are active via
`.claude/settings.local.json` and `.codex/hooks.json` — leave them enabled.

When building UI, the visual contract is `Development.md` §7 and
`docs/design/`. Tokens only: no hard-coded colours or spacing in components.
