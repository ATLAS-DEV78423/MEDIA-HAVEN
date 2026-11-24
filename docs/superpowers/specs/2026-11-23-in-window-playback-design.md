# Design: In-window playback via mpv `--wid`

- **Status:** **Proposed** — no code written, no ADR accepted, no checkpoint changed.
- **Date:** 2026-11-23
- **Decider:** contributing agent, under explicit human delegation ("take your own
  decision … most suitable to our project while keeping the security of the
  product in mind", 2026-11-23). The human approved the design in chat on
  2026-11-23. This decision is **reversible** in the same way as ADR-0021: if the
  spike disproves the approach, reopen it rather than forcing the code to fit.
- **Related:** ADR-0002 (playback on the Windows host), ADR-0005 (named commands,
  no generic passthrough), ADR-0015 (path-free `player://state`), O-09 (mpv
  licence), `Development.md` §4.4/§7.1, `roadmap.md` P0-03/P3-01.

## Context

`tauri.conf.json` defines exactly one window (`main`). Every screen — including
`PlayerPage` — is a React route or overlay inside it. The only second window is
mpv: `crates/playback/src/mpv.rs::MpvPlayer::spawn` launches an external mpv
child with `--idle=yes` and no parent window handle, so loading a file gives mpv
its own top-level window.

The requested behaviour is "view and play the movie directly in the app, no
second window". The existing boundary is sound and must not be weakened: mpv is
spawned with argument vectors (never a shell), talks over a private IPC endpoint,
is killed on `Drop`, and the renderer receives only `timePos` / `pause` /
`volume` / `mediaId`. Source resolution stays host-only and fail-closed.

## Decision

Keep mpv as an **external, user-supplied binary** and embed its output with
mpv's `--wid=<HWND>` option: mpv draws into a child window owned by our single
Tauri window. Do **not** bundle a DLL, do not add libmpv FFI, and do not add a
runtime dependency.

Rationale:

- No new native code in our installer, so the supply-chain and SBOM surface is
  unchanged. This is the security-conservative choice.
- No redistribution of mpv, so O-09 (LGPL vs GPL) does not bind this repository;
  the licence of the user's mpv build remains the user's responsibility. The
  project keeps an LGPL-compatible posture if bundling is ever proposed.
- The ADR-0005 boundary, the `Player` port, IPC, Drop-kills-child, and the B5
  completion / probe / Defender gates all stay exactly as they are.
- `windows = 0.62` is already a workspace dependency of `mediahaven-desktop`;
  only the `Win32_UI_WindowsAndMessaging` feature is added — no new crate.

Cost, stated honestly: mpv must still be present on the machine (today it is
not installed here); Win32 child-window and resize plumbing is new; the
WebView2 z-order and hit-testing assumptions need a live spike.

## Alternatives considered

| Option | Verdict |
|---|---|
| External mpv + `--wid` child HWND (chosen) | Smallest change, no bundling, no new licence obligation, preserves every gate |
| Bundle libmpv + render-to-texture | True in-DOM canvas, but new native DLL, new FFI dependency, D3D11/WebGL texture sharing, and an O-09 decision — rejected for now |
| Bundle a GPL mpv | Wider codecs, but GPL obligations on the app — rejected |
| Keep the separate mpv window | Zero change, but fails the requested behaviour |

## Scope

1. `crates/playback/src/mpv.rs`: add `spawn_with_parent(mpv_exe, hwnd)` alongside
   `spawn()`. When a parent is supplied, pass `--wid=<hwnd>`; all other arguments
   and the IPC setup are unchanged. `spawn()` stays for headless tests.
2. `apps/desktop/src-tauri/`: a small Win32 helper that creates, positions,
   shows and destroys a child HWND of the main window, with DPI scaling. Add the
   `Win32_UI_WindowsAndMessaging` feature to the existing `windows` dependency.
3. `PlayerState` owns the surface handle. `playback_start` uses it when
   attached; if no surface is attached it returns a fixed
   `player surface unavailable` error and never opens a second window.
4. One new named command `player_attach_surface(x, y, width, height)` with four
   display-only numbers. It is host-only, exactly like `player_control`: register
   it in `generate_handler!` and leave `COMMANDS` and the 24-route parity set
   unchanged (no backend route, no contract DTO). Add its renderer bridge in
   `src/services/player.ts` next to `playerControl`.
5. `PlayerPage.tsx` / `player.module.css`: add a dedicated surface rectangle
   between the top bar and the bottom controls; report it on mount and on
   `ResizeObserver` changes; hide the surface when playback stops or the page
   unmounts.

**Non-goals:** bundling or installing mpv; changing the IPC protocol or
`player://state` payload; changing resume/completion logic; catalog, search or
source discovery (separate sub-projects); remote/headless playback.

## Data flow and contract

```text
React surface rect (CSS px, four numbers)
  -> player_attach_surface -> host scales by GetDpiForWindow -> SetWindowPos(child HWND)
  -> playback_start -> MpvPlayer::spawn_with_parent(exe, hwnd) -> --wid
  -> mpv frames render inside our one window

engine -> player://state -> { timePos, pause, volume, mediaId }
```

No path, token, URL, credential or IPC endpoint appears in either direction.
The renderer never sees an HWND value.

## Error handling and security

- No mpv binary: existing `Spawn` error, unchanged.
- No attached surface: fixed error; **never** a silent fallback to a separate
  window.
- Surface creation/resize failure: fail closed, emit stopped state, log a
  redacted diagnostic. Never swallow the error and continue playing off-window.
- The child is destroyed on `playback_stop`, on route unmount, and on app exit;
  `MpvPlayer::drop` still kills the mpv child.
- Hit-testing: the child must not steal focus from WebView2 controls. Verify
  `WS_EX_TRANSPARENT` / `WS_EX_NOACTIVATE` behaviour in the spike; make the
  surface rectangle exclude the top and bottom control bands regardless.
- All input validation stays on the host: reject non-finite, negative or
  absurdly large rect values at the command boundary.

## Testing and gates

- Pure unit tests: rect validation and DPI conversion, `--wid` argument
  construction (assert the flag and that no shell is used), the no-surface
  error branch, and surface teardown.
- Existing 24-command route-parity test stays at 24 and remains green; the new
  command is host-only, as `player_control` already is.
- Frontend typecheck / lint / Vitest / token check / build unchanged in shape.
- **Spike gate (blocking):** throwaway `spikes/mpv-embed/` probe must observe:
  (1) no second taskbar/window entry, (2) WebView2 controls remain visible and
  clickable, (3) resize/reposition follows the surface rect, (4) kill-on-drop
  leaves no orphan mpv. Requires an mpv binary supplied by the human or explicit
  consent to fetch one — downloading an executable is a host/network change and
  will not be done silently.
- Live playback remains `#[ignore]` until mpv is present. No checkpoint passes
  from this design document alone.

## Risks / revisit triggers

- WebView2 z-order or hit-testing cannot be made reliable → reopen and choose
  libmpv render-to-texture, with a fresh licence/dependency decision.
- `--wid` behaves differently on the exact mpv build the user has → record the
  build and version, mark the matrix UNTESTED where absent.
- Adding a named command changes the frozen contract → the parity test must go
  red then green, not be edited to match silently.
- Reopen if the human declines the Win32 feature addition or wants mpv bundled.

## Self-review

No placeholders. Sections are internally consistent (external mpv throughout).
Scope is one sub-project; catalog/search and Torznab discovery are explicitly
out. "External binary" vs "no bundling" is stated in both Decision and
Non-goals. The only unproven assumption is named and gated by the spike.
