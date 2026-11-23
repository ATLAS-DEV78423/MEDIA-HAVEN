# Human end-to-end test guide

This machine is installed and running. Use this to test the real app.

## Start

The installed app is at:

```
C:\Users\Stanley\AppData\Local\MEDIA_HAVEN\mediahaven-desktop.exe
```

It is already open. If you closed it, launch it and wait about 20 seconds.
Expected: `core`, `probe`, `qbittorrent` running in Docker, `core` healthy on
`127.0.0.1:8080`. Prowlarr is running separately on `127.0.0.1:9117`.

## Ready-made test library

`C:\Users\Stanley\Videos\MediaHavenTest` contains two generated clips:

- `Test Movie 2024.mkv`
- `Test Episode S01E01.mkv`

Both are already imported and scanned. Pick a profile, then open **Movies** or
**Library** — you should see both as playable titles.

## What to try

1. **Profile** — pick `Alex`. Home should show real posters and attribution.
2. **Movies / TV Shows / Anime** — open each. Anime Discover uses TMDB's
   Japanese-animation filter (cold cache can take ~50 s).
3. **Details** — open `Test Movie 2024`. `Play` should be enabled now.
4. **Playback** — click `Play`. A native mpv window should open and play the
   file. This is the one piece not yet confirmed by automation.
5. **Player controls** — Space pauses, arrow keys seek, Esc leaves.
6. **Add your own folder** — Setup: point at a folder of your own media, scan,
   then open a title and play. This exercises the new mount wiring live.
7. **Find sources** — on a Details page, click `Find sources`. Prowlarr-backed
   magnet results should appear (no automatic download).
8. **Settings / Downloads / Queue** — walk each tab; report anything broken.

## Environment

All required settings are Windows user environment variables, already set:
`MEDIAHAVEN_ENGINE=windows`, image names, tokens, `MEDIAHAVEN_MPV`, TMDB and
Prowlarr keys. A fresh login picks them up automatically.

## Known limits

- Installer is unsigned; SBOM not bundled; nothing published.
- No axe accessibility scan or visual baseline comparison.
- WSL engine mode is untested here (no compatible WSL distro).
- Keyless IMDb posters are a fallback; TMDB posters are primary.

When something fails, note the exact screen, button, and expected vs actual.
