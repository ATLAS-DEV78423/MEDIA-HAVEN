# ADR-0005 — Host ↔ container boundary: one loopback port, per-install token, no Docker access from the frontend

- **Status:** Proposed — blocked on P0-13 (port reachability measurement)
- **Date:** 2026-11-16
- **Related:** ADR-0001, ADR-0007, `docs/architecture/docker-workspace.md` §3

---

## Context

The UI is a Windows process. The backend is a container. Something has to cross that
boundary, and the boundary is where most local-app security mistakes happen: a backend
bound to `0.0.0.0`, an unauthenticated localhost API any process can call, or — worst —
the frontend holding Docker daemon access and effectively becoming root.

## Decision

### 1. The API is HTTP on loopback, one port, one container

`core` publishes exactly one port. The host supervisor discovers it at startup (a fixed
default, overridable, recorded in app config) and hands the base URL to the UI.

Rationale: HTTP is already the language of the Tauri command layer, is trivially testable
with `curl`, and needs no protocol work. A unix socket would be marginally better on
Linux and is not reachable from Windows — so it is not an option here, and pretending
otherwise would add a proxy for no gain.

**The publication must be `127.0.0.1:PORT` inside the WSL2 VM, and the VM must be in the
default NAT networking mode.** WSL2's mirrored mode is explicitly designed to make the
instance reachable from the LAN — Microsoft lists "Connect to WSL directly from your local
area network (LAN)" among its benefits — and in mirrored mode `127.0.0.1` is shared with
the Windows host rather than private to the VM. A mirrored `.wslconfig` therefore
converts a loopback-only publication into a LAN-reachable one without any `portproxy`
rule. The preflight detects a mirrored configuration and reports it as a blocking
condition rather than working around it.

### 2. Authentication is a per-install bearer token, plus a request signature

- On first run the host generates **256 bits from the OS CSPRNG**.
- It is stored in **Windows Credential Manager** (see ADR-0007), never in a file in the
  repo, never in an image, never in a log.
- It is injected into `core` at container start. `core` refuses every request without it.
- Every request carries `Authorization: Bearer <token>` **and** an HMAC-SHA256 over
  `timestamp || method || path || body-sha256`, keyed from the same secret.
  - Rejection window: ±30 s. Prevents replay of a captured request.
  - Comparison is constant-time.
- `core` has exactly one token. There are no roles, because there is one user.

Why the signature if the token is already secret: the token lives in a container env var
and may legitimately appear in diagnostics. The signature means a *leaked token from a log*
is not by itself enough to replay a captured request; it also makes accidental
cross-request confusion (wrong body, wrong path) fail closed rather than succeed weirdly.

### 3. Nothing else is exposed, and no container has Docker access

- The Docker socket is **never** mounted into any container. No container can start,
  inspect or remove another container.
- The Windows frontend never invokes `docker`. All lifecycle operations go through the
  **host supervisor**, a Rust module in the Tauri process, which runs `docker` *inside* the
  WSL2 distro via `wsl.exe`. There is no Windows `docker.exe` dependency and no TCP Docker
  endpoint (`2375`/`2376`) is ever enabled.
- The UI's capability surface is expressed as **named Tauri commands**, not as a generic
  "call the backend" passthrough. [`invoke("http_request", …)`](.) would hand the frontend
  the whole API; `invoke("library_add_root", …)` hands it one operation.

### 4. Request validation is not optional

- Every command has an explicit request type; unknown fields are rejected, not ignored.
- The backend re-validates everything. The frontend's validation is a UX affordance, never
  a security control.
- Paths arriving at the backend are validated by the path policy (lexical, then
  canonicalised, refusing reparse points) before any filesystem call.
- Body size limits, per-route timeouts and a request-rate ceiling are enforced in `core`.

### 5. Failure must be legible

| Failure | User sees |
|---|---|
| Docker not installed | Setup screen with what is needed and why; nothing installed without consent |
| Engine not running | One-click "start the engine", with the actual error text |
| Container unhealthy | Diagnostics with the last 50 log lines, secrets redacted |
| Port unreachable from host | Falls through the documented fallbacks (§3.1 of the workspace doc) and reports which one worked |
| Token rejected | Explicit "credential mismatch" state and a re-provision action, not a silent retry loop |

### 6. Health and readiness are distinct

- `/healthz` — process alive, always unauthenticated, returns nothing but a status.
- `/readyz` — database migrated, library roots mounted, storage budgets initialised.
  Requires auth. The UI waits for `readyz`, not `healthz`, before showing the library.

## Rejected alternatives

| Alternative | Why not |
|---|---|
| Named pipe / unix socket | Not reachable across the WSL2 boundary from Windows without a relay; the relay is more attack surface than an authenticated loopback port |
| gRPC | Adds codegen and a second protocol for no capability gain at this size |
| Mutual TLS | Meaningful gain over token+HMAC only if we also pin a client cert; the cert must live on disk, which is weaker than Credential Manager. Revisit if a second machine ever appears. |
| No auth, "it's localhost" | Any process or any web page the user visits can reach loopback. Rejected outright. |
| Frontend holds a Docker context | Turns any frontend XSS into full host compromise |

## Consequences

- One secret to provision, rotate and recover; rotation is a single supervisor action that
  restarts `core` with a new token.
- Two copies of the request schema (Rust host, Rust backend) that must not drift. Mitigated
  by generating both from one source and a contract test that fails on divergence.
- A same-user attacker who can read the token is out of scope — documented as R-10.
- The same-user attacker can also read and write the SQLite database through the WSL2
  `\\wsl$` share (R-15). That is accepted: the database holds the user's own watch
  history and preferences, not a credential. The token lives in Credential Manager, not
  in the database.

## Verification

- P0-13: host reaches the port; a second machine cannot; `netsh interface portproxy` is
  empty; no `2375`/`2376` listener exists; and `.wslconfig` is **not** in mirrored mode.
- P1-08: the four-case auth matrix (no token / wrong token / replayed signature /
  tampered body) fails closed, and there is no generic passthrough command.
- A test that calls every command without a token, with a wrong token, with a replayed
  signature, and with a tampered body — all must fail closed.
