# ADR-0007 — Secrets live in Windows Credential Manager and are injected at runtime

- **Status:** Proposed
- **Date:** 2026-11-16
- **Related:** ADR-0005, `docs/architecture/docker-workspace.md` §4

---

## Context

The app needs at least three secrets:

1. the API token shared between the host and `core`,
2. any opted-in metadata-provider API key,
3. any opted-in download-provider credentials (out of v1 scope, but the mechanism must
   not need redesigning later).

None may appear in an image, in the compose file, in the repository, in a log, or in a
crash dump.

## Decision

| Secret | Generated | Stored | Reaches the container via |
|---|---|---|---|
| API token | by the host, 256-bit CSPRNG, on first run | Windows Credential Manager, target name `MEDIA_HAVEN/api-token` | environment variable set by the supervisor at `compose up` time |
| Provider API key | user pastes it; validated by a single test call | Windows Credential Manager | environment variable, only in `core` |
| Future download credentials | same pattern | Windows Credential Manager | separate container only, never `core` |

### Rules

- **No secret in a file inside the repo.** `.env` files are gitignored and are not the
  storage mechanism; the Credential Manager is.
- **No secret in an image.** Images are built from a Dockerfile that never `COPY`s a secret
  and never takes one as a build arg.
- **No secret in a log.** The supervisor installs a redaction filter over: token, provider
  keys, `Authorization` headers, and any `*_KEY`/`*_TOKEN` environment value. Container
  logs are passed through the same filter before they reach the diagnostics screen.
- **No secret in a crash report.** Error types carrying credentials use a `Debug` impl that
  prints `[redacted]`.
- **Rotation is one action.** The supervisor regenerates the token, restarts `core` with
  it, updates the Credential Manager entry, and invalidates in-flight requests.
- **Compose receives secrets through the environment, not a mounted file.** Docker secrets
  (`/run/secrets`) would be the textbook choice, but they require either Swarm mode or
  `file:`-backed secrets that must exist on disk — worse than the environment on a
  single-user machine where the container's environment is inside the VM and readable only
  by root in that VM.

### Honest limits

Environment variables are visible to `docker inspect` and to anything that can read the
daemon's metadata. On this machine that means: root in the WSL2 distro. That is the same
trust level as the daemon itself, which already controls the containers. No boundary is
lost that was actually holding. This is recorded as R-11 rather than papered over.

If the threat model later includes a hostile local user with WSL2 root, this ADR must be
revisited (Docker secrets with an on-disk file and restricted ACLs, or a keyring in the
container).

## Consequences

- Credential Manager access requires the Windows user's session, which is exactly the
  intended authorisation.
- Testing needs an injectable secret source, so the code must not call the OS directly from
  business logic — a `SecretProvider` trait with a fake implementation in tests.
- Uninstall must offer to clear the Credential Manager entries (and must not leave an
  orphaned token behind silently).

## Verification

- `docker inspect core` shows env vars — accepted and documented.
- `git grep -i "token\|api_key" -- '*.rs' '*.ts' '*.yaml'` finds no literal secrets.
- Log-redaction test: a deliberate token in an error path must not appear in the captured
  log output.
- P1-08: the token is provisioned from Credential Manager at container start and never
  appears in the repository, an image, or the named volume.
