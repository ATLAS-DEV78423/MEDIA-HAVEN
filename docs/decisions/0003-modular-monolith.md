# ADR-0003: Modular monolith, not microservices

- **Status:** Accepted (derived from `Development.md` §3.1) — the *deployment* boundary is reconciled by ADR-0008
- **Date:** 2026-11-15
- **Deciders:** human partner (per `Development.md`), contributing agent
- **Related:** `docs/architecture/overview.md`; `Development.md` §2.5, §3.1; ADR-0008

> **Addendum (2026-11-16):** the Decision below says the monolith is "hosted in a single
> Tauri process". That was true when Docker was optional. Docker is now mandatory in v1
> (ADR-0001 Part 1), which fired this ADR's own revisit trigger. The module split, the
> crate boundaries, the dependency direction and the rejection of microservices all stand
> unchanged. What changed is *where the monolith is deployed*: one codebase, deployed in
> two units — host-side crates in the Tauri process, backend crates in the `core`
> container — over the authenticated boundary in ADR-0005. See ADR-0008. This addendum
> is an annotation on history, not a rewrite of the decision.

## Context

The product needs clear internal boundaries but runs on one laptop for one
household. Process separation would add IPC failure modes, extra memory, extra
startup time and deployment complexity to a product whose primary constraint is
low resource use on 4–8 GB hardware.

## Decision

Build a **modular monolith**: one Rust workspace with crate-level boundaries for
`domain`, `application`, `ports`, and infrastructure adapters, hosted in a single
Tauri process. Modules communicate through traits (ports), not over the network.

Crate boundaries exist to enforce dependency direction and to make a future
extraction possible — not to create deployable services.

## Alternatives considered

| Option | Pros | Cons | Verdict |
|---|---|---|---|
| Modular monolith | Low overhead; simple debugging; single process; fast iteration | Boundaries must be enforced by discipline and CI, not the network | **Chosen** |
| Microservices | Independent deploy; hard boundaries | Memory, startup, IPC latency, ops burden on a laptop | Rejected — cost has no matching need |
| Single crate, no modules | Simplest start | Domain becomes entangled with I/O; untestable | Rejected — fails testability requirement |

## Consequences

- Dependency direction is checked in CI (no `domain` → infrastructure imports).
- A crate may be extracted later if a measured need appears; until then, adding
  a process is a decision requiring a new ADR.
- Testability improves: domain logic is unit-testable with no I/O.
- Risk: crate sprawl. Crates are added when a real module needs one, not in
  advance (`Development.md` §6.2: "avoid empty scaffolding").

## Evidence

`Development.md` §3.1 mandates the module split; §2.5 caps the process count at
three for the core product.

## Revisit trigger

Only if a measured requirement demands a separate process — for example a
headless streaming mode, which is itself out of scope for v1.
