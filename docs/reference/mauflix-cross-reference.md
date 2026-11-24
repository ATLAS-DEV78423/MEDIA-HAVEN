# Miauwatch (formerly Mauflix) — capability inventory and MEDIA-HAVEN cross-reference

- **Date:** 2026-11-23
- **Reference:** `https://github.com/maury91/miauwatch.git` (cloned read-only for analysis; HEAD from that date; local checkout is outside this repository and is not a runtime dependency).
- **Purpose:** answer the four questions asked — what it can do, which buttons work, how it is implemented, and how its recommendations / catalogue / search / magnet-to-play pipeline work — then cross-reference against this product and record adoption decisions.
- **Licence note:** reference is PolyForm Noncommercial 1.0. No reference source, artwork or data was copied into this repository.

## 1. What the reference actually is

A **self-hosted client–server web app** (Bun + Hono + TypeORM/SQLite backend, React 19 + Vite frontend, four Bun microservices over WebSocket RPC), plus a Samsung Tizen TV client and QR device pairing. It is not a desktop app and it is not local-first. Browser UI and server are separate; a working instance normally runs behind Docker Compose with VPN enforcement. See `docs/architecture/system-overview.md` and `docker-compose.yml` in the reference.

Because it is a server product, several of its features (accounts, QR login, admin configuration, notifications, per-device pairing) have no meaningful equivalent in a single-machine, no-account desktop app. Those are recorded as "not applicable", not as missing.

## 2. Answer: how it gets "user recommendations"

**It does not run a recommendation algorithm.** There is no scoring model, no similarity engine, no "because you watched", and ratings are stored but never feed ranking. The frontend has no recommendation endpoint.

What looks like recommendations is four mechanisms:
1. **Curated remote lists** fetched from Trakt (popular, trending, community lists, personal watchlist/favourites/history) and AniList (seasonal, trending, all-time popular) by its `list-service` (`services/list-service/src/service.ts:36-50`, `anilist.ts:41-43`).
2. **List slugs are paged** by `GET /api/lists` and `GET /api/list/:slug` (`features/media/api/lists.api.ts:22-58,86-111`); an empty list disappears from Home (`features/home-feed/listPages.ts:92-104`).
3. **"Continue watching" is a synthetic row** assembled in the frontend from `/api/progress`, not a backend list (`pages/home/home-feed.ts:1-17`, `useHomeFeed.ts:77-130`, polled every 60 s at `:67-75`).
4. **Session-scoped personalisation** via `X-Session-Id` on list/watchlist requests (`lists.api.ts:31-54`).

MEDIA-HAVEN already matches 3 of these (continue watching, recently added, genre-overlap "More Like This") without a server. **Adopted:** a local "Because you watched" shelf derived from library progress + genre overlap (Task 4). **Rejected:** remote curated lists — they require an online service and a content catalogue, which violates local-first and the no-content-source rule.

## 3. Answer: the catalogue and scrolling

Reference: infinite rows ("slugs") on Home, 20 per page (`pages/home/hooks/useHomeFeed.ts:17-50`), next page fetched when focus approaches the end (`:132-149`), row cards batched `POST /api/media/batch` (≤50, 225 ms rate limit, `features/media/api/media.api.ts:20-39`), neighbour-page prefetch (`features/home-feed/row-pages.ts:1-14`). TV rows are virtualised WebGL with DOM fallback. There is **no genre filter or sort UI on Home**; genre chips exist only in search.

MEDIA-HAVEN: `BrowseView` renders a grid with local genre chips (top 7) and Recent/A–Z/Year sort, plus horizontal `Shelf` carousels on Home. It caps a single fetch at 200 titles with no offset paging in the UI (`services/api.ts:49`, `crates/persistence/src/sqlite/library_repo.rs:18`).

**Decision:** no paging work in this change. The cap is a documented limit, and adding offset paging would touch the frozen query DTO and AppState loading path for no benefit at the current library sizes. Recorded as a known gap, not silently ignored.

## 4. Answer: how search works

Reference: realtime WebSocket search, **250 ms debounce** (`features/search/search.middleware.ts:9,27-40`), popular genre chips, recent terms with remove, mosaic results with infinite loading near the end (`pages/search/SearchPage.tsx:111-120,268-341`), `/` opens search globally, results open details with focus restored. Max input length 100.

MEDIA-HAVEN: 180 ms debounce, `/` hotkey, recents in localStorage, local SQL `title LIKE` + kind/genre filters (`sqlite/library_repo.rs:348-393`). Query genres were already supported by the API but not exposed in the UI. **Adopted:** genre chips wired to the existing `api.search(text, genres)` (Task 3). **Not adopted:** WebSocket transport and online mosaic loading (server-dependent, no local benefit).

## 5. Answer: how it pulls a magnet, downloads, and plays (analysis only — NOT ported)

End-to-end chain, for completeness of the question:

1. **Watch Now** → `POST /api/playback/sessions` (`backend/src/routes/playback.routes.ts:40-81`) → on-demand source search (`backend/src/services/source/source.service.ts:418-588`).
2. Sources service queries three adapter kinds — Cardigann (Jackett/Prowlarr YAML), Torznab, and Stremio addons (Torrentio/Comet/MediaFusion) — `services/sources/src/providers/registry.ts:104-139`, `stremio.ts:240-341`. Default providers inserted at `services/sources/src/db/database.ts:40-51`.
3. Results are normalised into `ContentSource` candidates; `buildMagnet()` injects public trackers (`services/sources/src/normalize/magnet.ts:32-48`). Torrent URLs are sealed as encrypted `fileRef` values.
4. Magnet → `.torrent` resolution tries the provider URL, then itorrents.org, then torrage TTL decode (`services/sources/src/torrent-file/torrent-file.service.ts:43-86`).
5. **WebTorrent in-process** (not qBittorrent/Transmission) attaches the torrent with encrypted `ChunkStore`, selective file/piece priority and read-ahead (`services/media-load/src/engine/download/torrent-attacher.ts:55-82,258-360`, `claim-selection.ts:73-129`).
6. Playback is **progressive byte-range streaming from verified pieces**, HTTP 206, ~N seconds read-ahead (`file-streamer.ts:177-250,364-493`), exposed as `GET /api/stream/:token` (`stream.routes.ts:190-226`). FFmpeg reads through a private loopback random-path proxy. HLS exists only for TV remux.
7. VPN is enforced by default (`DISABLE_VPN_CHECK=false`, `packages/vpn-guard/`), FlareSolverr is optional, and logs redact hashes/magnets.

**Decision: none of steps 1–7 was adopted.** `AGENTS.md` non-negotiables state "No content-source bundling. No trackers, indexers, catalogues or instructions for unlawful acquisition. Ever." MEDIA-HAVEN instead accepts an **operator-supplied** magnet/bare `.torrent` (`crates/acquisition/src/orchestrator.rs:22-26,47-64`), drives a user-owned qBittorrent WebUI (`qbittorrent.rs:206-252`), and gates playback on two fail-closed scans (`inspection.rs`, `defender.rs`). That boundary is preserved unchanged.

## 6. Full feature/button checklist with implementation notes

Legend: ✅ present locally · 🆕 adopted in this change · ➖ present in reference but not applicable to a local-first no-account desktop app · ⛔ deliberately rejected · ⚠️ partial / known gap.

### Browsing and discovery
| # | Feature / button | Reference implementation | MEDIA-HAVEN |
|---|---|---|---|
| 1 | Home rows / infinite paging | `useHomeFeed.ts`, batch media API | ✅ Shelves + hero; ⚠️ 200-title cap |
| 2 | Virtualised/TV WebGL rows | `HomePage.tsx:142-145,526-577` | ➖ desktop only |
| 3 | Hero crossfade on focus | `hero/MediaHeroDom.tsx:95-266` | ✅ static hero |
| 4 | Curated remote lists / Trakt / AniList | `services/list-service` | ⛔ online catalogue dependency |
| 5 | "Because you watched" local recommendation | absent in reference | 🆕 Task 4 |
| 6 | Continue watching row | frontend from `/api/progress` | ✅ `library_home` shelf |
| 7 | Genre filter + sort on browse | absent on Home | ✅ `BrowseView` chips + Segmented |
| 8 | Infinite search results | `SearchPage.tsx:111-120` | ⚠️ single capped query |
| 9 | Search genre chips + recents + `/` | `search.middleware.ts` | ✅ recents + `/`; 🆕 chips Task 3 |

### Detail page buttons
| # | Button | Reference | MEDIA-HAVEN |
|---|---|---|---|
| 10 | Watch Now / Resume with progress | `MediaDetails.tsx:527-553,661-684` | ✅ Play + resume |
| 11 | Notify me when unavailable | `:645-647` | ➖ no remote availability |
| 12 | Watchlist bookmark toggle | `:496-508` | ✅ Add to My List |
| 13 | Like / dislike / love rating | `:509-524,211-214` | ⚠️ not present; local ratings deferred |
| 14 | Season selector + episode rows | `:695-721`, `EpisodesSection.tsx` | ✅ episodes from local DB |
| 15 | Trailer / cast / reviews / share | absent in reference | ➖ absent in both |
| 16 | More Like This | absent on detail in reference | ✅ genre overlap |

### Player buttons and behaviours
| # | Control | Reference | MEDIA-HAVEN |
|---|---|---|---|
| 17 | Native `<video>` / mpv engine | `PlayerView.tsx:378-404` | ✅ mpv over IPC, embedded child HWND |
| 18 | Play/pause, ±10 s seek, seek bar, volume | `PlayerControls.tsx:54-67,508-577` | ✅ Space/←/→, ±15 s, range |
| 19 | Subtitle menu + timing shift | `subtitles.service.ts`, `PlayerControls.tsx` | ⚠️ mpv sidecar support; no UI |
| 20 | Audio/source menu + language search | `AudioMenu.tsx:180-259` | ⚠️ no source picker UI |
| 21 | Quality selector | planned only (`ToDo.md`) | ➖ no remote qualities |
| 22 | Fullscreen / mute | `PlayerControls.tsx` | ✅ |
| 23 | Skip intro / skip credits | `PlayerView.tsx:419-447` | ⚠️ skip-intro pill only |
| 24 | Next-episode prompt + prefetch at 70% | `usePlaybackPrompts.ts:304-315` | ⚠️ not present |
| 25 | Hover frame previews | unreleased (`DETAILED_CHANGELOG.md`) | ➖ |
| 26 | In-app playback, no second window | browser video element | ✅ `--wid` child surface; 🆕 teardown fix Task 2 |
| 27 | PiP | absent | ➖ |

### Account, social, system
| # | Feature | Reference | MEDIA-HAVEN |
|---|---|---|---|
| 28 | Email login / first-admin / JWT cookies | `auth.routes.ts` | ➖ no account by design |
| 29 | QR device pairing / TV remote | `tv-pairing/` | ➖ |
| 30 | Profiles + plushie avatars | `WhosWatchingPage.tsx`, `ProfileEditorPage.tsx` | ✅ profiles, colour+initial |
| 31 | Per-profile preferences (language/subs) | `PreferencesPage.tsx:126-165` | ⚠️ global settings only |
| 32 | Notifications + "Notify me" | `NotificationsMenu.tsx:224-305` | ➖ no server |
| 33 | Trakt sync / watch history sync | Trakt service | ⚠️ local progress only |
| 34 | Ratings stored | `media-rating.entity.ts` | ⚠️ not present |
| 35 | Admin source config UI (Cardigann/Torznab/Stremio) | `SourcesPage.tsx:60-276` | ⛔ indexer bundling forbidden |
| 36 | Download/progress UI | backend torrent preparation | ✅ operator-supplied download flow with scan gates |
| 37 | VPN / Gluetun / FlareSolverr | `docker-compose*.yml`, `vpn-guard` | ⛔ host/network mutation forbidden |
| 38 | Watch parties / comments / sharing | absent in reference | ➖ absent in both |
| 39 | Subtitle translation / lockscreen / TV | planned (`ToDo.md`) | ➖ |

## 7. Adoption decisions and rationale

**Adopted (local-first, zero new dependency, no content-source code):**
- Task 2 — stop playback and hide the embedded surface on PlayerPage unmount; observe layout with `ResizeObserver`. This is required for "one window" to be true in practice.
- Task 3 — search genre chips through the existing local search contract.
- Task 4 — local "Because you watched" shelf, deterministic and offline.

**Rejected with reasons:**
- Magnet/indexer/WebTorrent/FlareSolverr/VPN pipeline (steps 1–7 in §5): violates the no-content-source and no-host-mutation non-negotiables.
- Remote curated lists / Trakt / AniList: online dependency and a content catalogue; local library data is the only source of truth here.
- Accounts/QR/notifications/admin source UI: server concepts with no single-user desktop equivalent.
- Ratings, next-episode prompt, subtitle UI: useful but would require new persisted DTOs/routes (frozen at 24 commands) or new engine properties; deferred with a clear gap record rather than a partial implementation.

**What could not be verified:** the reference was read as source, not run (no Bun/Docker install or network service changes were permitted). Button behaviour statements are based on handler wiring in source and E2E specs, not on a live reference instance. The local MEDIA-HAVEN live playback gate beyond the throwaway Win32 probe is NOT RUN in the desktop app itself.
