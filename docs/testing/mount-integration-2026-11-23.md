# Host media-folder import — live verification (2026-11-23)

## What was missing

The installed app could not import a host media folder. The backend runs inside
`core`, so a Windows host path is not visible to it. The read-only mount renderer
(`mounts.rs`) existed but was never wired into startup or root add/remove.
`load_host_path_map()` derived host paths from backend roots, which cannot serve
both the backend (needs container paths to scan) and mpv (needs host paths).

## What changed (commits `9d172c5`, `1a11f49`, `1a11f49` review fixes)

- Host persists ordered `{containerPath, hostPath}` records in
  `%APPDATA%\MEDIA_HAVEN\mounts.json` (atomic write).
- `render_mounts` supports stable explicit container paths; `None` keeps the
  old positional behaviour and all prior tests.
- Supervisor adds `-f <override.yaml>` in both WSL and Windows engine modes.
- `runtime_start` stages the override before `compose up`.
- `library_root_add` validates the host path, appends a record, remounts,
  registers the **container** path in the backend, and rolls back on failure.
- `library_root_remove` removes only the matching record; survivors keep their
  container paths. Rollback on failure.
- `load_host_path_map` resolves through host records, so mpv gets host paths.
- Stale/offline host folders are skipped at staging/startup instead of blocking
  app startup; the persisted record is kept so the drive can come back.

## Live evidence (installed NSIS app, Windows engine mode)

Installer SHA-256: `41a9b747b10977618bc91a95d802cb7e4e605919ac9f1a37f4f9152e3b325c09`

| Check | Result |
|---|---|
| Cold start with a persisted mount record | PASS — `override.yaml` staged |
| Compose merge (`-f base -f override`) | PASS — `docker compose config` keeps all base volumes and adds `/media/root-0` |
| Containers | PASS — `core` healthy, `probe` and `qbittorrent` up |
| Container sees host folder | PASS — both test files listed in `/media/root-0` |
| `docker inspect` mount mode | PASS — `/media/root-0` and `/downloads` are `ro` |
| Backend registers container path | PASS — root `path` = `/media/root-0` |
| `library_scan` | PASS — `scanned: 2`, no errors |
| `library_home` | PASS — both files appear as `available` |
| `playback_start` | PASS — session returned for `Test Movie 2024` |
| Stale record tolerance | PASS — a nonexistent record was filtered; startup and mount unaffected |
| mpv decodes the resolved host file | PASS — `--frames=3` exit 0 |

Test library: `C:\Users\Stanley\Videos\MediaHavenTest` (two generated, lawful
test clips; no personal data).

## Not verified

- The UI click -> Tauri command dispatch -> mpv window path was not driven
  headlessly. Helpers and the backend/half are verified; the human tester
  confirms the visible player.
- WSL engine mode was not exercised (no WSL distro with Docker on this host).
  The Windows bind-source form was verified with `docker compose config`.
- `wait_ready()`'s 10 s ceiling under a slow container recreate was not stressed.
- No axe-core run, visual baseline, or second-machine LAN check.
