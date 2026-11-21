# ADR-0024: Reject executable-bearing torrents before download

- **Status:** Accepted (2026-11-21, human instruction)
- **Related:** ADR-0018, ADR-0019, Development.md §8.4/§8.7

## Context
The existing probe/Defender gates inspect a download only after qBittorrent has
written its bytes. A torrent may declare an `.exe` alongside media, so the app
could place an executable on disk before any gate refused to play it.

## Decision
Acquire every torrent paused. Read its declared file names through qB's files
endpoint, and refuse to resume if any name has a known executable extension
(exe, com, scr, pif, bat, cmd, msi, msp, jar, js, jse, vbs, vbe, wsf, wsh, hta,
ps1, ps1xml, lnk, reg, dll, ocx, sys, drv, cpl, msc, app, appx, msix) in its
final or any dotted segment. Case-insensitive. Empty/failed metadata fails
closed. A rejected task is removed with `deleteFiles=false` and no file bytes
are downloaded. This is a name-based pre-download control: it cannot inspect
bytes the client has not fetched, so the existing post-download media-parse and
Defender gates remain mandatory.

## Consequences
Positive: executables are never written to the download root by this app.
Negative: a file deliberately named with an executable-looking segment is
rejected even if it is media (the operator can rename the source); a magnet
whose metadata is not ready within ~1 s is rejected and can be retried. Both
fail closed by design.
