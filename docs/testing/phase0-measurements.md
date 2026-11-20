# Phase 0 — live measurements (2026-11-20)

Evidence only. **No checkpoint was passed by recording this document** — CP-0
requires human review. Consent was granted in-task for Docker engine use and
host measurement only. No Windows feature, firewall, network, proxy, DNS or
security setting was changed; no software was installed; no admin rights were
used; no source, Playwright config or `vite.config.ts` was touched.

Commands were run in PowerShell from the repo root. Native-command stderr is
transcribed as the meaningful program output; PowerShell's `NativeCommandError`
wrapper lines are omitted from the excerpts. The API token was injected via
environment variable and **its value is not recorded**.

## Machine and workspace

| Property | Observed value |
|---|---|
| Host | Windows 11, Intel Core i7-13700H (14C/20T), Intel Iris Xe, no NVIDIA GPU; 16 GB host RAM |
| WSL distros present | `OmniRushBox`, `docker-desktop` |
| WSL2 VM memory ceiling | `MemTotal: 7884648 kB` = 7,884,648,048 bytes ≈ 7.52 GiB (`wsl -d docker-desktop -- cat /proc/meminfo`) |
| Engine storage driver | `overlayfs` (from `docker inspect`, both containers) |
| Images | core `sha256:3d773f6f296526133712e961083754dda9efa1f815c133134840995fdfae0f4f`; probe `sha256:a0f3da2c451ab3f2a719df12601b67b9062f4a43ff5eb59e90edcc4046cce6e8` |
| Compose files | `infrastructure/containers/compose.yaml` + `C:\Users\Stanley\AppData\Local\Temp\opencode\mh-a9-override.yaml` (one `/media/root-0:ro` fixture mount) |
| API port | `18082` |

**Timeline caveat, stated honestly.** Another agent's workspace was running at
the start of this task and supplied the authoritative mount-scope inspect in
`docs/security/boundary-evidence.md`. It was removed externally mid-task; the
workspace was recreated twice with the *same* compose files, override and image
IDs for the footprint/decode measurements, then torn down by this agent. The
first instance's container ID was `577e9fa8d906…` (core) / `666689ebe18c…`
(probe); recreated instances had new IDs. This does not change any assertion
below, but the inspect and the stats are from different container instances.

---

## P0-13 — host reachability / LAN non-reachability

| Check | Command | Verbatim output | Result |
|---|---|---|---|
| Published port bound loopback only | `Get-NetTCPConnection -LocalPort 18082` | `127.0.0.1  18082  Listen  4544` (single row) | **PASS** |
| No portproxy rule | `netsh interface portproxy show all` | *(empty output)* | **PASS** |
| No unauthenticated Docker TCP endpoint | `Get-NetTCPConnection -LocalPort 2375,2376` | *(empty output)* | **PASS** |
| WSL networking mode not mirrored | `Get-Content C:\Users\Stanley\.wslconfig` | `--- .wslconfig ABSENT ---` | **PASS** (no mirrored override; default NAT) |
| `probe` network has no gateway | `docker inspect mediahaven-probe` | `"NetworkMode": "mediahaven_internal"`, `"Gateway": ""`, `"Ports": {}` | **PASS** |
| Host HTTP GET to the port | — | — | **NOT RUN** in this task (binding observed; the 9-case live auth matrix is recorded in `docs/security/boundary-evidence.md` at port 18081) |
| Reachability from a second LAN machine | — | — | **NOT RUN**: no second machine was provided or authorised, and creating a `portproxy` rule or opening a firewall port is outside this task's consent. This is the one acceptance criterion P0-13 cannot close from a single machine. |

`.wslconfig` absent means mirrored mode is not configured; it is not a positive
proof of the engine's internal networking mode, but it removes the R-18
blocking condition documented in `docs/architecture/docker-workspace.md` §3.1.

---

## P0-10 / P0-14 — workspace footprint

### Container CPU and memory (three samples)

Command, repeated three times with a 2 s gap:

```
docker stats --no-stream --format "{{.Name}}|{{.CPUPerc}}|{{.MemUsage}}|{{.MemPerc}}"
```

Verbatim output:

```
SAMPLE 1
mediahaven-core|3.10%|3.777MiB / 512MiB|0.74%
mediahaven-probe|0.00%|1.422MiB / 1GiB|0.14%
SAMPLE 2
mediahaven-core|0.00%|3.777MiB / 512MiB|0.74%
mediahaven-probe|0.00%|1.422MiB / 1GiB|0.14%
SAMPLE 3
mediahaven-core|0.00%|3.602MiB / 512MiB|0.70%
mediahaven-probe|0.00%|1.422MiB / 1GiB|0.14%
```

| Container | MEM % samples | Median MEM % | CPU % samples | Median CPU % | Resident set |
|---|---|---|---|---|---|
| core | 0.74, 0.74, 0.70 | **0.74 %** (of 512 MiB) | 3.10, 0.00, 0.00 | **0.00 %** | 3.60–3.78 MiB |
| probe | 0.14, 0.14, 0.14 | **0.14 %** (of 1 GiB) | 0.00, 0.00, 0.00 | **0.00 %** | 1.42 MiB |

`docker stats` MEM % is a percentage of the container's own limit, not of host
or VM RAM; CPU % is Docker's reported lifetime average while idle, not a peak.
An earlier probe instance (before external teardown) sampled 3.43 / 3.375 /
3.375 MiB and 0.33 %; it was idle too. The difference between instances is
recorded rather than averaged away.

### Disk footprint

Command and verbatim output:

```
docker system df
TYPE            TOTAL     ACTIVE    SIZE      RECLAIMABLE
Images          24        12        17.64GB   8.822GB (50%)
Containers      12        0         31.05MB   31.05MB (100%)
Local Volumes   14        3         170.3MB   48.16MB (28%)
Build Cache     67        0         10.26GB   4.97GB
```

These are **daemon-wide** totals on a shared engine (12 unrelated Supabase
containers are present, stopped), not the MEDIA_HAVEN-only delta. An isolated
daemon baseline was **NOT RUN**.

### Not measured

| Item | Status | Reason |
|---|---|---|
| Container cold start | **UNTESTED** | Checked both containers' labels (`com.docker.compose.*`) — no cold-start field. No prior-agent cold-start value was found in `progress.md` or the evidence directory. My own `up -d` was not instrumented, so no number is claimed. |
| Engine-only / core-only stepwise cost | **UNTESTED** | Only the combined `core`+`probe` state was sampled; the engine-alone and core-only rows of the P0-14 table were not captured. |
| Shared-engine effect (R-16) | **UNTESTED** now | Prior docs record 12 unrelated containers on the same daemon; no isolated-daemon comparison was run. |
| 1080p playback RAM/CPU | **NOT RUN** | Needs a playback engine and fixture media; outside this task's consent and scope. |
| WSL VM memory ceiling | **PASS** | 7,884,648 kB observed from `docker-desktop`, matching the ≈8 GB figure in `docs/architecture/docker-workspace.md` §5.1. |

`Development.md` §9.1 hardware tiers are **not confirmed or amended** by these
numbers: no full-stack, playback-active profile was measured.

---

## P0-11 — containerised hardware decode

All attempts ran inside `mediahaven-probe` (ffmpeg from the distroless probe
image; `LD_LIBRARY_PATH=/libs`). A 1-second 320×240 H.264 fixture was generated
in-container first:

```
docker exec mediahaven-probe ffmpeg -hide_banner -loglevel error -y -f lavfi -i testsrc=duration=1:size=320x240:rate=10 -c:v libx264 -pix_fmt yuv420p /tmp/a9-h264-test.mp4
```

Exit 0; the file probed as `Video: h264 (High) ... yuv420p, 320x240, 10 fps`.
Compiled hardware methods (`ffmpeg -hide_banner -hwaccels`):

```
Hardware acceleration methods:
vdpau
cuda
vaapi
qsv
drm
opencl
vulkan
```

`nvdec` is not listed by `-hwaccels`, but `-hwaccel nvdec` resolves to a CUDA
device, so the attempt below is recorded under that name.

### VAAPI

```
docker exec mediahaven-probe ffmpeg -hide_banner -loglevel info -hwaccel vaapi -i /tmp/a9-h264-test.mp4 -f null -
```

Verbatim error:

```
Device creation failed: -542398533.
[h264 @ ...] No device available for decoder: device type vaapi needed for codec h264.
Stream mapping:
  Stream #0:0 -> #0:0 (h264 (native) -> wrapped_avframe (native))
Device setup failed for decoder on input stream #0:0 : Generic error in an external library
VAAPI_EXIT=1
```

**FAIL — no VAAPI device.** `/dev/dri/renderD128` is absent:
`docker exec mediahaven-probe ffmpeg -hide_banner -loglevel info -i /dev/dri/renderD128`
→ `/dev/dri/renderD128: No such file or directory`, exit 1.

### NVDEC (hardware-availability observation only)

```
docker exec mediahaven-probe ffmpeg -hide_banner -loglevel info -hwaccel nvdec -i /tmp/a9-h264-test.mp4 -f null -
```

Verbatim error:

```
[AVHWDeviceContext @ ...] Cannot load libcuda.so.1
[AVHWDeviceContext @ ...] Could not dynamically load CUDA
Device creation failed: -1.
[h264 @ ...] No device available for decoder: device type cuda needed for codec h264.
Stream mapping:
  Stream #0:0 -> #0:0 (h264 (native) -> wrapped_avframe (native))
Device setup failed for decoder on input stream #0:0 : Operation not permitted
NVDEC_EXIT=1
```

**FAIL on this machine.** This reference machine has an **Intel Iris Xe iGPU and
no NVIDIA GPU**, so this is a hardware-availability observation. It is **not**
proof that `nvdec` would fail on an NVIDIA machine with the NVIDIA Container
Toolkit; that case remains **UNTESTED**.

### Vulkan

```
docker exec mediahaven-probe ffmpeg -hide_banner -loglevel info -hwaccel vulkan -i /tmp/a9-h264-test.mp4 -f null -
```

Verbatim error:

```
[AVHWDeviceContext @ ...] Instance creation failure: VK_ERROR_INCOMPATIBLE_DRIVER
Device creation failed: -542398533.
[h264 @ ...] No device available for decoder: device type vulkan needed for codec h264.
Stream mapping:
  Stream #0:0 -> #0:0 (h264 (native) -> wrapped_avframe (native))
Device setup failed for decoder on input stream #0:0 : Generic error in an external library
VULKAN_EXIT=1
```

**FAIL — no compatible Vulkan driver/device visible in-container.**

### Display and audio inside the container

```
docker exec mediahaven-probe ffmpeg -hide_banner -loglevel info -f lavfi -i testsrc=size=320x240:rate=10 -pix_fmt yuv420p -frames:v 1 -f xv :0
```

Verbatim error:

```
[xv @ ...] Could not open the X11 display '(null)'
Could not write header for output file #0 (incorrect codec parameters ?): Invalid argument
Error initializing output stream 0:0 --
Conversion failed!
XV_YUV_EXIT=1
```

**Display: unavailable.** No `DISPLAY` or `WAYLAND_DISPLAY` in either
container's environment; no devices passed (`"Devices": null`); `/proc/asound`
is in `MaskedPaths`.

```
docker exec mediahaven-probe ffmpeg -hide_banner -loglevel info -f alsa -i default -t 1 -f null -
```

Verbatim error:

```
ALSA lib conf.c:4555:(snd_config_update_r) Cannot access file /usr/share/alsa/alsa.conf
ALSA lib pcm.c:2666:(snd_pcm_open_noupdate) Unknown PCM default
[alsa @ ...] cannot open audio device default (No such file or directory)
default: Input/output error
EXIT=1
```

**Audio: unavailable.** Software decode to `-f null -` works, so these are
device/driver availability failures, not a broken ffmpeg build. ADR-0002's
host-placement rationale is supported by observation; the ADR itself was not
edited (outside this task's file scope).

---

## Teardown

```
docker compose -f infrastructure/containers/compose.yaml -f "C:\Users\Stanley\AppData\Local\Temp\opencode\mh-a9-override.yaml" down
```

with `MEDIAHAVEN_API_TOKEN` (64 hex, value redacted), `MEDIAHAVEN_CORE_IMAGE` /
`MEDIAHAVEN_PROBE_IMAGE` set to the image IDs above and `MEDIAHAVEN_API_PORT=18082`.

Verbatim output:

```
Container mediahaven-probe Stopped
Container mediahaven-probe Removing
Container mediahaven-probe Removed
Container mediahaven-core Stopped
Container mediahaven-core Removing
Container mediahaven-core Removed
Network mediahaven_internal Removed
Network mediahaven_edge Removed
DOWN_EXIT=0
```

Orphan check — `docker ps -a --filter name=mediahaven`, `Get-NetTCPConnection
-LocalPort 18082`, `docker ps` — all returned **empty**. **PASS: zero orphans,
zero listeners, zero running containers.** Named volumes `mediahaven_core-data`
and `mediahaven_staging` were intentionally left in place (`down`, not
`down -v`) so no data was deleted.
