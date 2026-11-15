# ADR-0008 — Modular monolith across the Windows host and the Docker backend

- **Status:** Proposed — human review required
- **Date:** 2026-11-16
- **Deciders:** architecture; human partner approves
- **Related:** ADR-0001, ADR-0003, ADR-0005, `docs/architecture/overview.md`, `Development.md` §2.2, §3.1

---

## Context

ADR-0003 was written before Docker became mandatory. It says the modular monolith is
"hosted in a single Tauri process". That statement is now wrong in one specific way, and
ADR-0003's own revisit trigger fires: "Only if a measured requirement demands a separate
process".

The mandatory Docker boundary (ADR-0001) is exactly such a requirement, and it has now
been approved by the product owner.

ADR-0003 is **not** superseded. Its substance — one Rust workspace, crate-level
boundaries, dependency direction inward only, no microservices, no network between
modules — is unchanged and still correct. What changes is that the monolith is now
*deployed* across two places: the Windows host and the `core` container. This ADR
reconciles the two without weakening either.

## Decision

**The modular monolith is one Rust workspace and one codebase. It is deployed in two
units: the host-side crates run in the Tauri process, and the backend crates run in the
`core` container. The two units communicate only over the authenticated loopback API
defined in ADR-0005. There is no shared memory, no shared process and no shared volume
between them.**

### What this means concretely

| Question | Answer |
|---|---|
| Is the codebase still one codebase? | Yes. One Cargo workspace, one repository, one `Cargo.lock`. |
| Is there one process? | No. Two processes, in two trust domains, across one authenticated boundary. |
| Are modules still separated by crates? | Yes, and this is now the *only* boundary between host and container code — there is no "shared library" crate that both sides link against, because a crate linked into both would be a crate whose code runs in both trust domains. |
| Do modules still talk over traits (ports)? | Yes, within a unit. Across the unit boundary, the port becomes an HTTP client/server pair generated from one schema. |
| Does the host still talk to the container over anything but the API? | No. No shared volume, no Docker socket, no host filesystem mount into the container. |
| Do we still avoid microservices? | Yes, emphatically. Two deployment units is not a microservice architecture. There is no service discovery, no per-service versioning, no inter-service chat. |

### Why the split falls where it does

The split line is not arbitrary. It follows the trust boundary:

- **Host-side crates** (`app-core` command surface, `platform-windows`, `playback`,
  `runtime-adapter`, `diagnostics`, UI-facing DTOs) do things that require the Windows
  host: native windows, the display and GPU, the audio endpoint, the Windows Credential
  Manager, and the Docker/WSL lifecycle.
- **Container-side crates** (`domain`, `application`, `persistence`, `media-library`,
  `storage-policy`, `metadata`, `queue`) own the product rules, the database and the
  filesystem operations that must be sandboxed. `domain` and `application` are
  host-agnostic and could be compiled into either unit — which is exactly why they must
  not be allowed to depend on anything host-specific or container-specific.

The `domain` and `application` crates have **no dependency on either** the Tauri
command layer or the HTTP transport. That is the property that makes the split cheap to
move and cheap to test, and it is enforced by a dependency-direction check in CI
(P1-02), the same check ADR-0003 already required.

### The rule that keeps this from becoming a distributed system

> If a change makes the host and the container need to agree on something at runtime
> beyond one request/response, that change is a design smell. It must be reworked so the
> agreement is either entirely on one side, or entirely expressed in the shared schema.

Concretely: no distributed transactions, no cross-unit cache coherence, no two-phase
commit, no event bus, no "the host asks the container, the container calls back into the
host" loops. The API is a request/response boundary and behaves like one.

## Alternatives considered

| Option | Pros | Cons | Verdict |
|---|---|---|---|
| Modular monolith deployed across host + container | One codebase; the trust boundary is explicit; no microservices | Two deployment units; a schema to keep in sync | **Chosen** |
| Keep everything in the Tauri process, containerise nothing | Simplest; no IPC | Rejected: violates the mandatory Docker boundary (ADR-0001) | Rejected by the product owner |
| Keep everything in the container, including the UI | One process | No display, no GPU, no audio endpoint, no native window | Rejected — ADR-0002 |
| Split the backend into several services (microservices) | Independent scaling | Enormous operational cost on a single laptop; violates ADR-0003 | Rejected |
| One process, two threads, same binary, running on the host | Simplest IPC | No filesystem or process isolation at all | Rejected — defeats the purpose of the container boundary |

## Consequences

**Positive**
- The security boundary and the module boundary are the same boundary, so there is one
  place to audit instead of two independent ones.
- The backend can be restarted, upgraded or replaced without interrupting playback
  (ADR-0002).
- The domain and application layers stay portable and unit-testable with no I/O.

**Negative / accepted**
- Two copies of the request schema (Rust host, Rust backend) that must not drift.
  Mitigated by generating both from one source and a contract test that fails on
  divergence (also noted in ADR-0005).
- The host-side unit is not sandboxed from the Windows host, and cannot be. See ADR-0002
  for the controls that apply and R-19 for the residual risk.
- A single contributor can now accidentally reach across the boundary. The dependency
  direction check in CI is what stops this, and it must be treated as a release gate
  rather than a nicety.

## Evidence

- `Development.md` §3.1 mandates the module split; §2.2 mandates the container boundary.
- ADR-0001 Part 1 (Docker mandatory in v1), ADR-0003 (module split, revisit trigger
  fired), ADR-0005 (the boundary and its schema).

## Revisit trigger

If a third deployment unit is ever proposed, or if the host/container boundary needs
bidirectional streaming rather than request/response, this ADR must be revisited — that
is the point at which the design stops being a modular monolith and becomes something
else, which needs its own decision and its own threat model.
