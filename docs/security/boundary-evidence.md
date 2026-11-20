# Boundary evidence

Observed commands and outputs for the mandatory Docker/Compose boundary.
`PASS` / `FAIL` / `NOT RUN` describe runs on this machine at the recorded commit.
No checkpoint is passed by recording evidence here — a human reviews each CP.

## 2026-11-19 — A6: image builds (engine online, consent granted in-task)

| Check | Command | Result |
|---|---|---|
| core image build | `docker build -f infrastructure/containers/Dockerfile.core -t mediahaven-core:local .` | **PASS** exit 0, release compile 1m34s |
| probe image build | `docker build -f infrastructure/containers/Dockerfile.probe -t mediahaven-probe:local .` | **PASS** exit 0 |
| core image ID | `docker image inspect mediahaven-core:local --format '{{.Id}}'` | `sha256:9b7d00f6e68fd60ba5cf4385549afac1f51e1bf074e57cb895917e5e0d9cdf18` (146,776,635 bytes) |
| probe image ID | `docker image inspect mediahaven-probe:local --format '{{.Id}}'` | `sha256:f0d5077cca1c4937d2fe2c6d421a275fb0bbc6b3335d8e6deed0ee8ef55237ea` (390,014,945 bytes) |

### Build defects found and fixed (real, observed)

1. **probe ffprobe missing `libpulsecommon-16.1.so`**, then `libblas.so.3` — the
   original `COPY .../x86_64-linux-gnu/*.so*` glob skipped libraries in
   subdirectories (`pulseaudio/`, `blas/`). `ffprobe -version` exited 127.
   First fix (copy the whole multiarch tree) still missed `libblas.so.3`.
   Final fix: in the builder, resolve the full `ldd` closure of `ffprobe` and
   `ffmpeg` into a flat `/libs` dir (`cp -L`), copy it into the distroless
   runtime and set `LD_LIBRARY_PATH=/libs`. `ffprobe -version` now prints a
   valid version banner. Image size dropped from 1.10 GB to 390 MB.
2. **core fails closed without a token** — `docker run mediahaven-core:local`
   exited 1 with `MEDIAHAVEN_API_TOKEN is not set. Provide the install token, or
   set MEDIAHAVEN_DEV=1`. This is expected fail-closed behaviour, not a defect.

### Honest limitation on digest pinning

Locally built images have a content-addressable image ID but no registry
`RepoDigests`. `compose.yaml` requires `<image>@sha256:<digest>`; the image ID
above is the immutable local pin. A registry `RepoDigest` is only available
after an image is pushed to a registry, which has not happened and is not
authorised here. This must be revisited at P7 (release) before claiming
registry-grade pinning.

## 2026-11-19 — A8: auth matrix + isolation (live workspace)

Workspace started with a generated 64-hex token on `127.0.0.1:18081`:
`docker compose -f infrastructure/containers/compose.yaml up -d --wait` → exit 0,
both containers healthy.

### Auth matrix (live HTTP, script at `.superpowers/sdd/2026-11-19-remaining/auth-matrix.ps1`)

| Case | Expected | Observed |
|---|---|---|
| no token on `/settings_get` | 401 | **401** |
| wrong token (64 zeros) | 401 | **401** |
| valid signed `/settings_get` | 200 | **200** |
| tampered body (signature for `{}`, sent `{"x":1}`) | 401 | **401** |
| replay: valid signature reused on `/library_home` | 401 | **401** |
| stale timestamp (−120 s) | 401 | **401** |
| `GET /healthz` unauthenticated | 200 | **200** |
| `GET /readyz` without auth | 401 | **401** |
| `GET /readyz` signed | 200 | **200** |

All nine fail-closed / open-as-designed. Results also in
`.superpowers/sdd/2026-11-19-remaining/auth-results.txt`.

### `docker inspect` isolation assertions (observed)

| Assertion | core | probe |
|---|---|---|
| user | `10001:10001` | `10002:10002` |
| read-only rootfs | `true` | `true` |
| `cap_drop` | `["ALL"]` | `["ALL"]` |
| `no-new-privileges` | `true` | `true` |
| privileged | `false` | `false` |
| pids limit | 256 | 128 |
| memory limit | 536,870,912 (512M) | 1,073,741,824 (1G) |
| CPU limit (nano_cpus) | 2,000,000,000 (2.0) | — |
| Docker socket mounted | **no** | **no** |
| published port | `127.0.0.1:18081->8080` (loopback only) | none |

Mounts: core = `core-data` at `/var/lib/mediahaven` + `staging` at
`/var/lib/mediahaven/staging`; probe = `staging` only. No extra paths.

Network: core on `edge` (gateway 172.20.0.1) + `internal`; probe on `internal`
only with no gateway (`gw=invalid IP`). `docker exec mediahaven-probe ffprobe
-version` works (version 5.1.9), confirming the distroless runtime.

### Build defect found and fixed during A8

- **core could not create `library.db`** — named volumes are root-owned while the
  container runs as UID 10001 (exit 1, "unable to open database file", code 14).
  Fixed in `Dockerfile.core` by creating and `chown`ing `/var/lib/mediahaven`,
  `/var/lib/mediahaven/staging` and `/var/cache/mediahaven` to 10001 before
  `USER`. Volumes recreated (`down -v`); core then healthy.
- **probe restart loop** — bare `ffprobe` entrypoint exits 1 with no input. Fixed
  by copying `/bin/sleep` into the distroless runtime and using
  `ENTRYPOINT ["/usr/local/bin/sleep","infinity"]`; ffprobe/ffmpeg remain on PATH
  for on-demand `docker exec`. Probe then stable.

These are real defects found by running the workspace, now fixed.

### Shutdown

`docker compose down` → exit 0. Orphan check: **no `mediahaven-*` containers
remain** (PASS).

## A9 — mount-scope evidence

**NOT RUN.** Rendering a media-root read-only mount override and asserting exact
mount scope on the live engine is the next step; the supervisor code exists
(`src-tauri/src/mounts.rs`) but its live validation needs the app-side root
config path.

## 2026-11-20 — ADR-0015: executable boundary evidence (playback host wiring)

ADR-0015 was **Accepted 2026-11-20** (via ADR-0016), so code may depend on it.
All evidence below is from commands run on this machine on 2026-11-20.

### Host-internal source lookup is a GET, outside the renderer command surface

- `GET /playback_source/:media_id` is registered in the **authenticated**
  protected router (same middleware as `/readyz`); it is a GET, so the
  POST-only route-parity parser excludes it by construction.
- `crates/contract/src/lib.rs::COMMANDS` was **not** changed. The parity test
  `crates/contract/tests/route_parity.rs` still asserts exactly 24 commands and
  passed: `command_lists_derive_from_one_source ... ok`.
- There is no renderer command that issues this GET and no generic HTTP
  passthrough command, so the WebView cannot invoke the lookup. The signed GET
  is issued by the host forwarder (`get_source`) using the per-install token.

### Resolution and fail-closed tests (observed, headless)

| Check | Command | Result |
|---|---|---|
| Stored container path returned; unknown/stale row fails closed | `cargo test --workspace` → `media_source_path_returns_stored_path_and_fails_closed` | **PASS** |
| Resolver maps a declared root and refuses stale/outside paths | `cargo test --workspace` → `host_helper_resolves_declared_path_and_fails_closed_on_stale` | **PASS** |
| Resolver containment suite (escape, prefix, missing file) | `cargo test --workspace` → `media_playback` unit tests | **PASS** (13 passed, 1 ignored across crate) |
| Engine event shape carries only display numbers + media id | `cargo test --workspace` → `status_event_has_no_path_or_token_keys` | **PASS** |

The host helper `resolve_for_media(container_path, A9 host map)` calls
`media_playback::resolve_source`; a missing root key, a moved/deleted file, or
an escape all return `SourceError::Unavailable` and the host returns a fixed
`source unavailable` `CommandError`. No path is guessed or returned.

### No path or token in renderer-facing DTOs

- `PlaybackSession` is unchanged and path-less; `PlayerStateEvent` is
  `{ timePos, pause, volume, mediaId }` only (test asserts exactly four keys,
  no `path`/`token` substrings).
- Production bundle scan: `npm run check:bundle` →
  `check-bundle: production bundle clean (no dev token or signed HTTP client).`
  **PASS** exit 0.
- Renderer test `player driven by the engine > follows engine timePos and renders
  no filesystem path` asserts the rendered DOM contains no drive path or
  `/media/root` string. **PASS**.

### The new named host command `player_control`

`player_control(action, value)` is a **named host→engine control command** for
`play`, `pause`, `seek` and `volume`. It is deliberately *not* a renderer or
backend capability: it is not in `COMMANDS`, not a backend route, accepts no
path, token or arbitrary command name, and only drives the local mpv instance
that the host spawned. It cannot reach the container, the filesystem or the
Docker daemon. It is registered only in the Tauri `generate_handler!` list.

### NOT RUN

- **Live mpv playback / codec / hardware-decode smoke — NOT RUN.** mpv is not
  installed here and installing it requires human consent; the one live test
  remains `#[ignore]`. No file was opened by a real engine.
- `player://state` emission through a live Tauri WebView — **NOT RUN** (needs a
  display and the Tauri runtime). The payload shape is verified headlessly.
- Signed `GET /playback_source/:media_id` against a live `core` container —
  **NOT RUN** (needs engine consent). Route registration and auth middleware
  are unchanged from the verified `/readyz` precedent.
- **CP-3 remains OPEN.** No checkpoint was passed by recording this evidence.

## 2026-11-20 — A9: mount scope on the live engine

This section **supersedes the "A9 — mount-scope evidence: NOT RUN" note above**.
Docker engine consent was granted in-task. All values below were observed on
this machine. The workspace was started from
`infrastructure/containers/compose.yaml` plus the override the prior agent used,
`C:\Users\Stanley\AppData\Local\Temp\opencode\mh-a9-override.yaml` — confirmed by
the `com.docker.compose.project.config_files` label on both containers. Image
IDs: core `sha256:3d773f6f296526133712e961083754dda9efa1f815c133134840995fdfae0f4f`,
probe `sha256:a0f3da2c451ab3f2a719df12601b67b9062f4a43ff5eb59e90edcc4046cce6e8`.
The API token was injected as an environment variable; **its value is not
recorded here**.

### Command

```
docker inspect mediahaven-core
```

Verbatim relevant output (container `577e9fa8d906…`, healthy):

```
"Binds": [
    "mediahaven_staging:/var/lib/mediahaven/staging:rw",
    "/run/desktop/mnt/host/c/Users/Stanley/AppData/Local/Temp/opencode/mh-mount-root:/media/root-0:ro",
    "mediahaven_core-data:/var/lib/mediahaven:rw"
],
"NetworkMode": "mediahaven_edge",
"CapDrop": ["ALL"],
"Privileged": false,
"ReadonlyRootfs": true,
"SecurityOpt": ["no-new-privileges:true"],
"PidsLimit": 256,
"Memory": 536870912,
"NanoCpus": 2000000000,
"PortBindings": { "8080/tcp": [ { "HostIp": "127.0.0.1", "HostPort": "18082" } ] },
"Mounts": [
    { "Type": "bind",
      "Source": "/run/desktop/mnt/host/c/Users/Stanley/AppData/Local/Temp/opencode/mh-mount-root",
      "Destination": "/media/root-0", "Mode": "ro", "RW": false, "Propagation": "rprivate" },
    { "Type": "volume", "Name": "mediahaven_core-data",
      "Source": "/var/lib/docker/volumes/mediahaven_core-data/_data",
      "Destination": "/var/lib/mediahaven", "Mode": "rw", "RW": true },
    { "Type": "volume", "Name": "mediahaven_staging",
      "Source": "/var/lib/docker/volumes/mediahaven_staging/_data",
      "Destination": "/var/lib/mediahaven/staging", "Mode": "rw", "RW": true }
],
"User": "10001:10001",
"Networks": {
    "mediahaven_edge":    { "Gateway": "172.20.0.1", "IPAddress": "172.20.0.2" },
    "mediahaven_internal":{ "Gateway": "", "IPAddress": "172.18.0.3" }
}
```

No `docker.sock`, no `C:\`, no profile/Windows directory, no device mappings
(`"Devices": null`) and no `DeviceRequests` appear anywhere in the inspect.

### Command

```
docker exec mediahaven-core /bin/sh -c 'echo A9-RO-WRITE-TEST > /media/root-0/__a9_ro_write_test__ 2>&1; echo "WRITE_EXIT=$?"; if [ -e /media/root-0/__a9_ro_write_test__ ]; then echo "FILE_PRESENT=YES"; else echo "FILE_PRESENT=NO"; fi'
```

Verbatim output:

```
/media/root-0: 1: cannot create /media/root-0/__a9_ro_write_test__: Read-only file system
WRITE_EXIT=2
FILE_PRESENT=NO
```

### Command

```
docker inspect mediahaven-probe
```

Verbatim relevant output (container `666689ebe18c…`):

```
"Binds": [ "mediahaven_staging:/var/lib/mediahaven/staging:rw" ],
"NetworkMode": "mediahaven_internal",
"CapDrop": ["ALL"],
"Privileged": false,
"ReadonlyRootfs": true,
"SecurityOpt": ["no-new-privileges:true"],
"PidsLimit": 128,
"Memory": 1073741824,
"NanoCpus": 1000000000,
"Ports": {},
"Mounts": [
    { "Type": "volume", "Name": "mediahaven_staging",
      "Source": "/var/lib/docker/volumes/mediahaven_staging/_data",
      "Destination": "/var/lib/mediahaven/staging", "Mode": "rw", "RW": true }
],
"User": "10002:10002",
"Networks": {
    "mediahaven_internal": { "Gateway": "", "IPAddress": "172.18.0.2" }
}
```

### Result

| Assertion | core | probe | Result |
|---|---|---|---|
| Exactly the declared mounts, no more | `core-data`, `staging`, `/media/root-0:ro` | `staging` only | **PASS** |
| Media mount is read-only | `Mode: ro`, `RW: false`, write refused | no media mount | **PASS** |
| Docker socket mounted | no | no | **PASS** |
| Privileged | `false` | `false` | **PASS** |
| Read-only rootfs | `true` | `true` | **PASS** |
| `cap_drop: ALL` / `no-new-privileges` | yes / yes | yes / yes | **PASS** |
| Published port | `127.0.0.1:18082->8080` | none | **PASS** |
| Internal network gateway-free | `Gateway: ""` | `Gateway: ""` | **PASS** |

The prior agent's override renders exactly one root (`/media/root-0`) from a
temporary fixture directory containing `sample-a.txt` and `sample-b.mkv`; no
personal media was mounted. **NOT RUN:** removing a root from the rendered
override and re-inspecting to prove its mount disappears on the next `up` —
that needs a second render/recreate cycle and is left to the integration task.
**No checkpoint was passed by recording this evidence**; CP-0 stays OPEN.

## 2026-11-20 — A9: mount-scope proof (live engine; Docker consent granted in-task)

This section supersedes the earlier `A9 — mount-scope evidence / NOT RUN` placeholder
above. All values below were observed on this machine at HEAD
`7d485075a7c7873917150b226ac077dbc57e1eb0`. **No checkpoint is passed by this record.**

### Preconditions and image identity

| Item | Observed |
|---|---|
| Engine | Docker Engine 29.9.3, Docker Desktop WSL2 backend, kernel `6.18.58.1-microsoft-standard-WSL2`, driver `overlayfs`, NCPU 20 |
| `docker info` | `MemTotal=8073875456`, `SecurityOptions=[name=seccomp,profile=builtin name=cgroupns]` (no AppArmor) |
| core image actually used | `sha256:3d773f6f296526133712e961083754dda9efa1f815c133134840995fdfae0f4f` (146,789,249 bytes) |
| probe image actually used | `sha256:a0f3da2c451ab3f2a719df12601b67b9062f4a43ff5eb59e90edcc4046cce6e8` (390,093,468 bytes) |

**Image IDs changed since the 2026-11-19 A6 record** (core was
`sha256:9b7d00f6…`, probe `sha256:f0d5077c…`): repository sources and both Dockerfiles
are modified in the working tree, so the locally built image IDs are no longer the ones
recorded in A6. This is an observation, not a defect claim; the IDs listed above are the
ones actually started for this test.

### Fixture and override

Test root (approved temp dir, no personal data):
`C:\Users\Stanley\AppData\Local\Temp\opencode\mh-mount-root`
containing `sample-a.txt` (72 bytes) and `sample-b.mkv` (77 bytes — a plain-text
placeholder, **not** a real video).

Override written to `C:\Users\Stanley\AppData\Local\Temp\opencode\mh-a9-override.yaml`,
exact bytes:

```yaml
services:
  core:
    volumes:
      - "C:\\Users\\Stanley\\AppData\\Local\\Temp\\opencode\\mh-mount-root:/media/root-0:ro"
```

This is the shape `render_mounts` emits: `services.core.volumes` entries of
`"<host>:<container>:ro"`, with the host path JSON/YAML-escaped (backslashes doubled) so
a Windows path cannot break out of the scalar. **Deviation from the literal renderer
output:** on Windows, `std::fs::canonicalize` prefixes the host path with `\\?\`, so the
renderer's verbatim output would begin `"\\\\?\\C:\\…"`. The live test used the
Docker-acceptable equivalent without that verbatim prefix. Whether Docker Desktop
accepts the literal `\\?\`-prefixed bind source is **UNTESTED** (see Concerns).

`docker compose config` (merged, abbreviated to the relevant block) showed exactly:

```yaml
    volumes:
      - type: volume
        source: core-data
        target: /var/lib/mediahaven
      - type: volume
        source: staging
        target: /var/lib/mediahaven/staging
      - type: bind
        source: C:\Users\Stanley\AppData\Local\Temp\opencode\mh-mount-root
        target: /media/root-0
        read_only: true
```

### Startup

Command (env: disposable 64-hex test token, the two local image IDs above, port 18082):

```
docker compose -f infrastructure/containers/compose.yaml -f C:\Users\Stanley\AppData\Local\Temp\opencode\mh-a9-override.yaml up -d --wait
```

Output (PowerShell wrapped native progress on stderr as `NativeCommandError`; text verbatim):

```
 Network mediahaven_edge Creating
 Container mediahaven-probe Creating
 Network mediahaven_edge Created
 Container mediahaven-core Creating
 Container mediahaven-probe Created
 Container mediahaven-core Created
 Container mediahaven-probe Starting 
 Container mediahaven-core Starting 
 Container mediahaven-probe Started 
 Container mediahaven-core Started 
 Container mediahaven-probe Waiting 
 Container mediahaven-core Waiting 
 Container mediahaven-probe Healthy 
 Container mediahaven-core Healthy 
ELAPSED_MS=7148
EXITCODE=0
```

Result: **PASS** — both containers healthy, exit 0, 7148 ms cold start.

### `docker inspect mediahaven-core` — exact mount scope

Command: `docker inspect mediahaven-core --format '{{json .Mounts}}'`

```json
[{"Type":"bind","Source":"/run/desktop/mnt/host/c/Users/Stanley/AppData/Local/Temp/opencode/mh-mount-root","Destination":"/media/root-0","Mode":"ro","RW":false,"Propagation":"rprivate"},{"Type":"volume","Name":"mediahaven_core-data","Source":"/var/lib/docker/volumes/mediahaven_core-data/_data","Destination":"/var/lib/mediahaven","Driver":"local","Mode":"rw","RW":true,"Propagation":""},{"Type":"volume","Name":"mediahaven_staging","Source":"/var/lib/docker/volumes/mediahaven_staging/_data","Destination":"/var/lib/mediahaven/staging","Driver":"local","Mode":"rw","RW":true,"Propagation":""}]
```

Exactly three mounts: the one `/media/root-0` `ro` bind plus `core-data` and `staging`.
No extra paths, no `docker.sock`, no host profile/drive roots. `mediahaven-probe` had
only the `staging` volume. Isolation fields from the same inspect:

```
Privileged=false ReadonlyRootfs=true CapDrop=["ALL"] SecurityOpt=["no-new-privileges:true"] User=10001:10001 PidMode= NetworkMode=mediahaven_edge Memory=536870912 NanoCpus=2000000000 PidsLimit=256
```

Result: **PASS** — mount scope and isolation assert as declared.

### Read-only enforcement from inside `core`

| Command (inside `docker exec mediahaven-core …`) | Output (verbatim message) | Exit |
|---|---|---|
| `touch /media/root-0/write-test.txt` | `touch: cannot touch '/media/root-0/write-test.txt': Read-only file system` | 1 |
| `sh -c 'echo x > /media/root-0/write-test2.txt'` | `sh: 1: cannot create /media/root-0/write-test2.txt: Read-only file system` | 2 |
| `touch /tmp/mh-control.txt` (control, writable path) | *(no output)* | 0 |
| `head -c 40 /media/root-0/sample-a.txt` (control, read) | `MEDIA_HAVEN A9 mount-scope proof - harml` | 0 |

Host-side fixture directory after the attempts still contained only `sample-a.txt` (72)
and `sample-b.mkv` (77) — no write-test files. Result: **PASS** — the mount is genuinely
read-only, not merely declared `ro` in Compose.

### Shutdown and anomaly

`docker compose … down` → `DOWN_EXIT=0`; orphan check `docker ps -a --filter
name=mediahaven` → empty; no `mediahaven_*` networks remained; port 18082 had no listener.

Anomaly recorded honestly: the harness server restarted mid-session. The Docker engine
named pipe was briefly unavailable (`failed to connect to the docker API at
npipe:////./pipe/dockerDesktopLinuxEngine …`), then recovered; the two containers came
back under their restart policies. The mount JSON was re-inspected after recovery and was
byte-identical to the value above. This did not change any result.

**A9 live-engine acceptance checks: PASS for the read-only mount scope on this machine.**
A human must still review before any CP is signed off.

## 2026-11-20 — Task 2: qBittorrent sandbox + download volume

All values below were observed on this machine. **No checkpoint is passed by this record.**

### Preconditions

| Item | Observed |
|---|---|
| Engine | Docker Engine 29.9.3 |
| core image | `sha256:9cc5a0c79dda4c08b295fc7786214d33def2e3c5b7fc3f89bc175374cde261f1` (147 MB) |
| probe image | `sha256:67a2305f36ed98a4a4a9be2b15b07e650fee7528d3a3d7afea1a4dd8b7100298` (390 MB) |
| qBittorrent image | `qbittorrentofficial/qbittorrent-nox@sha256:43b2d352cf76330a7ca34c7320b9b74227d6f31e662e0e123d0457206b09a7ea` (headless/nox, 363,506,164 bytes). Multi-arch manifest digest obtained by `docker buildx imagetools inspect qbittorrentofficial/qbittorrent-nox:latest`; pinned by registry digest, not a mutable tag. Pulled by digest for the live verification. |
| Tokens | two host-generated 64-hex values (API + qB WebUI); **values not recorded** |
| API port | 18083 (loopback) |

No other project's containers or the 14 preserved volumes were touched. The pulled qB
image and the two new empty named volumes remain on the host after verification;
no system dependency was installed.

### Static checklist greps (observed)

```
Select-String -Path infrastructure/containers/compose.yaml -Pattern 'privileged|/var/run/docker\.sock|network_mode'
FORBIDDEN_MATCHES=0
Select-String -Path infrastructure/containers/compose.yaml -Pattern '^\s+image:' | Where-Object { $_.Line -notmatch '@sha256:' }
UNPINNED_IMAGES=0
```

### `docker compose config --quiet`

Command (all six required vars set):

```
docker compose -f infrastructure/containers/compose.yaml config --quiet
CONFIG_ALL_VARS_EXIT=0
```

Command (all required vars removed):

```
docker compose -f infrastructure/containers/compose.yaml config --quiet
CONFIG_NO_VARS_EXIT=1
```

Verbatim errors (all six required variables reported missing — fail-closed, including
the two new qB variables):

```
error while interpolating services.core.environment.MEDIAHAVEN_API_TOKEN: required variable MEDIAHAVEN_API_TOKEN is missing a value: generated per install by the host supervisor
error while interpolating services.core.image: required variable MEDIAHAVEN_CORE_IMAGE is missing a value: must be <image>@sha256:<verified-digest>
error while interpolating services.core.ports.[]: required variable MEDIAHAVEN_API_PORT is missing a value: set by the host supervisor
error while interpolating services.probe.image: required variable MEDIAHAVEN_PROBE_IMAGE is missing a value: must be <image>@sha256:<verified-digest>
error while interpolating services.qbittorrent.environment.QB_WEBUI_TOKEN: required variable MEDIAHAVEN_QB_TOKEN is missing a value: host-generated
error while interpolating services.qbittorrent.image: required variable MEDIAHAVEN_QB_IMAGE is missing a value: must be <image>@sha256:<digest>
```

### Startup

```
docker compose -f infrastructure/containers/compose.yaml up -d
UP_EXIT=0
```

Created networks `mediahaven_edge`, `mediahaven_internal`, `mediahaven_acquire` and
volumes `mediahaven_downloads`, `mediahaven_qb-config`; started all three containers.

### Container state (observed, honest)

```
docker ps -a --filter name=mediahaven --format '{{.Names}} | {{.Status}} | {{.Ports}}'
mediahaven-qbittorrent | Exited (134) 22 seconds ago |
mediahaven-probe | Up 24 seconds |
mediahaven-core | Up 25 seconds (healthy) | 127.0.0.1:18083->8080/tcp
```

**Defect observed:** the qBittorrent container starts then aborts with exit 134.
Verbatim tail of `docker logs mediahaven-qbittorrent`:

```
mkdir: can't create directory '/config/qBittorrent/': Permission denied
/entrypoint.sh: line 36: can't create /config/qBittorrent/config/qBittorrent.conf: nonexistent directory
Could not create required directory '/config/qBittorrent/cache'
```

Cause: the brief's `user: "10003:10003"` is applied to a root-owned named volume;
the official image's entrypoint only chowns `/config` when it runs as root, so as
UID 10003 it cannot initialise its config dir. A custom `Dockerfile.qbittorrent`
that pre-chowns `/config` and `/downloads` to 10003 (Docker copies image-dir
ownership into a fresh named volume) is the smallest fix — **not implemented here**
(brief pinned the official image by digest; recorded as a concern for the parent).
The boundary assertions below are configuration facts and are unaffected by this
liveness defect.

### `docker inspect` assertions (observed)

```
docker inspect mediahaven-qbittorrent --format '...'
USER=10003:10003 CAPDROP=["ALL"] SEC=["no-new-privileges:true"] PORTBIND={} PORTS={}
NETWORKS=mediahaven_acquire(gw=invalid IP,ip=invalid IP)
```

`gw/ip=invalid IP` because the aborted container released its address; the network
object below is the container-state-independent egress proof.

```
docker inspect mediahaven-qbittorrent --format '{{json .Mounts}}'
[{"Type":"volume","Name":"mediahaven_qb-config","Destination":"/config","Mode":"rw","RW":true},{"Type":"volume","Name":"mediahaven_downloads","Destination":"/downloads","Mode":"rw","RW":true}]
```

Only the two declared named volumes — no `docker.sock`, no host/bind path, no device.

```
docker inspect mediahaven-probe --format '...'
USER=10002:10002 CAPDROP=["ALL"] PORTS={} NETWORKS=mediahaven_internal(gw=invalid IP,ip=172.21.0.3)
```

Probe remains on `internal` only, with no gateway (gateway-free), no published port.

```
docker network inspect mediahaven_acquire --format '...'
NAME=mediahaven_acquire INTERNAL=false DRIVER=bridge IPAM=[{"Subnet":"172.18.0.0/16","Gateway":"172.18.0.1"}]
docker network inspect mediahaven_internal --format '...'
NAME=mediahaven_internal INTERNAL=true DRIVER=bridge IPAM=[{"Subnet":"172.21.0.0/16","Gateway":"172.21.0.1"}]
```

`acquire` is a normal bridge with a gateway (egress allowed). `internal` has
`Internal: true`, so Docker blocks all external routing; the container sees no
gateway. No service published a port to the LAN; qBittorrent WebUI has no published
port at all.

| Assertion | Result |
|---|---|
| qBittorrent: no published port (`PORTBIND={}`, `PORTS={}`) | PASS |
| qBittorrent: no `docker.sock` / host bind / device | PASS |
| qBittorrent: `cap_drop ALL`, `no-new-privileges`, non-root 10003 | PASS |
| qBittorrent: attached only to `acquire` (egress bridge, gateway 172.18.0.1) | PASS |
| probe: still on gateway-free `internal` only | PASS |
| acquire network: egress allowed (`Internal: false`) | PASS |
| Fail-closed when required vars missing (exit 1) | PASS |

### Rust checks (observed, `CARGO_TARGET_DIR=target/verify`)

| Check | Command | Result |
|---|---|---|
| format | `cargo fmt --all` | **PASS** exit 0 |
| lint | `cargo clippy -p mediahaven-desktop --all-targets -- -D warnings` | **PASS** exit 0 |
| tests | `cargo test -p mediahaven-desktop` | **PASS** exit 0 — 32 passed, 0 failed (incl. `download_root_renders_one_read_only_downloads_bind_with_verbatim_stripped`) |

### Shutdown

```
docker compose -f infrastructure/containers/compose.yaml down
DOWN_EXIT=0
```

Orphan check: `docker ps -a --filter name=mediahaven` and
`docker network ls --filter name=mediahaven` both returned empty — no orphan
containers or networks. The two empty named volumes remain (plain `down`, no `-v`).

### NOT RUN / concerns

- **qBittorrent liveness — FAIL on this machine** (exit 134, permission denied on
  `/config`; see Defect above). Boundary config is correct; a follow-up custom image
  is needed for the engine to actually run.
- WebUI reachability from `core` on `acquire` is **NOT RUN**: the base compose does
  not attach `core` to `acquire`; that connection (needed by Task 3's
  `http://qbittorrent:8080`) is not part of this task's scoped YAML.
- Live read-only `/downloads` bind into `core` is **NOT RUN** — it is rendered at
  runtime by the supervisor override; only the pure renderer test was exercised.
- LAN non-reachability of the WebUI (second-machine test) is a human evidence gate;
  **NOT RUN**.
- **No checkpoint was passed by recording this evidence.**

## 2026-11-20 — Task 2 fix: qBittorrent liveness (controller-authorised)

Supersedes the "qB liveness FAIL (exit 134)" finding in the Task 2 section above.
All values below observed on this machine. **No checkpoint is passed by this record.**

### Fix

Added `infrastructure/containers/Dockerfile.qbittorrent`: `FROM` the same pinned
official digest, `USER root` + `RUN mkdir -p /config /downloads && chown -R
10003:10003 /config /downloads` + `USER 10003:10003` (no packages). Compose gained a
`build:` stanza referencing it; the `image:` env var still requires the built
image's digest (no `latest`). Stale `mediahaven_downloads` / `mediahaven_qb-config`
volumes (from the failed first attempt) were removed so the new ownership was
copied into fresh volumes.

```
docker build -f infrastructure/containers/Dockerfile.qbittorrent -t mediahaven-qbittorrent:local infrastructure/containers
BUILD_EXIT=0
docker image inspect mediahaven-qbittorrent:local --format 'ID={{.Id}} SIZE={{.Size}}'
ID=sha256:fde0f374ef039c04e2204df8dcc0083cef9dbd07dcd3505dd450f03f91d2fd3e SIZE=363519153
```

Build image digest used for `MEDIAHAVEN_QB_IMAGE` (core/probe digests, port 18083
and two host-generated 64-hex tokens — values not recorded — as before).

### Config + live result

```
docker compose -f infrastructure/containers/compose.yaml config --quiet   # all vars   -> CONFIG_ALL_VARS_EXIT=0
docker compose -f infrastructure/containers/compose.yaml config --quiet   # no vars    -> CONFIG_NO_VARS_EXIT=1
docker compose -f infrastructure/containers/compose.yaml up -d                          -> UP_EXIT=0
```

Container state 8 s after `up` (verbatim):

```
mediahaven-qbittorrent | Up 9 seconds |
mediahaven-probe | Up 8 seconds |
mediahaven-core | Up 9 seconds (healthy) | 127.0.0.1:18083->8080/tcp
```

Result: **PASS** — no exit 134; the pre-chown fixed the runtime blocker.

WebUI response, loopback in the qB container:

```
docker exec mediahaven-qbittorrent /bin/sh -c 'wget -qO- http://127.0.0.1:8080/'
<!DOCTYPE html>
... <meta name="description" content="qBittorrent WebUI">
```

WebUI from `core` over `acquire` — a temporary override attached `core` to
`acquire` for this check only (deleted afterwards; scoped compose unchanged):

```
docker exec mediahaven-core /bin/sh -c 'curl -sS -o /dev/null -w HTTP_CODE=%{http_code} http://qbittorrent:8080/'
HTTP_CODE=200
```

Result: **PASS** — WebUI reachable on the container network, still with no
published port to the LAN.

Boundary re-inspect after the fix (qB while Up, not exited):

```
USER=10003:10003 CAPDROP=["ALL"] SEC=["no-new-privileges:true"] PORTBIND={} PORTS={}
NETWORKS=mediahaven_acquire(gw=172.21.0.1,ip=172.21.0.2)
Mounts: mediahaven_qb-config -> /config rw, mediahaven_downloads -> /downloads rw (no docker.sock/bind)
```

### Teardown, checks, storage

```
docker compose -f infrastructure/containers/compose.yaml -f <temp-override> down   -> DOWN_EXIT=0
docker ps -a --filter name=mediahaven        -> empty
docker network ls --filter name=mediahaven   -> empty
```

`cargo fmt --all` exit 0; `cargo clippy -p mediahaven-desktop --all-targets -- -D
warnings` exit 0; `cargo test -p mediahaven-desktop` exit 0 (32 passed, 0 failed),
all with `CARGO_TARGET_DIR=target/verify`.

`docker rmi qbittorrentofficial/qbittorrent-nox@sha256:43b2d352…` → exit 0 (unused
by any container; the built image shared its layers and stayed intact). No other
project's image or preserved volume was touched.

### NOT RUN / concerns

- WebUI authentication (`QB_WEBUI_TOKEN` is injected but not enforced by the
  official image) — **NOT RUN**; later task.
- Real torrent acquisition — **NOT RUN** (no source supplied; no bundled sources).
- `docker compose up -d --build` path — **NOT RUN** (built via `docker build`).
- `core` still not attached to `acquire` in scoped compose (Task 3 needs it).
- **No checkpoint was passed by recording this evidence.**

## 2026-11-20 — Task 3: WebUI authentication + download-root runtime wiring

All values below were observed on this machine. **No checkpoint is passed by
this record.** Two fresh host-generated 64-hex values were injected (API token
and qB WebUI password); **their values are not recorded** and no secret appears
in source, an image, a log or a renderer DTO.

### WebUI authentication (mandatory Task 2 review item 1)

The official qBittorrent image ignores `QB_WEBUI_TOKEN`; its generated
`qBittorrent.conf` had no operator password. `Dockerfile.qbittorrent` now
installs `/usr/local/bin/mh-qb-init`, which fails closed when the secret is
absent and generates `/config/qBittorrent/config/qBittorrent.conf` at container
start from `$QB_WEBUI_TOKEN`. qB 5.3 verifies with PBKDF2-HMAC-SHA512 (100000
iterations, 16-byte salt, 64-byte key) stored as the literal
`"<base64 salt>:<base64 key>"` inside a quoted `@ByteArray(...)` value. This
format was derived from the pinned 5.3.0 source and confirmed by two live
observations: a generated conf accepted a login, and qB's own
`app/setPreferences` wrote the same shape. python3 ships in the pinned base
image (no new package); if a future base drops it, the overlay needs a rebuild.
The no-published-port boundary remains the compensating control.

| Item | Observed |
|---|---|
| `docker build -f infrastructure/containers/Dockerfile.qbittorrent -t mediahaven-qbittorrent:local infrastructure/containers` | **exit 0** |
| `docker image inspect mediahaven-qbittorrent:local --format 'ID={{.Id}} SIZE={{.Size}}'` | `sha256:329db0cd857cd00da9d74710740866fa1b26a6b5654ba3a0fa23264433a31fab`, 363,563,667 bytes (~44 KB over the base) |
| `docker exec mediahaven-qbittorrent /bin/sh -c 'grep -c Password_PBKDF2 /config/qBittorrent/config/qBittorrent.conf'` | **1** (config generated in `/config`) |

`compose.yaml` now injects the same secret into `core` as
`MEDIAHAVEN_QB_TOKEN` and attaches `core` to `acquire`. Observed from inside
`core` over the container network (script piped over stdin; command line and
evidence contain no secret):

```
TOKEN_PRESENT=yes
LOGIN_HTTP=204
AUTH_INFO_HTTP=200
UNAUTH_INFO_HTTP=403
AUTH_BODY=[]
```

| Assertion | Result |
|---|---|
| `POST /api/v2/auth/login` with injected secret → 204 + session cookie | **PASS** |
| `GET /api/v2/torrents/info` with cookie → 200 `[]` | **PASS** |
| Same API without cookie → 403 (WebUI not open) | **PASS** |

Boundary after `up` (all three containers Up; core healthy):

```
QB_PORTS={} QB_NETS=mediahaven_acquire CAPDROP=["ALL"] USER=10003:10003
CORE_NETS=mediahaven_acquire mediahaven_edge mediahaven_internal
```

No published port on qB; `core` reaches the WebUI only on `acquire`; no
`docker.sock`, host bind or device on qB (unchanged from Task 2).

### Download-root runtime wiring (mandatory Task 2 review item 2)

`renderer.rs::root_specs` now sets `RootSpec.download_root` from a persisted
host acquisition directory (`%APPDATA%\MEDIA_HAVEN\acquisition-root.txt`,
overridable by `MEDIAHAVEN_DOWNLOAD_ROOT`). The same specs drive the override
fragment and the host map. Rust integration test
`acquisition_root_is_rendered_and_added_to_the_host_map` observed: exactly one
`/downloads:ro` bind in the fragment and `map["/downloads"] ==
canonicalize(downloads)`.

Verification-only override (temp file
`C:\Users\Stanley\AppData\Local\Temp\opencode\mh-task3-ro-override.yaml`,
deleted after teardown) rendered the same shape against the live engine:

```
SOURCE=/run/desktop/mnt/host/c/Users/Stanley/AppData/Local/Temp/opencode/mh-task3-downloads DEST=/downloads MODE=ro RW=False
TASK3 RO FIXTURE
touch: cannot touch '/downloads/write-test': Read-only file system
WRITE_EXIT=1
FILE_PRESENT=NO
```

Result: **PASS** — the rendered `/downloads` bind is genuinely read-only and
its files are readable from `core`. Production supervisor rendering itself was
not exercised (no supervisor UI in this task); only the shared renderer output.

### Commands and exact exit codes

| Check | Command | Result |
|---|---|---|
| format | `cargo fmt --all` | **exit 0** |
| lint | `cargo clippy -p media-acquisition -p mediahaven-desktop --all-targets -- -D warnings` | **exit 0** |
| client tests | `cargo test -p media-acquisition` | **exit 0** — 3 passed (add sends magnet+save path; remove `deleteFiles` only when asked; 401 → `QbError::Api`) |
| host tests | `cargo test -p mediahaven-desktop` | **exit 0** — 33 passed (was 32; +1 integration assertion) |
| config, all vars | `docker compose -f infrastructure/containers/compose.yaml config --quiet` | **exit 0** |
| config, no vars | same, vars removed | **exit 1** — all seven required vars reported missing (fail-closed) |
| startup | `docker compose … up -d` | **exit 0** |
| teardown | `docker compose … down` | **exit 0** |
| orphans | `docker ps -a --filter name=mediahaven` / `docker network ls --filter name=mediahaven` | both empty |
| whitespace | `git diff --check` | **exit 0** |

All `cargo` commands used `CARGO_TARGET_DIR=target/verify`. No new target dir,
no large pull, no install, no host feature change and no commit.

### NOT RUN / concerns

- Real torrent acquisition — **NOT RUN** (no source supplied; lawful use, no
  bundled sources).
- Second-machine LAN non-reachability of the WebUI — human evidence gate,
  **NOT RUN**.
- The qB client resolves `add`'s hash from `/torrents/info` (qB's add endpoint
  returns no hash); magnets are matched by URI. `.torrent`-file sources are not
  covered by that lookup and remain Task 4's concern.
- The qB container's `downloads` named volume and the host acquisition
  directory rendered into `core` are separate stores; the supervisor must
  render the matching host bind for qB as well before real files flow end to
  end (outside this task's scope).
- **No checkpoint was passed by recording this evidence.**

## 2026-11-20 — Task 3 fix: single acquisition store (Critical review finding)

**This section supersedes the "separate stores" concern immediately above.**
The Critical finding is closed: qB now writes to the *same* host directory the
supervisor binds into `core` and mpv resolves. All values below were observed
on this machine. **No checkpoint is passed by this record.**

### Change

`compose.yaml`: `qbittorrent.volumes` now has
`${MEDIAHAVEN_DOWNLOAD_ROOT:?host acquisition directory}:/downloads:rw`
instead of the named `downloads` volume; the unused `downloads:` declaration
was removed. `core-data`, `staging`, `qb-config`, core's three networks, the
loopback-only published port and qB's `cap_drop ALL` / `no-new-privileges` /
no-published-port / `acquire` boundary are unchanged. `Dockerfile.qbittorrent`
comment updated: its build-time chown helps the `/config` named volume but not
the `/downloads` host bind.

### Observed checks

| Check | Command / observation | Result |
|---|---|---|
| config, all vars | `docker compose -f infrastructure/containers/compose.yaml config --quiet` (incl. `MEDIAHAVEN_DOWNLOAD_ROOT`) | **exit 0** |
| config, no vars | same, all vars removed | **exit 1** |
| config, download root only missing | same, other six vars set | **exit 1** — `required variable MEDIAHAVEN_DOWNLOAD_ROOT is missing` |
| startup | `docker compose … up -d` (with a verification-only core override binding the same host dir `ro`) | **exit 0** — all three containers Up, core healthy |
| qB `/downloads` mount | `docker inspect mediahaven-qbittorrent --format '{{json .Mounts}}'` filtered to `/downloads` | `TYPE=bind DEST=/downloads MODE=rw RW=True` |
| qB writes as its own UID | `docker exec -i mediahaven-qbittorrent /bin/sh -s` script: `id`, then `printf … > /downloads/qb-write-test.txt` | `uid=10003 gid=10003`, `WRITE_EXIT=0` |
| file visible on Windows host | `Test-Path` + `Get-Content` on `…\mh-task3-hostbind\qb-write-test.txt` | **host file present**, content `qb-write-test` |
| same file from `core` | `docker exec mediahaven-core /bin/sh -c "cat /downloads/qb-write-test.txt"` | `qb-write-test`, **exit 0** |
| same file from `qB` | `docker exec mediahaven-qbittorrent /bin/sh -c "cat /downloads/qb-write-test.txt"` | `qb-write-test`, **exit 0** |
| WebUI auth still enforced after fix | login **204**, cookie-authenticated info **200 `[]`**, unauthenticated info **403** | **PASS** |
| teardown | `docker compose … down` | **exit 0** |
| orphans | `docker ps -a` / `docker network ls` filtered `mediahaven` | both empty |
| persistence after down | host file still present with same content | **YES** |

Permission alignment: on this Docker Desktop (WSL2) host, a Windows bind is
writable by the container's UID 10003 without a chown — the write above is the
observation, not an assumption. A build-time chown would not have helped a bind.

### Notes / concerns

- The supervisor must set `MEDIAHAVEN_DOWNLOAD_ROOT` to the same host directory
  for both `compose up` and the renderer's `acquisition_root()`; compose now
  fails closed when it is absent. The renderer still has an APPDATA default for
  standalone use, but production cannot start without the explicit value.
- An empty stale `mediahaven_downloads` named volume from earlier verification
  runs remains on this host; it is no longer declared or attached by compose.
- No real torrent was acquired (lawful use, no source supplied), so qB's own
  download path was not exercised end to end; the write permission was proven
  with a test file created by the container user.
- **No checkpoint was passed by recording this evidence.**

## 2026-11-20 — Task 6: completion → coordinated cleanup (ADR-0019)

Gates recorded in the conversion plan: G-4 completion = B5 policy (watched ≥90%
+ near-end + EOF; seeking/close never complete); G-5 replay grace = 24 h;
G-8 quota = 20 GB. **No checkpoint is passed by this record.**

### Boundaries asserted in code and shown by tests

| Property | Where | Result |
|---|---|---|
| Only app-managed files deletable: `execute` refuses when there is no `file_mappings` row (empty mapping set) and when the torrent hash is absent | `crates/acquisition/src/cleanup.rs` | **PASS** — `media_with_no_file_mapping_row_deletes_zero_files` |
| Source media under a user library root is never deleted: paths must be under the `/downloads` container root; traversal and non-`/downloads` paths are refused before any delete | `is_managed_path`, `execute` | **PASS** — `outside_root_paths_are_refused_before_any_deletion`, `managed_path_guard_accepts_only_downloads` (in the same test) |
| Unfinished / playing / unverified / prefetch-target items are never eligible | `is_eligible` | **PASS** — `eligibility_matrix_admits_only_the_fully_eligible_case`, `unfinished_playing_and_unverified_files_are_never_eligible`, `b5_adversarial_cases_never_confirm_completion` |
| Multi-episode torrent: `deleteFiles=false` when a sibling episode is still needed; only orphaned files are deleted. Fully orphaned torrent uses `deleteFiles=true` | `removal_mode`, `CleanupWorker::execute` | **PASS** — `multi_episode_torrent_keeps_needed_files_and_deletes_only_orphans`, `fully_orphaned_torrent_is_removed_with_delete_files` |
| A failed deletion/torrent removal/verification reconciles to `failed`, never `removed` | `reconcile` | **PASS** — `failed_file_deletion_reconciles_to_failed_never_removed`, `torrent_remove_failure_reconciles_to_failed`, `unverified_deletion_never_marks_removed` |
| Renderer DTOs carry no path | `ManagedFile`/`CleanupRepository::ManagedFileRow` are host-internal, non-serde structs; `DownloadsScreen` unchanged | **PASS** — static review; no serialization path from cleanup to renderer |
| Two new cleanup routes stay under `/host/`, out of the 24-command POST parity surface | `crates/core/src/acquisition.rs` | **PASS** — `host_routes_are_outside_the_renderer_command_namespace`, `cargo test -p mediahaven-contract --test route_parity` exit 0 (`command_lists_derive_from_one_source ... ok`) |
| No new dependency | No `Cargo.toml` change in `acquisition`/`persistence`; `core` reused its existing `media-persistence`, `chrono`, `sqlx` deps | **PASS** — static review |
| No migration change needed | Tables `cleanup_entries`, `file_mappings`, `download_items`, `prefetch_entries` already exist in `0004_acquisition.sql` (Task 1) | **PASS** — `cargo test -p media-persistence --test migrations` exit 0 |

### The release-blocker file-count proof

`media_with_no_file_mapping_row_deletes_zero_files` creates three harmless temp
fixture files, constructs a media id with **no** `file_mappings` row, runs the
worker with an injected real `std::fs::remove_file` deleter, and asserts the
file count is unchanged (3 → 3), every fixture still exists, no torrent `remove`
call was made, and reconciliation yields `failed` (never `removed`). All
filesystem effects are temp fixtures; **live deletion against real media is
NOT RUN** (no lawful source, and deleting source media is forbidden).

### Trigger semantics

`/host/acquisition_cleanup_confirm` accepts the B5 inputs from the host player
and only writes a `pending` cleanup entry (with `eligible_at = now + 24 h`) when
`is_complete` returns true. `playback_checkpoint` calls
`on_playback_checkpoint(media_id)` after persisting progress; it reads that
record and calls the pure `is_eligible` only for confirmed rows, and never
deletes inline. Seeking / close / queue-removal / app-close produce no
confirmation, so they never reach cleanup. Actual deletion is the separate
`/host/acquisition_cleanup_pass` operation (ready for a scheduler); it
re-evaluates the full exclusion matrix and verifies each file is gone before
writing `removed`.

### Commands and exact exit codes

All Rust commands used `CARGO_TARGET_DIR=target/verify` (no new target dir, no
install, no host change, no commit).

| Check | Command | Result |
|---|---|---|
| format | `cargo fmt --all` | **exit 0** |
| format check | `cargo fmt --all -- --check` | **exit 0** |
| lint | `cargo clippy -p media-acquisition -p media-persistence -p mediahaven-core --all-targets -- -D warnings` | **exit 0** |
| tests | `cargo test -p media-acquisition -p media-persistence -p mediahaven-core` | **exit 0** — 73 passed, 0 failed, 1 ignored (`searching_10k_items_is_within_budget`); 12/12 Task 6 cleanup tests pass |
| 24-command parity | `cargo test -p mediahaven-contract --test route_parity` | **exit 0** — 4/4, `command_lists_derive_from_one_source ... ok` |
| whitespace | `git diff --check -- crates/acquisition crates/persistence/src/sqlite crates/core/src docs/security/boundary-evidence.md` | **exit 0** |

One transient `LNK1104` linker file-lock occurred on the `cleanup` test binary
on the first attempt (known AV lock in `target/verify`); a straight retry
succeeded with exit 0. No code change was made for it.

### NOT RUN / concerns

- **Live deletion against a real qBittorrent task and real media — NOT RUN**
  (no lawful source; source media must never be deleted). The file-count proof
  and both multi-episode branches are exercised with temp fixtures and fakes.
- The scheduled pass is a callable host operation; no wall-clock timer was
  introduced (no scheduler dependency or host change was authorised in scope).
- Frontend cleanup-status rendering was not added — that file surface was
  outside this task's scope. Status is persisted honestly in
  `cleanup_entries.state`/`outcome` and the host pass returns a processed count;
  exposing it in the renderer is a follow-up decision for the parent.
- **No checkpoint was passed by recording this evidence.**
