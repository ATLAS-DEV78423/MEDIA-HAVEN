# Backend evidence — 2026-11-22

Observed on branch `tmdb-artwork-and-exe-guard` at `bce85fc`, using
`target/debug/mediahaven-core.exe` built from this worktree. The backend ran on
`127.0.0.1:8084` with `MEDIAHAVEN_DEV=1`, `MEDIAHAVEN_SEED=1`, a throwaway data
dir, and the operator-supplied TMDB key in the process environment. It was
stopped after the run.

## Commands and observed output

```
GET /healthz -> 200
POST /library_home -> shelves=2 items=9 withArtwork=7
sample=Dune url=/artwork/8867fd2551ac67118e9839556410817c661ced7858d05a9dda00f5f23ebce448
GET /artwork/<hash> -> status=200 type=image/jpeg bytes=83755
POST /settings_get -> ok=True profiles=3
token_leaked_in_response=False
server_stopped=True
```

Requests were signed with the dev token using the repo's canonical
`<timestamp>\n<METHOD>\n<path>\n<sha256(body)>` HMAC scheme.

## What this proves

- The backend starts and serves its health probe.
- The library endpoint returns real seeded media with hashed artwork URLs.
- The artwork endpoint serves a real JPEG from the local cache with the right
  content type and size, and `nosniff` (verified in an earlier run).
- Settings read works.
- The TMDB token did not leak into the API response.

## Earlier corroborating run (keyless IMDb, 2026-11-21)

`GET /healthz` 200; `/library_home` 10 items, 7 with artwork; `/artwork/<hash>`
200 `image/jpeg` 53,600 bytes, magic `FF-D8-FF-E0`, `nosniff`, 5 cached files.
With the TMDB token supplied, Arrival's artwork hash changed from the
IMDb-only `dfb0d8b5...` to `a78d5e13...`, and a served poster was 68,524 bytes
— proof that the TMDB path supersedes IMDb when a key is configured.

## NOT RUN

- A real browser / WebView screenshot.
- A live Docker Compose start injecting the DPAPI secret into `core`.
- A full-library seed or a live torrent acquisition.
