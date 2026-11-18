# ADR-0002 — Playback runs on the Windows host, not in a container

- **Status:** **Accepted (provisional, 2026-11-18)** — mpv via JSON IPC; engine selection sub-decisions below remain open
- **Date:** 2026-11-16 (evidence refreshed 2026-11-15 against the mpv manual and primary docs)
- **Deciders:** architecture; final approval at CP-0
- **Related:** ADR-0001, `docs/architecture/docker-workspace.md` §2.3, roadmap P0-11, P0-12, P3-01

---

## Context

MEDIA_HAVEN must play common video and audio formats with hardware-accelerated decoding,
subtitles, audio passthrough where the hardware allows it, reliable seeking, and smooth
output across resolutions.

Docker is now mandatory (ADR-0001), which raises an obvious question: should the playback
engine also live in the container, for consistency and isolation?

The requirements pull in opposite directions: Docker wants everything inside; the display,
GPU and audio stack are all on the Windows side of the WSL2 boundary.

## Decision

**The playback engine runs natively on Windows, inside the Tauri host process's span of
control. It is an intentional, documented exception to "everything backend-ish lives in
Docker".**

The backend stays in the container. It remains the authority on *what* to play: it owns
the library, the resume position, the track preferences and the policy. The host owns
*putting pixels and sound on the screen*.

### Why the container cannot do this reliably

These are properties of the Windows/WSL2/Docker stack, not preferences:

1. **No display server.** A container has no access to the Windows compositor. It could
   render offscreen, but the output would then have to cross the VM boundary as a video
   stream — adding encode latency, a second decode and a bandwidth ceiling to protect
   against, purely to satisfy an architectural symmetry.
2. **No audio device.** WSL2 has no Windows audio endpoint. PulseAudio-over-TCP requires a
   server on the Windows side that does not exist by default; the whole chain would be a
   custom, fragile shim. Audio passthrough (the bitstream case) is not achievable this way
   at all.
3. **Frame pacing and vsync.** Judder-free output needs presentation tied to the display's
   refresh. That requires being the process that owns the swapchain.
4. **The Linux hardware-decode path does not exist for Windows GPUs.** This is the point
   that is usually overstated in both directions, so here it is stated precisely.

On point 4, the mpv manual lists the actively supported hardware decoders. Three are
relevant:

| mpv hwdec | Platform | Notes |
|---|---|---|
| `d3d11va` | Windows | "requires `--vo=gpu` or `--vo=gpu-next` with `--gpu-context=d3d11`" — this is the native Windows path we intend to use |
| `vaapi` | **Linux only** | The only ordinary Linux hardware-decode path. It needs a Linux GPU driver stack, which WSL2 does not provide for Windows GPUs. |
| `nvdec` | "Any platform CUDA is available" | Works inside a WSL2 container **only** with an NVIDIA GPU *and* the NVIDIA Container Toolkit installed on the host |

**The honest correction:** "WSL2 exposes no GPU to containers at all" is **not** true.
WSL2's `/dev/dxg` does expose the GPU to compute APIs — CUDA and DirectML — and mpv's
`nvdec` backend can therefore decode video inside a container *on an NVIDIA machine with
the NVIDIA Container Toolkit installed*. We are not building v1 on that path, because:

- it requires a host-side GPU driver plus the NVIDIA Container Toolkit, an additional
  dependency and a per-vendor branch that does not exist for Intel or AMD;
- it still leaves points 1–3 completely unsolved — no display, no audio endpoint, no
  vsync — so the video would still have to cross the VM boundary;
- it makes the playback path behave differently on NVIDIA than on the Intel/AMD hardware
  that the "spare laptop" target actually is.

The accurate statement is: **WSL2 offers no hardware *video-decode* path that works
across the GPU vendors we target, and no path at all to the display or audio
hardware.** Microsoft documents GPU access in WSL2 exclusively as CUDA and DirectML —
compute APIs.

5. **A note on `vulkan`.** mpv also supports `--hwdec=vulkan` with `--vo=gpu-next` on
   "any platform with Vulkan Video Decoding", and Vulkan is cross-platform, so in
   principle a Linux container on a Windows host with a Vulkan-capable driver could
   decode. This is worth one experiment in P0-12 because it is the only credible route
   that would move decoding into the container across vendors. It is **not** expected to
   change the decision, because points 1–3 are untouched by it, and because the WSL2
   Vulkan/driver story on consumer Windows hardware is unproven. If the experiment
   succeeds it would justify re-opening this ADR, not a quiet implementation choice.

Any one of points 1–4 is arguable. Together they are decisive.

### Why host playback is not a security regression

The concern with running a media engine on the host is that media parsers are the most
attack-prone code in the application. The mitigation is structural, not hopeful:

| Concern | Control |
|---|---|
| Malicious file exploits the decoder | Accepted residual risk. This is the same risk every media player carries. Bounded by: OS-level mitigations (ASLR/CFG/DEP), a sandboxed renderer, and no privileged host actions taken by the player process. |
| Player process reaches the container network | The player never talks to the backend API. It receives a resolved file path and a start position from the UI and nothing else. |
| Player process writes to the library | It opens the file read-only. No file-writing code path exists in the playback module. |
| Player becomes a privilege-escalation vector | It runs at the same integrity level as the app, which runs as the standard user. No elevation, no service, no driver. |
| Codec parsing is the risky part, and it is now on the host | **This is the real trade-off** and is recorded as residual risk R-19. The counter-argument: the alternative is a playback experience that fails the product's core promise, or a re-encode pipeline that doubles CPU cost. A player that cannot play has no security value. |

### The exception, stated precisely

> Everything that decides, stores, authorises or parses *untrusted* media bytes runs in
> Docker. The component that turns already-authorised bytes into pixels runs on Windows,
> because the OS interfaces it needs exist only there.

`probe` (in Docker) is what touches files during import, before they are ever offered for
playback. Playback is the second, deliberate contact — with the user having chosen that
specific title — and it happens where the hardware is.

## Engine selection

Still open between the candidates, and now scoped to **native Windows builds only**:

| Candidate | Windows hardware decode | Subtitles | Audio passthrough | Notes |
|---|---|---|---|---|
| **libmpv** (`--vo=gpu-next`, `--gpu-context=d3d11`, `--hwdec=d3d11va`) | d3d11va / DXVA2 | Full ASS/SSA via libass | `--audio-spdif` | Embeddable, scriptable, mature; LGPL/GPL build choice matters |
| **mpv as a child process + `--wid` embedding** | Same as above | Same | Same | Simpler integration, weaker control over the surface |
| **WebView2 HTML5 `<video>`** | Browser-dependent; no MKV, no HEVC-in-MKV, no ASS | Poor | None | Rejected: fails the format matrix outright |
| **VLC/libvlc** | DXVA2/D3D11 | Good | Yes | Viable fallback; heavier embedding story |
| **FFmpeg + custom renderer** | Full control | Very high effort | Yes | Rejected: months of work for parity |

**Leading candidate: libmpv, native Windows build, `--vo=gpu-next --gpu-context=d3d11
--hwdec=d3d11va`.** Note the `gpu-context` requirement: `d3d11va` is only usable with a
D3D11 GPU context, so `--hwdec=d3d11va` alone is not a working configuration.

Open sub-decisions to settle with evidence:

- **LGPL vs GPL build.** mpv is "GPLv2 'or later' by default, LGPLv2.1 'or later' with
  `-Dgpl=false`". A GPL build forces the whole application to GPL. If MEDIA_HAVEN is to
  be permissively licensed, the LGPL build is required, which constrains which optional
  libmpv features can be used. **This is a licensing decision for the product owner
  (O-09) and it is a hard gate, not a detail.** The LGPL build must be produced from
  source with `--enable-lgpl`; a prebuilt binary's licence must be checked, not assumed.
- Whether mpv's own audio output (`wasapi`) is sufficient for passthrough, or whether the
  host module needs direct WASAPI exclusive-mode handling.
- Whether the player runs in the Tauri process or as a supervised child process (the
  latter is preferred: a decoder crash then kills a child, not the UI).

### Expected codec reality on the reference machine

The reference machine has an **Intel Iris Xe** (Xe-LP) integrated GPU. Working
expectation, to be confirmed by P0-12 rather than assumed:

- H.264 and HEVC hardware decode: available.
- VP9 hardware decode: available.
- **AV1 hardware decode: not expected.** AV1 *decode* arrived with Intel Arc and later
  parts; the Xe-LP integrated GPU has AV1 *encode* but not decode. AV1 files should be
  expected to fall back to software decode, and the fallback must be *known and
  documented*, not discovered by a user.
- 10-bit HEVC: decode support exists, but 4K 10-bit is a realistic stress case on this
  GPU class.

P0-12 records the actual matrix. Nothing above is a tested result.

## Consequences

**Positive**
- Hardware decode, vsync-accurate presentation and audio passthrough all work.
- The backend can be restarted, upgraded or replaced without interrupting playback.
- Playback continues to work if the container is stopped (for files already resolved).

**Negative / accepted**
- Two trust domains in one product: a sandboxed backend and a host-side native player.
  The boundary between them must be written down (it is, above) and tested.
- The player cannot be fuzzed in the same harness as the backend; it needs its own
  malformed-media test corpus on Windows.
- IPC between UI and player becomes an internal API that deserves its own tests.

## Verification (required before Accepted)

1. **P0-12 — GPU/playback matrix.** For each of: H.264 1080p, H.264 4K, HEVC 10-bit 4K,
   AV1 4K, VP9, and an ASS-subtitle title — record: hardware decode engaged (GPU engine
   counters, not "looks smooth"), CPU %, dropped frames, audio format and passthrough
   result, seek latency. On this machine's Iris Xe, HEVC 10-bit and AV1 are the cases most
   likely to fall back to software; the fallback must be *known*, not discovered by a user.
2. **P0-11 — container GPU check.** Attempt hardware decode inside a container and record
   the actual failure. Test the three candidate paths so the conclusion rests on observed
   results rather than on this document's reasoning: `vaapi` (expect: no Linux driver
   stack), `nvdec` with the NVIDIA Container Toolkit (expect: no NVIDIA GPU on the
   reference machine), and `vulkan` video decode (expect: unproven on consumer Windows
   hardware). Record what was actually tried, including that the reference machine has
   no NVIDIA GPU, so the `nvdec` result is a hardware-availability observation and not a
   proof that `nvdec` would fail on an NVIDIA machine.
3. **Licence check (O-09).** Determine the libmpv build's licence and confirm it is
   compatible with the intended project licence. Blocks CP-0.

## Provisional decision (2026-11-18)

**Engine and integration: mpv as an independently managed process, communicating via JSON IPC.** Do not introduce libmpv directly into the main application at this stage.

- Implement playback inside crates/playback.
- Use mpv as the initial backend, driven through its JSON IPC interface.
- Define an abstract playback-engine interface so alternative backends (including libmpv) can be introduced later; never hardcode mpv specifics outside the crate.
- Keep process management, lifecycle, error handling and recovery inside the playback crate; support state, seeking, volume, subtitles, audio-track selection, position and events; preserve hardware-acceleration integration; allow independent terminate/restart.
- Licensing: prefer an integration that does not unnecessarily introduce GPL obligations into the main application; document mpv's licensing and distribution obligations; verify actual implications before distributing binaries.

## Acceptance record — build licence (2026-11-18)

**O-09 decided: the shipped mpv is an LGPL build.** The human partner chose the LGPL
route over a GPL build when asked directly, so the project is not forced into GPL by
its playback dependency. This settles *which* build is wanted; it does not prove the
artefact is one. Before distribution, the exact binary must be checked (built with
`-Dgpl=false` / `--enable-lgpl`, or its provenance confirmed), and P0-12's codec
matrix must state what the LGPL configuration loses. Recorded here by the agent at
the human's direction.
