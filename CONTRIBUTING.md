# Contributing

## Before you start

Read `Development.md` (the contract), `roadmap.md` (what is being built and in
what order), `progress.md` (what actually works) and `AGENTS.md` (rules). If you
are an AI agent, those four files are mandatory reading before any edit.

## Workflow

1. Pick a task from `roadmap.md` with its dependencies satisfied. Do not start a
   task whose blocker is unresolved.
2. Create a branch: `feat/P1-03-structured-logging`, `fix/P3-04-completion-seek`.
3. Make the smallest change that satisfies the acceptance criteria.
4. Add or update tests. A bug fix without a regression test is incomplete.
5. Run the checks for the area you touched.
6. Update `roadmap.md` status and `progress.md` with real results.
7. Open a pull request that names the task ID.

## Commit messages

Conventional Commits, scoped by area:

```
feat(playback): persist resume position on pause
fix(storage): reject cleanup targets outside the managed root
docs(decisions): add playback engine ADR
test(queue): cover reorder with duplicate entries
chore(ci): pin rust toolchain
```

Reference the roadmap task in the body: `Refs: P3-03`.

## Code standards

### Rust

- `cargo fmt` and `cargo clippy -- -D warnings` must pass.
- Domain logic lives in `crates/app-core/domain` and has no I/O imports.
- `thiserror` for domain errors; `anyhow` only at application boundaries.
- No `unwrap()`/`expect()` on user-controlled input paths.
- Validate at the boundary; do not trust the frontend.
- Every filesystem path crossing a trust boundary is canonicalised and checked
  for containment.

### TypeScript / React

- Strict TypeScript. No `any` in production code.
- Components consume design tokens; no colour or spacing literals.
- Product policy lives in Rust, not in a component.
- Every interactive element is keyboard-reachable and has an accessible name.
- Tests cover empty, loading, offline, missing-artwork and error states.

### PowerShell

- Read-only unless the script's stated purpose is to change something.
- `Set-StrictMode -Version Latest` and `$ErrorActionPreference = 'Stop'`.
- Every destructive or host-changing script documents what it changes and
  requires explicit confirmation.
- No `Invoke-Expression` on downloaded content.
- No secrets in script parameters.

## Documentation

Documentation must describe **actual** behaviour. If code and docs disagree,
that is a defect. Fix one, and say which in the pull request.

- Behaviour or setup change → update the affected doc in the same change.
- Decision made → an ADR in `docs/decisions/`, not a comment in code.
- New open question → add it to `roadmap.md` as `[?]`. Never resolve it silently.

## What will be rejected

- Scope creep beyond the task.
- A silent stack, dependency or architecture change.
- Deleting user data paths without preview and confirmation.
- A security rule implemented in the frontend.
- Hard-coded secrets, tokens, personal paths or real media titles.
- Tests asserted as passing without being run.
- Adding containers or services to "make a demo work".
- Bundled content sources of any kind.

## Reviewing

A reviewer checks, in order:

1. Does it satisfy the acceptance criteria in `roadmap.md`?
2. Does it stay in scope?
3. Does it respect the layering in `docs/architecture/overview.md`?
4. Does it hold the guarantees in `SECURITY.md`?
5. Are the tests real, and do they fail without the fix?
6. Are `roadmap.md` and `progress.md` updated honestly?

## Reporting

End every task report with what changed, files changed, commands run, actual
results, what was **not** tested, risks, and the next recommended task.
