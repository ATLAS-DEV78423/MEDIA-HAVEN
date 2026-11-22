# Security check — 2026-11-22

Run with procedures from the local cybersecurity skills library:
- `implementing-secret-scanning-with-gitleaks`
- `performing-docker-bench-security-assessment`
- `conducting-api-security-testing`

Scope: our own repository, our own Windows machine, our own loopback backend.
No third-party system was attacked. No state-changing exploit was attempted.

## Short answer

**No, not everything is fully wired.** The security controls that exist are in
place and passed their checks. Several things are still only built or tested
with fakes: a live download, the packaged app's cold start, a real browser
view, and a live check that an executable-bearing torrent is refused.

## 1. Secret scan

Gitleaks is **not installed** on this machine. The procedure's prerequisite
was not met, so I did not install it. Instead I ran the equivalent direct
checks:
- Searched tracked source (`.rs`, `.ts`, `.tsx`, `.yaml`, `.json`, `.toml`) for
  secret-looking literals: **no matches**.
- Searched the entire git history for the three real keys used during this
  project (TMDB v3 key, TMDB JWT, Prowlarr key): **no matches**.
- Verified those keys live only in Windows user environment variables and in
  the OS DPAPI store, never in the repo.

Result: PASS for "no secret in source or history". NOT RUN: a real Gitleaks
scan, including its entropy rules.

## 2. Containers

Docker Engine 29.9.3 is running. Only `mediahaven-prowlarr` is up; the media
stack containers are **not running** on this machine right now.

Docker Bench for Security was **not run**: its documented command needs
privileged host mounts and root-level access to `/etc`, `/var/lib` and the
daemon socket. That is beyond a read-only check and was not authorized here.
I did the manual equivalent from the procedure's checklist:

| Check | Result |
|---|---|
| `core` user | `10001:10001` (non-root) |
| `core` root filesystem | `read_only: true` |
| `core` capabilities | `cap_drop: ALL` |
| `core` extra privileges | `no-new-privileges:true` |
| `core` port | `127.0.0.1` only, never `0.0.0.0` |
| `probe` user / caps | `10002:10002`, `cap_drop: ALL`, internal network only, no gateway |
| `qbittorrent` user / caps | `10003:10003`, `cap_drop: ALL`, acquire network only |
| Images | digest-pinned via `${..._IMAGE}` variables, no `latest` |
| Networks | `edge` published, `internal` no gateway, `acquire` no published port |

**Finding (low, operator infrastructure):** the Prowlarr container we installed
has no explicit `user`, no `cap_drop`, and a writable root filesystem. It is
bound to `127.0.0.1:9117` only, so it is not exposed to the LAN, but it is less
hardened than our own compose services. Hardening it means recreating the
container and re-entering its API key.

## 3. API security testing

Ran a loopback backend with the dev token and probed 15 cases. All requests
were signed with the repo's HMAC scheme unless the test deliberately removed
or altered credentials.

| Probe | Result |
|---|---|
| Public `/healthz` | 200, `{"status":"ok"}` |
| No credentials | 401 |
| Wrong token | 401 |
| Stale timestamp (-90 s) | 401 |
| Bad signature | 401 |
| Valid signed request | 200 |
| Malformed JSON on parsing endpoint | 400, fixed error |
| Unknown JSON field on parsing endpoint | 400, fixed error |
| `/artwork/../../etc/passwd` | 401, no file served |
| `/artwork/not-a-hash` | 400, fixed error |
| `/api/debug`, `/.env` | 401, no data |
| Host route without credentials | 401 |
| Host route with credentials | 200 |
| Cross-origin request from `https://evil.example` | 200 but **no** `Access-Control-Allow-Origin` header |
| Body larger than 1 MiB | 400 |

No error body contained a key, path, stack trace or technology detail.

**Finding (medium, by design):** there is no per-client rate limiting on the
loopback API. The signature scheme stops unauthenticated use, but a client
holding the install token can issue unlimited requests. This is acceptable for
a single-user local appliance and is recorded, not fixed.

## 4. Code-level security gates (test suite)

Re-ran the Rust suites for the three security-bearing crates. All passed:
`mediahaven-core`, `media-acquisition` and `media-metadata`, including route
parity, the executable-download guard, the probe/Defender gates, secret
redaction, and the path/key validation tests.

## Still open before a security sign-off

- Live download of a chosen source, including executable rejection with real
  qBittorrent.
- Packaged-app cold start through WSL/Docker with the DPAPI secrets injected.
- Second-machine LAN non-reachability measurement.
- A real browser/WebView review.
- Gitleaks and Docker Bench actually installed and run.
- Prowlarr container hardening decision.
- Human checkpoint sign-off CP-0 to CP-7.
