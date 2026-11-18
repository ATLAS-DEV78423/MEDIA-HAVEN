# apps/desktop

The user-facing surface: a React 19 + TypeScript + Vite UI for MEDIA HAVEN,
wired to the Rust service in `crates/core` (package `mediahaven-core`).

## Run it locally

Two processes, both loopback:

```sh
# 1. the backend, with a seeded demo library and a local data directory
MEDIAHAVEN_DEV=1 MEDIAHAVEN_SEED=1 \
MEDIAHAVEN_DATA_DIR="$PWD/.mediahaven-dev" \
cargo run -p mediahaven-core

# 2. the interface
cd apps/desktop
npm install
npm run dev            # http://localhost:5173/
```

### In the Tauri host

`apps/desktop/src-tauri` is a Rust workspace member that opens the real window
and loads the same UI. `npm run tauri:dev` starts Vite and the host together:

```sh
cd apps/desktop
npm install            # once, installs @tauri-apps/cli
npm run tauri:dev      # window titled MEDIA_HAVEN
```

The host currently exposes one command, `get_app_info`. It does **not** yet own
the container supervisor, the install token or the named command surface, so the
renderer inside the window still calls the API over HTTP exactly as the browser
does. Do not treat the window as the production transport until A5/A7 land.

`MEDIAHAVEN_DEV=1` uses the documented development token
(`dev-only-not-for-production`). Outside development the service refuses to
start without a real `MEDIAHAVEN_API_TOKEN` of at least 32 characters, and the
desktop host must pass that token to the UI as `VITE_MEDIAHAVEN_TOKEN`.

| Variable | Default | Purpose |
|---|---|---|
| `MEDIAHAVEN_API_TOKEN` | dev token when `MEDIAHAVEN_DEV=1` | Install secret |
| `MEDIAHAVEN_BIND` | `127.0.0.1:8080` | Listener address |
| `MEDIAHAVEN_DATA_DIR` | platform data dir | SQLite location |
| `MEDIAHAVEN_SEED` | off | Seed the demo library once |
| `VITE_MEDIAHAVEN_API` | `http://127.0.0.1:8080` | Backend origin |
| `VITE_MEDIAHAVEN_TOKEN` | dev token in `npm run dev` | Install secret |

## How the data flows

```
React pages ──► state/AppState.tsx (snapshot + local prefs)
                  │
                  ├──► services/api.ts  (signed named operations)
                  │        └── HMAC-SHA256 over timestamp, method, path, body
                  └──► services/adapt.ts (backend DTOs ──► view model)
```

- `services/api.ts` is the only network boundary. It signs every request per
  ADR-0005 and never invents a fallback response.
- `state/AppState.tsx` fetches `home`, `movies`, `shows`, `watchlist`, `roots`,
  `settings` and `downloads` after `/readyz`, and exposes mutations that write
  through the API.
- `services/adapt.ts` maps backend records to the view model. Missing fields
  (rating, match score, tagline) stay absent and the UI hides them.
- `data/catalog.ts` holds only the view-model types, formatting helpers and the
  deterministic artwork fallback. There is no fixture catalogue.

## Layout

```
src/
├── components/   nav, hero, cards, shelf, browse, ui primitives
├── pages/        Home, Movies, Shows, Anime, Details, Player, Library,
│                 Watchlist, Downloads, Search, Settings, Setup, Profiles
├── hooks/        useSpatialFocus (geometry-based arrow-key focus)
├── services/     api (signed client), types (DTOs), adapt (view model)
├── state/        AppState provider (server snapshot + local preferences)
├── styles/       tokens.css (all literals), base.css, app.module.css
└── data/         view-model types and artwork mapping
```

## What is real and what is not

Real: library scan and indexing, movies/series/episodes, search, details,
watchlist, profiles (stored in backend settings), playback sessions and
resume checkpoints, settings, library roots, downloads state, diagnostics.

Not real yet:

- **Video playback.** The player simulates a clock at 4× and labels itself
  "Playback preview". Sessions and checkpoints are real, the picture is artwork.
- **Downloads.** The backend reports the service as disabled and exposes no
  mutation; the screen is read-only.
- **Music.** The schema only stores movie/series/episode, so there is no Music
  tab or album UI.
- **Artwork metadata.** No provider is configured; posters come from the
  bundled local set.
- **Per-profile progress.** Profiles are stored, but playback positions and the
  watchlist are global to the install.

## Rules for this directory

- **Presentation only.** No SQL, filesystem policy or player engine here. The
  backend decides; this layer displays and collects intent.
- **Tokens only.** No colour or spacing literal outside `styles/tokens.css`
  (`npm run check:tokens`).
- **Typed boundary.** Every operation goes through `services/api.ts`.
- **Keyboard first.** Arrows + Enter reach everything; every icon button has an
  accessible name.
- **Honest states.** Empty, loading, offline and failure states are explicit;
  the UI never falls back to fabricated titles.

## Checks

```sh
npm run typecheck
npm run lint
npm run test
npm run check:tokens
npm run build
```

Rust: `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt --check`.

## Related documents

- `docs/design/design-system.md` — tokens, type, motion, focus
- `docs/design/navigation-spec.md` — focus model and traversal
- `Development.md` §7 — the UI contract
