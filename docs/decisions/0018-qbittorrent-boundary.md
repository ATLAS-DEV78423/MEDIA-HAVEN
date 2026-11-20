# ADR-0018: qBittorrent sandbox and trust boundary

- **Status:** Accepted (2026-11-20)
- **Date:** 2026-11-20
- **Decider:** human partner
- **Scribe:** contributing agent
- **Authority:** the human partner's "i accept the ADRs" (2026-11-20), naming ADR-0018.
- **Related:** ADR-0017, ADR-0005 §3, ADR-0014 (DPAPI token), ADR-0016,
  `docs/security/compose-security-checklist.md`, conversion plan Task 2/3

## Context

qBittorrent must run in Docker to isolate downloader activity, but unlike the
networkless `probe` it needs outbound network access and a WebUI API. Its WebUI
is a privileged management interface that must never reach the browser or LAN.

## Decision

1. qBittorrent runs as its own container on a dedicated `acquire` bridge network
   **with egress** and **no published port to the LAN**.
2. The WebUI is reachable only from `core` on the `acquire` network over loopback
   (container-network address), never from the renderer. The renderer has no
   WebUI capability and no downloader credentials — only named commands.
3. WebUI credentials are host-generated, stored via the DPAPI secret path
   (ADR-0014), and injected into the container at start; never in source, images,
   logs, or renderer DTOs.
4. Acquired media lives in a dedicated download directory bind-mounted read-write
   into qBittorrent and read-only into `core`; the host resolves acquired files
   through the A9 host-path map for mpv. (Storage option B; gate G-1.)
5. Downloaded media and supplied links are treated as untrusted input. No
   trackers, indexers, catalogues or "find a torrent" features are bundled —
   the user supplies magnet links / `.torrent` files only. Lawful use only, per
   the target specification.

## Consequences

- A third Docker network and a new container join `core`/`probe`; the compose
  security checklist must be extended and re-verified.
- The qBittorrent WebUI client is loopback-only and hand-written (std TcpStream)
  unless a future ADR approves an HTTP dependency.
- No LAN exposure is permitted; a second-machine reachability test remains a
  human evidence gate.

## Revisit trigger

A requirement to expose the WebUI remotely (reopens ADR-0005 and this ADR), or
a legal/scope change to acquisition sources.
