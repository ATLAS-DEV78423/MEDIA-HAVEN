# ADR-0004: Local HDMI playback first

- **Status:** Not started — create and approve via `roadmap.md` P0-01
- **Date:** —
- **Deciders:** human partner
- **Related:** `Development.md` §1.5, §18 item 9; `roadmap.md` P0-01

> **This file is a placeholder for a decision the human partner must make.** It
> is intentionally not pre-filled with an answer. An agent may draft the
> context and alternatives; only the human partner can accept it.

## Context

`Development.md` §1.5 states the first release prioritises playback on the same
laptop connected to the TV, and that remote/headless streaming is a future
capability. §18 item 9 lists remote access as out of MVP.

Shipping remote streaming would add: authentication, TLS termination, network
exposure, patching obligations, multi-device session state and a materially
larger threat model — none of which exist in the current design.

## Decision

_Pending. Expected: the MVP is a local HDMI appliance; remote streaming is
deferred and requires a new spec before implementation._

## Alternatives considered

| Option | Pros | Cons |
|---|---|---|
| Local HDMI only (proposed) | Small threat model; offline; simple; fits target hardware | One screen, one viewer |
| Local + LAN streaming | Same-household second screen | Auth, TLS, session state, network exposure |
| Full remote streaming | Access anywhere | Patching, accounts, abuse surface, support burden |

## Consequences

_To be completed when the decision is made._

## Evidence

`Development.md` §1.5 and §18 item 9. No measurements required; this is a
product-scope decision, not a technical one.

## Revisit trigger

A confirmed requirement for headless operation or a second device — which must
arrive as a spec change, not as an implementation shortcut.
