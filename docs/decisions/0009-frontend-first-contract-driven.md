# ADR-0009 — Frontend-first delivery, built against a frozen command contract

- **Status:** Proposed — human review required
- **Date:** 2026-11-15
- **Deciders:** product owner (sequencing), architecture
- **Related:** ADR-0001, ADR-0005, ADR-0008, `docs/architecture/overview.md`, `roadmap.md` P4-06 … P4-08

---

## Context

The approved roadmap sequences the work backend-first: Phase 2 (library) → Phase 3
(playback) → Phase 4 (TV experience). The frontend therefore could not start until
CP-2 and CP-3 had passed.

The product owner has chosen a different sequence: **build the full frontend first, then
build the backend.**

This is a legitimate sequencing decision, and it is also a genuinely different *kind* of
work, not just a reordering. The frontend cannot talk to a backend that does not exist,
so building it first requires deciding what the frontend is allowed to know. That
decision has consequences — some of them good, one of them dangerous — and they need
writing down before any code depends on them.

## Decision

**The frontend is built first, against a frozen, typed command contract, with a
development-only mock transport behind a single seam. The real transport lands with the
host↔container boundary (P1-08).**

### The three pieces

1. **A typed contract (`apps/desktop/src/types/media.ts`).** The DTOs the frontend
   renders are defined once, in one place, with no dependency on where the data came
   from. This is not new invention: ADR-0005 already requires a generated schema shared
   between the host and the backend, and the "no generic passthrough" rule already
   implies named, typed operations. The frontend build simply makes that contract
   concrete **first**, which is strictly better than inventing it against a live server.

2. **A command layer (`apps/desktop/src/services/commands.ts`).** The frontend calls
   *named operations* — `library_home`, `library_search`, `playback_start` — never a
   generic `http_request`. This is ADR-0005's rule, enforced in the frontend rather
   than in the backend, which is a stronger position.

3. **A mock transport (`apps/desktop/src/services/mock/`).** In development the command
   layer is served by an in-memory fixture library. The swap to the real Tauri transport
   is a one-file change behind `services/transport.ts`.

### The rules that keep this honest

| Rule | Why |
|---|---|
| The mock is **development-only** and is excluded from the production bundle | A mock that ships is a fake product. The build must fail if mock code reaches production. |
| The mock is **not** a host-side backend fallback | `Development.md` §2.3 forbids running the backend outside the container boundary. The mock implements *no* policy: no path validation, no filesystem access, no database. It returns fixture objects. |
| The frontend **never** gets Docker access, a filesystem path it can act on, or a raw backend error | Unchanged from ADR-0005. The mock does not change this. |
| Every screen has a **loading, empty, offline, missing-artwork and error** state | The frontend is built before the data exists, so these states must be designed now rather than discovered later. |
| The contract is the **only** thing the frontend knows | Adding a field to a DTO without changing the contract is a build error, not a runtime surprise. |

### What the frontend-first order gets us

- The Apple TV-style interface — the product's most visible surface — is built and
  reviewed while the cheapest possible changes are still possible.
- The contract is designed against the *user's* model of the data rather than against
  whatever the database happens to hold. That is the right way round.
- Focus navigation, the documented highest-regression area, gets a real test suite
  before there is a backend to regress against.
- The backend then has a fixed target to satisfy, rather than the frontend adapting to
  whatever the backend produced.

### What it costs

| Cost | Why it is acceptable |
|---|---|
| The UI is first built against fixture data | Unavoidable, and the contract keeps the risk bounded. The DTOs are the risk, not the pixels. |
| Data-dependent acceptance criteria cannot be evidenced yet | They stay **open**, not passed. The roadmap records this explicitly rather than marking them done. |
| A contract change after the backend lands is more expensive than a UI change | Accepted. It is still cheaper than a UI change after a *backend design* is baked in. |
| The real transport (P1-08) is on the critical path to a usable product | Unchanged. The product is not shippable without it. |

## Alternatives considered

| Option | Pros | Cons | Verdict |
|---|---|---|---|
| Backend first, as originally sequenced | No fixture data; every screen shows real state | The most visible surface of the product is built last and most expensively | Rejected by the product owner |
| Frontend first, with the contract written ad hoc as pages are built | Fastest start | The contract drifts into the components; the backend inherits a mess | Rejected |
| Frontend first, against a frozen contract with a mock (this ADR) | Contract-first; testable; UI reviewed early; backend has a fixed target | Needs discipline about the mock boundary | **Chosen** |
| Frontend first, with a real containerised backend built in parallel | Real data throughout | Two agents (or one agent split across a boundary) changing the same contract concurrently; defeats the purpose of sequencing | Rejected |

## Consequences

**Positive**
- The interface is designed, reviewed and tested while it is cheap to change.
- The command contract is a first-class artifact, not a by-product of the backend.
- Focus navigation and accessibility are tested against a real DOM, with real fixtures.

**Negative / accepted**
- The roadmap's phase numbering no longer equals execution order. `roadmap.md` now
  states the execution order explicitly instead of implying it.
- Some Phase 4 acceptance criteria are data-dependent and cannot pass until the
  backend lands. They are marked as blocked, not as complete.

## Evidence

- `Development.md` §2.3 (mandatory container boundary, no host-side backend fallback).
- ADR-0005 §3 (named commands, no generic passthrough, no Docker access in the
  frontend).
- `docs/architecture/overview.md` ("One authority rule": the backend decides; the
  frontend presents).
- `roadmap.md` P4-06 (typed contract), P4-07 (development mock), P4-08 (integration
  against the real backend).

## Revisit trigger

If the mock turns out to have grown policy, filesystem access or a database — i.e. if
it has become a host-side backend — this ADR is void and the mandatory boundary
(ADR-0001) has been breached.
