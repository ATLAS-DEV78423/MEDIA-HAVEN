# Backend flow — plain-language system design

Date: 2026-11-22.

This describes the path from a person clicking something in the app to the
containers and back again. Words in brackets are the exact names in the code.

## The simple picture

```
YOU (Windows desktop)
   |
   | click a button / type in a box
   v
REACT USER INTERFACE  (apps/desktop/src)
   |
   | calls a NAMED COMMAND, for example library_home or source_search
   v
TAURI HOST  (apps/desktop/src-tauri, Rust, runs on Windows)
   |  - holds the install key in Windows protected storage
   |  - signs each request (HMAC) so nobody can fake it
   |  - starts the containers when the app opens
   |  - opens the video player
   v
LOOPBACK API at 127.0.0.1:8080   (only this machine can reach it)
   |
   v
CORE CONTAINER  (crates/core, inside Docker in WSL2)
   |  - SQLite database, library rules, settings, search, safety checks
   |  - talks to the other containers on private networks
   +----> PROBE CONTAINER  (no network, no shell, ffprobe only)
   +----> QBITTORRENT CONTAINER  (private acquire network, no open port)
   +----> TMDB / IMDb / Prowlarr  (only when a key or address is supplied)
```

## Where everything lives

| Thing | Where | What it holds |
|---|---|---|
| Screens and buttons | Windows desktop, React | What you see |
| Named command bridge | Windows desktop, Tauri host | The install token, DPAPI secrets, the video player |
| Backend | `core` container in Docker | SQLite, rules, caches, all business logic |
| Media scanner | `core` container | Reads your folders through a read-only bind |
| Media inspector | `probe` container | ffprobe only; no network, no shell |
| Downloader | `qbittorrent` container | Torrent tasks and downloaded files |
| Source finder | Your own Prowlarr, on this machine | Search results from indexers you chose |
| Database | Named Docker volume `core-data` | Library records, progress, sources, state |
| Posters | `core` data directory | Small cached JPEGs, served by hash |
| Settings secrets | Windows protected storage (DPAPI) | TMDB key, Prowlarr address, Prowlarr key |

The Windows host and the containers never share a dashboard. There is one user
surface: the desktop app.

## Flow 1 — Starting the app

1. You open the desktop app.
2. React calls `startRuntime()` once when the app mounts.
3. The Tauri host calls `runtime_start`.
4. The host reads the install token from DPAPI, writes the compose file to a
   temporary location, converts the path to WSL form, and runs
   `docker compose up -d` inside the dedicated WSL2 distro.
5. The host passes the TMDB key, Prowlarr address and Prowlarr key into the
   `core` container as environment variables.
6. React retries `library_home` / `settings_get` for about 29 seconds while
   the backend becomes ready. If it cannot start, you see a fixed "setup
   required" message.

## Flow 2 — Browsing your own movies and shows

1. You add a folder in Setup.
2. React calls `library_root_add` then `library_scan`.
3. The host signs the request and sends it to `core`.
4. `core` scans the folder through a read-only mount. It never moves or deletes
   your files.
5. Results go into SQLite and come back as simple data objects.
6. React renders cards. Each card asks for a poster from
   `http://127.0.0.1:8080/artwork/<hash>`.
7. `core` checks the hash is exactly 64 lowercase hex characters before opening
   any file. It serves cached bytes with `image/jpeg` and `nosniff`. If there is
   no poster, the card keeps its bundled placeholder.

## Flow 3 — Posters and the catalogue

1. You save a TMDB key in Settings. It goes into DPAPI, never into the repo.
2. The next app start injects it into `core`.
3. For a movie page, `core` asks TMDB for the title by name and year. If a key
   is present, TMDB is used first; otherwise the keyless IMDb route is used.
4. The poster path from the answer is cleaned and checked. Only an allowed CDN
   path is accepted.
5. The image is downloaded (at most 2 MB), stored under a hash, and given a
   relative URL like `/artwork/<hash>`.
6. The Discover tab uses one command, `discover_catalog`, with `movie`, `tv` or
   `anime`. Anime is TMDB's Animation genre plus Japanese language filter —
   an approximation, not a dedicated anime service.
7. At most 12 posters per page are fetched, so a slow site cannot freeze the
   screen.

## Flow 4 — Finding and choosing a torrent

1. On a movie page you click **Find sources**.
2. React calls the Tauri command `source_search`.
3. The host forwards it to `core` at `/host/source_search`.
4. `core` reads your Prowlarr address and key from its environment.
5. `core` asks Prowlarr which indexers are enabled, then asks each one through
   its Torznab feed. This is per indexer; there is no single "all" feed.
6. Each result is filtered: the title must match as a word-aligned phrase, the
   year/season/episode must match when supplied, and the source must be a
   magnet or a bare `.torrent` name. Prowlarr often gives only an HTTP link, so
   `core` builds a magnet from the 40-character info hash when available.
7. Results are sorted by seeders, capped at 10, and returned with no address,
   key or file path.
8. React shows them as a short list. Nothing downloads automatically.
9. When you choose one, React calls the download command then `source_set` to
   attach the chosen source to that library item.

## Flow 5 — Downloading safely

1. `core` adds the torrent to qBittorrent with `paused=true`, so no file bytes
   are written yet.
2. It reads the declared file names. Any name with a Windows executable
   extension (`.exe`, `.bat`, `.cmd`, `.scr`, and others) causes the task to be
   removed and rejected. A bad or empty file list is also rejected.
3. Only a clean task is resumed. qBittorrent then writes files into the shared
   download folder.
4. When the task completes, `core` triggers the probe container:
   `docker run --network=none --entrypoint ffprobe ... -v error -show_format
   /downloads/...`. The probe has no network and no shell. It either parses the
   file as media or it does not. Failure means not playable.
5. The Windows host also runs a Defender command-line scan. Both gates use
   argument lists, never a shell string. Both must pass.
6. Only then can playback start. The executable guard, probe and Defender
   checks are never skipped for a renderer-driven action.

## Flow 6 — Playing a file

1. React calls `playback_start`.
2. `core` checks the live session and the safety gates, then returns a
   container path such as `/downloads/show/e01.mkv`.
3. The Tauri host maps that container path to a Windows path using the Compose
   mount map. No path is ever put into a renderer DTO.
4. The host opens the local video player with that Windows path.
5. The backend marks the item as the live session; cleanup waits until playback
   ends and the item is watched.
6. Cleanup removes files through qBittorrent's own delete command, never by
   deleting files directly inside `core`.

## Commands and routes

- **Renderer commands (25):** `library_home`, `library_movies`, `library_shows`,
  `library_detail`, `library_episodes`, `library_search`, `library_roots`,
  `library_root_add/remove`, `library_scan`, `library_watchlist`,
  `watchlist_add/remove/reorder`, `queue_list/add/remove/reorder`,
  `downloads_list`, `settings_get/set`, `playback_start/stop/checkpoint`.
  These are signed and validated; unknown fields are rejected.
- **Host-only routes:** `/host/acquisition_add`, `acquisition_status`,
  `acquisition_remove`, `acquisition_buffer_status`,
  `acquisition_cleanup_confirm/pass`, `acquisition_inspect`,
  `acquisition_source_set`, `imdb_lookup`, `indexers_seed`, `source_search`.
  These are not part of the 25 renderer commands.
- **Public routes:** `/healthz` and `GET /artwork/:key`.

## Network boundaries

- The API is published on `127.0.0.1` only. It is never exposed to the LAN.
- `edge` network carries the published API port.
- `internal` network has no gateway; the probe lives here only.
- `acquire` network has no published port; `core` and qBittorrent meet here.
- Prowlarr is bound to `127.0.0.1:9117` on this machine.
- TMDB and IMDb requests only leave the machine when a key or supplied dataset
  exists; posters are cached locally afterwards.

## What is not live yet

- A complete cold start of the installed app through WSL/Docker.
- A real click through download, probe, Defender scan and playback.
- A real browser/WebView visual check.
- A second-machine network reachability test.
- Human sign-off of checkpoints CP-0 to CP-7.

This design is implemented and unit-tested; those five items are the remaining
proof gates.
