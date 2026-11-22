# ADR-0028: App startup wiring and Prowlarr inputs

- **Status:** Accepted (2026-11-22) — human instruction recorded below.
- **Date:** 2026-11-22
- **Deciders:** human partner (2026-11-22 instruction), contributing agent (scribe)
- **Related:** ADR-0005 (host/container boundary, named commands), ADR-0007
  (secrets), ADR-0014 (DPAPI host token), ADR-0026 (operator TMDB token),
  `docs/superpowers/plans/2026-11-22-app-startup-and-inputs.md`

## Context

The installed app did not start its own backend, and Settings could only accept
a TMDB token. The human instructed (2026-11-22): "completely plan out and fix
the backend and wire everything cleanly ... plug and play after they give their
inputs like their Prowlarr URL and their api key for TMDB". The backend must
start from the host on UI mount, and the operator must be able to supply a
Prowlarr address and API key alongside the existing TMDB token. Jackett/Prowlarr
itself, indexers and catalogues remain a separate, later plan (ADR-0026).

## Decision

The Tauri host gains one host-only command, `runtime_start`, called once on UI
mount. It loads the compiled-in compose definition, stages it at an absolute
temp path (a Windows-relative path does not resolve inside the WSL distro), and
starts the workspace through the existing `Supervisor::ensure_running`. It then
returns immediately with one fixed status string — `ready`, `setup-required` or
`failed`. It deliberately does **not** block on a readiness poll: the UI detects
readiness through its normal signed API calls. No path, distro name, environment
value or engine detail crosses this boundary.

The provider secret allowlist is extended from `tmdb` to exactly three kinds:
`tmdb`, `prowlarr_url` and `prowlarr_key`. One `account_for(kind)` helper maps a
kind to a distinct DPAPI account, and all three named commands use it; an
unknown kind is rejected before any store access. The supervisor forwards the
two Prowlarr inputs as `MEDIAHAVEN_PROWLARR_URL` and
`MEDIAHAVEN_PROWLARR_API_KEY`, with matching `WSLENV` names, only when values
are present. Blank values add no environment entry and leave the no-input case
byte-for-byte unchanged. `core` declares both variables with `:-` defaults so
the workspace stays startable before configuration.

Settings gains a Prowlarr address (text) field and a Prowlarr API key (password)
field, each with Save/Clear and a configured-status indicator, mirroring the TMDB
token field. Draft inputs are cleared on success, saved values are never
rendered, and all notices are fixed strings. Values go straight to the DPAPI
store through the named-command bridge; browser dev uses a per-kind,
session-only shim that production builds tree-shake out.

The renderer command contract stays at 25 commands (`runtime_start` is a
host-only Tauri command), and no dependency, runtime, site list or indexer is
added.

## Alternatives considered

| Option | Pros | Cons | Verdict |
|---|---|---|---|
| Host-only `runtime_start`, non-blocking, fixed status | No path/secret in output; UI owns readiness; contract stays 25 | UI must tolerate a starting backend | **Accepted** |
| Block on `wait_ready()` inside the command | Single call "confirms" readiness | Holds the command open past the startup deadline and blocks mount | Rejected |
| Pass the compose path relative to the repo | No staging step | Does not resolve inside WSL | Rejected |
| Store Prowlarr inputs in config/localStorage/DTO | Easy | Leaks secrets; violates ADR-0007/0014 | Rejected |
| Bundle Prowlarr/Jackett or an indexer list | No operator input needed | Violates ADR-0018; new attack surface; separate plan | Out of scope |

## Consequences

Positive: opening the app starts the backend, and all three operator inputs are
saved locally and reused automatically. Negative: a newly saved value reaches
`core` only after an app restart, which Settings states. The host stages one
small compiled-in compose file in the temp directory; no source media or user
data is touched. Engine-output redaction is widened to mask all forwarded
provider variables in an assignment, keeping secrets out of user-facing errors.

Preserved: loopback-only binding, fail-closed corrupt-blob handling, fixed error
strings, no host feature/admin change, no telemetry and no AI runtime
dependency.

## Evidence

- Human instruction 2026-11-22 quoted above; recorded by the agent as scribe,
  not self-accepted.
- `cargo test -p mediahaven-desktop --lib`: RED (20 compile errors on the new
  test seams), then GREEN (52 passed, 0 failed). Frontend targeted tests: RED
  (4 failed / 4 passed), then GREEN (8 passed). No live WSL/Docker start.
- Full gate results are in
  `.superpowers/sdd/2026-11-22-app-startup-and-inputs/task-2-report.md`.

## Revisit trigger

A later plan adds Prowlarr/Jackett orchestration or indexers, changes the
provider kind set, or moves backend startup off the host. Reopen with a
superseding ADR rather than quietly changing this one.
