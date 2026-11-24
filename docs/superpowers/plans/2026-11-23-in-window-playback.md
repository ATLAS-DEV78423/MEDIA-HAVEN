# In-window playback Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Play media inside MEDIA_HAVEN's single Tauri window instead of a second mpv window, without weakening the host/container boundary.

**Architecture:** Keep mpv as an external, user-supplied binary. Add a Win32 child HWND owned by the main window, report its rectangle from React through one host-only named command, and pass it to mpv as `--wid=<HWND>`. All existing IPC, path resolution, completion and security gates stay unchanged.

**Tech Stack:** Rust 1.99, Tauri 2, `windows` 0.62 (add one feature flag, no new crate), React 19 + TypeScript, Vitest.

**Spec:** `docs/superpowers/specs/2026-11-23-in-window-playback-design.md`

## Global Constraints

- Windows 11 x64 only. No new crate, no new runtime service, no bundling of mpv.
- `windows = { version = "0.62", features = ["Win32_Security_Cryptography", "Win32_Foundation"] }` is already in `[workspace.dependencies]`; add only `Win32_UI_WindowsAndMessaging`.
- `COMMANDS` stays at 24 entries. `player_control` is already host-only and outside the parity set; `player_attach_surface` follows that exact pattern. Do not add a backend route or a DTO.
- No filesystem path, token, URL or HWND value ever enters a renderer-facing payload. The renderer sends four finite numbers and receives nothing.
- mpv stays a user-supplied binary (`MEDIAHAVEN_MPV` or `mpv` on PATH). Never install or download it silently.
- No commit, push, ADR acceptance, checkpoint sign-off or host/network change without explicit human permission. Task "Commit" steps are **blocked** until that permission is given; do them as separate uncommitted changes otherwise.
- Use `CARGO_TARGET_DIR=target/verify` for cargo verification to avoid the known `LNK1104` default-target file lock.
- Frontend commands run in `apps/desktop`; there is no root `package.json`.

---

## File Structure

- Create: `apps/desktop/src-tauri/src/win_surface.rs` — pure rect validation/DPI math plus Win32 child-window create/position/show/destroy. One responsibility: the native surface.
- Create: `spikes/mpv-embed/src/main.rs`, `spikes/mpv-embed/Cargo.toml` — throwaway live probe, no production use.
- Modify: `Cargo.toml` — add the `windows` feature flag.
- Modify: `crates/playback/src/mpv.rs` — argument construction and `spawn_with_parent`.
- Modify: `apps/desktop/src-tauri/src/player.rs` — own the surface, fail closed when absent, hide on stop, destroy on drop.
- Modify: `apps/desktop/src-tauri/src/commands/renderer.rs` — the `player_attach_surface` command.
- Modify: `apps/desktop/src-tauri/src/lib.rs` — module declaration and `generate_handler!` entry.
- Modify: `apps/desktop/src/services/player.ts` — renderer bridge for the command.
- Modify: `apps/desktop/src/pages/Player/PlayerPage.tsx` and `PlayerPage.test.tsx` — attach before `playbackStart`, update on resize.

---

### Task 1: Blocking live spike (throwaway)

**Files:**
- Create: `spikes/mpv-embed/Cargo.toml`
- Create: `spikes/mpv-embed/src/main.rs`

**Interfaces:**
- Consumes: an mpv binary path in `MEDIAHAVEN_MPV`; a short generated video fixture (any real video file you are authorised to use).
- Produces: observed PASS/FAIL for the four gate questions; written into `progress.md` in Task 7. No production code depends on this crate.

- [ ] **Step 1: Check the precondition**

Run: `$env:MEDIAHAVEN_MPV`; `Get-Command mpv -ErrorAction SilentlyContinue`
Expected: a path, or a clear "not installed" result. **If absent, STOP this task and ask the human to supply a binary or consent to fetching one.** Do not download an executable on your own.

- [ ] **Step 2: Write the probe**

`spikes/mpv-embed/Cargo.toml`:
```toml
[package]
name = "mpv-embed-spike"
version = "0.0.0"
edition = "2021"

[dependencies]
windows = { version = "0.62", features = ["Win32_Foundation", "Win32_UI_WindowsAndMessaging", "Win32_System_LibraryLoader"] }
```

`spikes/mpv-embed/src/main.rs` (create a host window, a STATIC child, spawn mpv with `--wid`, print observations):
```rust
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::process::{Command, Stdio};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, EnumWindows, GetWindowTextW, GetWindowTextLengthW,
    WS_CHILD, WS_EX_TRANSPARENT, WS_VISIBLE, WINDOW_EX_STYLE, WINDOW_STYLE,
};

fn main() {
    unsafe {
        let module = GetModuleHandleW(None).unwrap();
        let class_name = OsString::from("STATIC").encode_wide().chain(Some(0)).collect::<Vec<_>>();
        let parent = CreateWindowExW(
            WINDOW_EX_STYLE::default(), windows::core::PCWSTR(class_name.as_ptr()),
            windows::core::w!("mpv embed spike"), WINDOW_STYLE::default(),
            0, 0, 960, 540, HWND::default(), None, module, None,
        ).unwrap();
        let child = CreateWindowExW(
            WS_EX_TRANSPARENT, windows::core::PCWSTR(class_name.as_ptr()),
            windows::core::w!("surface"), WS_CHILD | WS_VISIBLE,
            0, 0, 960, 480, parent, None, module, None,
        ).unwrap();
        println!("parent={:?} child={:?}", parent.0, child.0);

        let mpv = std::env::var_os("MEDIAHAVEN_MPV").unwrap_or(OsString::from("mpv"));
        let mut child_process = Command::new(mpv)
            .arg(format!("--wid={}", child.0 as usize))
            .arg("--idle=yes")
            .arg("--no-config")
            .arg(std::env::var_os("MEDIAHAVEN_TEST_MEDIA").unwrap_or(OsString::from("fixture.mkv")))
            .stdin(Stdio::null())
            .spawn()
            .expect("spawn mpv");
        println!("mpv pid={}", child_process.id());

        // Enumerate top-level window titles: the probe's own window plus mpv's.
        let mut titles: Vec<String> = Vec::new();
        let _ = EnumWindows(Some(|hwnd, _| {
            unsafe {
                let len = GetWindowTextLengthW(hwnd) as usize;
                if len > 0 {
                    let mut buf = vec![0u16; len + 1];
                    let n = GetWindowTextW(hwnd, &mut buf);
                    titles.push(String::from_utf16_lossy(&buf[..n as usize]));
                }
            }
            true
        }), 0);
        println!("top_level_titles={titles:?}");
        std::thread::sleep(std::time::Duration::from_secs(8));
        let _ = child_process.kill();
        let _ = child_process.wait();
    }
}
```

- [ ] **Step 3: Run the probe and record observations**

Run: `$env:MEDIAHAVEN_TEST_MEDIA = "<authorised fixture>"; cargo run -p mpv-embed-spike`
Expected: prints `parent`/`child` handles and `top_level_titles`. Gate passes only if **no title belonging to mpv appears in `top_level_titles`** and the video is visibly inside the probe window. Note whether `WS_EX_TRANSPARENT` still delivers mouse input to the parent.
Expected on failure: STOP, record the observed titles, and reopen the spec decision — do not proceed to Tasks 2–6.

---

### Task 2: Pure surface math and mpv argument construction

**Files:**
- Modify: `Cargo.toml` (workspace dependency feature)
- Create: `apps/desktop/src-tauri/src/win_surface.rs`
- Modify: `crates/playback/src/mpv.rs`

**Interfaces:**
- Consumes: nothing outside the two crates.
- Produces: `win_surface::SurfaceRect { x, y, width, height }`, `win_surface::validate_rect(f64,f64,f64,f64) -> Option<SurfaceRect>`, `win_surface::to_physical(SurfaceRect, u32) -> SurfaceRect`; `mpv::mpv_args(&str, Option<usize>) -> Vec<String>`; `MpvPlayer::spawn_with_parent(&Path, Option<usize>)`.

- [ ] **Step 1: Add the Windows feature flag**

In `Cargo.toml` change line 43 to:
```toml
windows = { version = "0.62", features = ["Win32_Security_Cryptography", "Win32_Foundation", "Win32_UI_WindowsAndMessaging"] }
```

- [ ] **Step 2: Write failing tests for rect validation**

In `apps/desktop/src-tauri/src/win_surface.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_non_finite_negative_and_absurd_rects() {
        assert!(validate_rect(f64::NAN, 0.0, 10.0, 10.0).is_none());
        assert!(validate_rect(0.0, -1.0, 10.0, 10.0).is_none());
        assert!(validate_rect(0.0, 0.0, 0.0, 10.0).is_none());
        assert!(validate_rect(0.0, 0.0, 10.0, 200_000.0).is_none());
    }
    #[test]
    fn converts_logical_rect_to_physical_pixels() {
        let physical = to_physical(validate_rect(10.0, 20.0, 100.0, 50.0).unwrap(), 144);
        assert_eq!(physical, SurfaceRect { x: 15, y: 30, width: 150, height: 75 });
    }
}
```
Add `mod win_surface;` near the top of `apps/desktop/src-tauri/src/lib.rs`.

- [ ] **Step 3: Run the tests, expect compile failure**

Run: `$env:CARGO_TARGET_DIR="target/verify"; cargo test -p mediahaven-desktop --lib win_surface`
Expected: FAIL — `validate_rect`, `to_physical` and `SurfaceRect` are undefined.

- [ ] **Step 4: Implement the pure functions**

In `win_surface.rs`:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

pub fn validate_rect(x: f64, y: f64, width: f64, height: f64) -> Option<SurfaceRect> {
    let values = [x, y, width, height];
    if !values.iter().all(|v| v.is_finite()) { return None; }
    if x < 0.0 || y < 0.0 || width < 1.0 || height < 1.0 { return None; }
    if values.iter().any(|v| *v > 100_000.0) { return None; }
    Some(SurfaceRect {
        x: x as i32, y: y as i32, width: width as i32, height: height as i32,
    })
}

pub fn to_physical(rect: SurfaceRect, dpi: u32) -> SurfaceRect {
    let scale = dpi as f64 / 96.0;
    let each = |v: i32| -> i32 { ((v as f64 * scale).round() as i64).clamp(i32::MIN as i64, i32::MAX as i64) as i32 };
    SurfaceRect {
        x: each(rect.x), y: each(rect.y),
        width: each(rect.width).max(1), height: each(rect.height).max(1),
    }
}
```

- [ ] **Step 5: Run the tests, expect PASS**

Run: `$env:CARGO_TARGET_DIR="target/verify"; cargo test -p mediahaven-desktop --lib win_surface`
Expected: PASS — 2 tests.

- [ ] **Step 6: Write a failing test for mpv arguments**

In `crates/playback/src/mpv.rs` tests:
```rust
#[test]
fn mpv_args_embed_only_when_a_parent_is_given() {
    let embedded = super::mpv_args("pipe-1", Some(4242));
    assert!(embedded.iter().any(|a| a == "--wid=4242"));
    assert!(embedded.iter().any(|a| a == "--input-ipc-server=pipe-1"));
    assert!(embedded.iter().any(|a| a == "--idle=yes"));
    assert!(embedded.iter().any(|a| a == "--no-config"));
    let standalone = super::mpv_args("pipe-2", None);
    assert!(!standalone.iter().any(|a| a.starts_with("--wid=")));
}
```

- [ ] **Step 7: Run it, expect compile failure**

Run: `$env:CARGO_TARGET_DIR="target/verify"; cargo test -p media-playback --lib mpv_args_embed_only_when_a_parent_is_given`
Expected: FAIL — `mpv_args` undefined.

- [ ] **Step 8: Implement `mpv_args` and `spawn_with_parent`**

Replace the body of `MpvPlayer::spawn` in `crates/playback/src/mpv.rs` with a call to the new constructor, and add:
```rust
fn mpv_args(pipe_path: &str, parent: Option<usize>) -> Vec<String> {
    let mut args = vec![
        format!("--input-ipc-server={pipe_path}"),
        "--idle=yes".to_string(),
        "--hwdec=auto-safe".to_string(),
        "--no-config".to_string(),
    ];
    if let Some(hwnd) = parent {
        args.push(format!("--wid={hwnd}"));
    }
    args
}

impl MpvPlayer<ConnectedIpc, ProcessChild> {
    pub fn spawn(mpv_exe: &Path) -> Result<Self, PlayerError> {
        Self::spawn_with_parent(mpv_exe, None)
    }

    /// Spawn mpv idle, optionally embedding it in a parent HWND (Task 1 spike).
    /// Argument vectors only; no shell. The child is killed on drop.
    pub fn spawn_with_parent(mpv_exe: &Path, parent: Option<usize>) -> Result<Self, PlayerError> {
        let pipe_path = unique_ipc_path();
        let mut command = Command::new(mpv_exe);
        for arg in mpv_args(&pipe_path, parent) {
            command.arg(arg);
        }
        let child = command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(PlayerError::Spawn)?;
        let transport = match ipc::connect(&pipe_path, Duration::from_secs(5)) {
            Ok(transport) => transport,
            Err(err) => {
                let mut child = child;
                let _ = child.kill();
                let _ = child.wait();
                return Err(PlayerError::Ipc(err));
            }
        };
        Ok(Self::with_parts(transport, Some(ProcessChild(child)), Some(pipe_path)))
    }
}
```
Keep every existing test in `mpv.rs` untouched.

- [ ] **Step 9: Run the crate tests, expect PASS**

Run: `$env:CARGO_TARGET_DIR="target/verify"; cargo test -p media-playback`
Expected: PASS — all existing tests plus the new one.

---

### Task 3: Native surface lifecycle in `win_surface.rs`

**Files:**
- Modify: `apps/desktop/src-tauri/src/win_surface.rs`

**Interfaces:**
- Consumes: `SurfaceRect`, `to_physical` from Task 2.
- Produces: `pub struct Surface { parent: usize, hwnd: usize }`, `pub fn create_surface(parent: usize, rect: SurfaceRect, dpi: u32) -> Result<Surface, SurfaceError>`, `pub fn position_surface(&self, rect: SurfaceRect, dpi: u32)`, `pub fn show_surface(&self, show: bool)`, and `Drop` destroying the child. `Surface` is `Send`. `SurfaceError::Create` is host-internal.

- [ ] **Step 1: Write a pure test for the style bits**

```rust
#[test]
fn child_window_styles_are_transparent_to_hit_testing() {
    // The child must not steal focus/clicks from the WebView2 control bands.
    assert_eq!(CHILD_EX_STYLE & WS_EX_TRANSPARENT, WS_EX_TRANSPARENT);
    assert_ne!(CHILD_STYLE & WS_CHILD, WINDOW_STYLE::default());
}
```

- [ ] **Step 2: Run it, expect compile failure**

Run: `$env:CARGO_TARGET_DIR="target/verify"; cargo test -p mediahaven-desktop --lib child_window_styles`
Expected: FAIL — constants undefined.

- [ ] **Step 3: Implement the lifecycle**

```rust
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, SetWindowPos, ShowWindow,
    SW_HIDE, SW_SHOW, SWP_NOACTIVATE, SWP_NOZORDER, WINDOW_EX_STYLE, WINDOW_STYLE,
    WS_CHILD, WS_EX_NOACTIVATE, WS_EX_TRANSPARENT, WS_VISIBLE, HWND_TOP,
};

pub const CHILD_EX_STYLE: WINDOW_EX_STYLE = WS_EX_TRANSPARENT | WS_EX_NOACTIVATE;
pub const CHILD_STYLE: WINDOW_STYLE = WS_CHILD | WS_VISIBLE;

#[derive(Debug, thiserror::Error)]
pub enum SurfaceError {
    #[error("could not create the video surface")]
    Create,
}

pub struct Surface {
    hwnd: HWND,
}

impl Surface {
    pub fn create(parent: usize, rect: SurfaceRect, dpi: u32) -> Result<Self, SurfaceError> {
        let physical = to_physical(rect, dpi);
        unsafe {
            let module = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
                .map_err(|_| SurfaceError::Create)?;
            let class = windows::core::w!("STATIC");
            let hwnd = CreateWindowExW(
                CHILD_EX_STYLE, class, windows::core::w!("mediahaven-video-surface"),
                CHILD_STYLE,
                physical.x, physical.y, physical.width, physical.height,
                HWND(parent as *mut core::ffi::c_void), None, module, None,
            ).map_err(|_| SurfaceError::Create)?;
            Ok(Self { hwnd })
        }
    }

    pub fn hwnd(&self) -> usize { self.hwnd.0 as usize }

    pub fn position(&self, rect: SurfaceRect, dpi: u32) {
        let p = to_physical(rect, dpi);
        unsafe {
            let _ = SetWindowPos(self.hwnd, HWND_TOP, p.x, p.y, p.width, p.height,
                SWP_NOZORDER | SWP_NOACTIVATE);
        }
    }

    pub fn show(&self, show: bool) {
        unsafe { let _ = ShowWindow(self.hwnd, if show { SW_SHOW } else { SW_HIDE }); }
    }
}

impl Drop for Surface {
    fn drop(&mut self) { unsafe { let _ = DestroyWindow(self.hwnd); } }
}

unsafe impl Send for Surface {}
```

- [ ] **Step 4: Run it, expect PASS**

Run: `$env:CARGO_TARGET_DIR="target/verify"; cargo test -p mediahaven-desktop --lib child_window_styles`
Expected: PASS — 1 test.

---

### Task 4: Host-only `player_attach_surface` command and fail-closed playback

**Files:**
- Modify: `apps/desktop/src-tauri/src/player.rs`
- Modify: `apps/desktop/src-tauri/src/commands/renderer.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `win_surface::{Surface, SurfaceRect, validate_rect}` (Task 2/3), `mpv::MpvPlayer::spawn_with_parent` (Task 2).
- Produces: `PlayerState::attach_surface(x,y,w,h,dpi,parent) -> Result<(), PlayerError>`, `PlayerError::NoSurface`, and the Tauri command `player_attach_surface(x: f64, y: f64, width: f64, height: f64, state, app)`. `playback_start` consumes the attached surface.

- [ ] **Step 1: Write the failing test for the no-surface branch**

In `player.rs` tests:
```rust
#[test]
fn no_surface_fails_closed_instead_of_opening_a_window() {
    let state = PlayerState::new();
    let err = state.surface_hwnd().expect_err("no attached surface");
    assert!(matches!(err, PlayerError::NoSurface));
}
```

- [ ] **Step 2: Run it, expect compile failure**

Run: `$env:CARGO_TARGET_DIR="target/verify"; cargo test -p mediahaven-desktop --lib no_surface_fails_closed`
Expected: FAIL — `surface_hwnd` and `PlayerError::NoSurface` undefined.

- [ ] **Step 3: Extend `PlayerState`**

Add to `PlayerError`:
```rust
#[error("player surface unavailable")]
NoSurface,
#[error("invalid surface rectangle")]
InvalidSurface,
#[error("video surface: {0}")]
Surface(#[from] crate::win_surface::SurfaceError),
```
Add fields to `PlayerState`:
```rust
surface: Arc<Mutex<Option<crate::win_surface::Surface>>>,
```
Initialise `surface: Arc::new(Mutex::new(None))` in `new()`. Add:
```rust
pub fn attach_surface(
    &self, x: f64, y: f64, width: f64, height: f64, dpi: u32, parent: usize,
) -> Result<(), PlayerError> {
    let rect = crate::win_surface::validate_rect(x, y, width, height)
        .ok_or(PlayerError::InvalidSurface)?;
    let mut guard = self.surface.lock().map_err(|_| PlayerError::Lock)?;
    match guard.as_mut() {
        Some(surface) => surface.position(rect, dpi),
        None => *guard = Some(crate::win_surface::Surface::create(parent, rect, dpi)?),
    }
    if let Some(surface) = guard.as_mut() { surface.show(true); }
    Ok(())
}

pub fn surface_hwnd(&self) -> Result<usize, PlayerError> {
    let guard = self.surface.lock().map_err(|_| PlayerError::Lock)?;
    guard.as_ref().map(|s| s.hwnd()).ok_or(PlayerError::NoSurface)
}
```
In `open_media`, before locking `inner`, resolve `let parent = self.surface_hwnd()?;` and pass it to `MpvPlayer::spawn_with_parent(&self.mpv_exe, Some(parent))`. In `stop()`, after clearing `inner`, hide the surface: `if let Ok(guard) = self.surface.lock() { if let Some(s) = guard.as_ref() { s.show(false); } }`.

- [ ] **Step 4: Run the test, expect PASS**

Run: `$env:CARGO_TARGET_DIR="target/verify"; cargo test -p mediahaven-desktop --lib no_surface_fails_closed`
Expected: PASS — 1 test.

- [ ] **Step 5: Write the command and register it**

In `commands/renderer.rs`:
```rust
/// Host-only: attach the native video surface inside our single window.
/// Accepts four finite numbers; never a path, token, HWND value or URL.
#[tauri::command]
pub fn player_attach_surface(
    x: f64, y: f64, width: f64, height: f64,
    state: tauri::State<'_, PlayerState>,
    app: tauri::AppHandle,
) -> Result<Value, CommandError> {
    let parent = app
        .get_webview_window("main")
        .and_then(|window| window.hwnd().ok())
        .map(|hwnd| hwnd.0 as usize)
        .ok_or_else(|| CommandError::new(AppErrorCode::InvalidRequest))?;
    let dpi = app
        .get_webview_window("main")
        .and_then(|window| window.scale_factor().ok())
        .map(|factor| (factor * 96.0).round() as u32)
        .unwrap_or(96);
    (*state)
        .attach_surface(x, y, width, height, dpi, parent)
        .map_err(|_| CommandError::new(AppErrorCode::InvalidRequest))?;
    Ok(Value::Null)
}
```
Add `commands::player_attach_surface,` after `commands::player_control,` in `generate_handler!` in `lib.rs`.

- [ ] **Step 6: Verify the command surface is still 24**

Run: `$env:CARGO_TARGET_DIR="target/verify"; cargo test -p mediahaven-contract command_lists_derive_from_one_source`
Expected: PASS. `COMMANDS` is untouched; the new command is host-only, like `player_control`.

- [ ] **Step 7: Run the host crate tests**

Run: `$env:CARGO_TARGET_DIR="target/verify"; cargo test -p mediahaven-desktop --lib`
Expected: PASS — all existing tests plus the new one.

---

### Task 5: Renderer bridge and PlayerPage attachment

**Files:**
- Modify: `apps/desktop/src/services/player.ts`
- Modify: `apps/desktop/src/pages/Player/PlayerPage.tsx`
- Modify: `apps/desktop/src/pages/Player/PlayerPage.test.tsx`

**Interfaces:**
- Consumes: `call` from `services/transport`, `isTauriHost` from `services/player`.
- Produces: `attachPlayerSurface(rect: {x,y,width,height}): Promise<void>`. `PlayerPage` calls it before `api.playbackStart(mediaId)` and on resize.

- [ ] **Step 1: Write a failing frontend test**

In `services/player.test.ts` (create it) — or add to the existing PlayerPage test if that is where mocks live:
```ts
import { describe, expect, it, vi } from "vitest";
import { attachPlayerSurface } from "./player";

describe("attachPlayerSurface", () => {
  it("sends only four numbers and no path or token", async () => {
    const invoke = vi.fn().mockResolvedValue(null);
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: { invoke },
    });
    await attachPlayerSurface({ x: 0, y: 72, width: 1280, height: 480 });
    expect(invoke).toHaveBeenCalledWith("player_attach_surface", {
      x: 0, y: 72, width: 1280, height: 480,
    });
    const serialised = JSON.stringify(invoke.mock.calls[0]);
    expect(serialised.toLowerCase()).not.toContain("path");
    expect(serialised.toLowerCase()).not.toContain("token");
  });
});
```

- [ ] **Step 2: Run it, expect failure**

Run: `cd apps/desktop && npx vitest run src/services/player.test.ts`
Expected: FAIL — `attachPlayerSurface` is not exported.

- [ ] **Step 3: Implement the bridge**

Append to `services/player.ts`:
```ts
export interface SurfaceRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Attach the native video surface. No-op outside the Tauri host. */
export function attachPlayerSurface(rect: SurfaceRect): Promise<void> {
  if (!isTauriHost()) return Promise.resolve();
  return call<void>("player_attach_surface", rect);
}
```

- [ ] **Step 4: Run it, expect PASS**

Run: `cd apps/desktop && npx vitest run src/services/player.test.ts`
Expected: PASS — 1 test.

- [ ] **Step 5: Attach before playback starts and on resize**

In `PlayerPage.tsx`:
- Import `attachPlayerSurface`, `isTauriHost` from `@/services/player`.
- Add refs next to `elapsedRef`:
```tsx
const topRef = useRef<HTMLDivElement>(null);
const controlsRef = useRef<HTMLDivElement>(null);
```
- Add a helper before `start`:
```tsx
const reportSurface = useCallback(async () => {
  if (!isTauriHost()) return;
  const top = topRef.current?.getBoundingClientRect();
  const controls = controlsRef.current?.getBoundingClientRect();
  if (!top || !controls) return;
  await attachPlayerSurface({
    x: 0,
    y: top.bottom,
    width: window.innerWidth,
    height: Math.max(1, controls.top - top.bottom),
  });
}, []);
```
- In `start`, immediately after `setLoadError(null)` and before `api.playbackStart(mediaId)`, add `await reportSurface();`.
- Add an effect after the `listenPlayerState` effect:
```tsx
useEffect(() => {
  if (!session || !isTauriHost()) return;
  const onChange = () => { void reportSurface().catch(() => {}); };
  window.addEventListener("resize", onChange);
  onChange();
  return () => window.removeEventListener("resize", onChange);
}, [session, reportSurface]);
```
- Attach `ref={topRef}` to the `<div className={`${styles.top} ...`}>` element and `ref={controlsRef}` to the `<div className={styles.controls...}>` element (the one carrying `styles.controls`).

- [ ] **Step 6: Add a regression test in `PlayerPage.test.tsx`**

```tsx
it("attaches the video surface before starting playback", async () => {
  const calls: string[] = [];
  vi.spyOn(playerApi, "attachPlayerSurface").mockImplementation(async () => {
    calls.push("attach");
  });
  vi.spyOn(api, "playbackStart").mockResolvedValue({ /* existing fixture shape */ } as never);
  render(<PlayerPage />);
  await waitFor(() => expect(calls).toContain("attach"));
  expect(calls.indexOf("attach")).toBeLessThan(calls.indexOf("start"));
});
```
Use the same mock patterns already present in `PlayerPage.test.tsx`; if `api.playbackStart` is mocked through a module mock, add `attachPlayerSurface` there instead. Do not invent a new fixture shape — reuse the existing one.

- [ ] **Step 7: Run the frontend gates**

Run (from `apps/desktop`): `npm run typecheck`, `npm run lint`, `npx vitest run`, `npm run check:tokens`, `npm run build`
Expected: all exit 0; token check reports 0 violations.

---

### Task 6: Full verification and blocking live gate

**Files:** none (verification only).

- [ ] **Step 1: Rust gates**

Run (from repo root, with `CARGO_TARGET_DIR=target/verify`):
`cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace`
Expected: all exit 0; 24-command parity test still passes.

- [ ] **Step 2: Frontend gates**

Run from `apps/desktop`: `npm run typecheck`, `npm run lint`, `npx vitest run`, `npm run check:tokens`, `npm run build`, `npm run check:bundle`
Expected: all exit 0; no dev token or signed client in the production bundle.

- [ ] **Step 3: Live gate (requires Task 1's mpv binary)**

Run the built app with `MEDIAHAVEN_MPV` pointing at the supplied binary and an authorised media file. Observe and record:
- No second taskbar entry / top-level mpv window.
- Top bar and bottom controls remain visible and clickable while video plays.
- Resizing the window repositions the video.
- Closing playback leaves no orphan mpv process.
Expected: all four observed. If any fails, STOP — do not report the feature complete; reopen the spec.

- [ ] **Step 4: Honest reporting**

Record PASS / FAIL / NOT RUN per gate in `progress.md`. Live playback is NOT RUN when mpv is absent. No checkpoint is passed by this work.

---

### Task 7: Documentation (uncommitted)

**Files:** Modify `roadmap.md`, `progress.md`.

- [ ] **Step 1: Record the change**

In `progress.md`, add a dated entry with: what changed, files changed, commands run, observed results, what was NOT RUN (live playback if mpv absent), risks, and the single next safe action. In `roadmap.md`, do not mark any checkpoint passed; note the in-window playback work under the appropriate phase with status `[~]` only if code exists.

- [ ] **Step 2: Check the diff**

Run: `git diff --check`
Expected: exit 0.

- [ ] **Step 3: Commit (BLOCKED)**

No commit is authorised. Leave all changes uncommitted and state that in the handover. Only commit if the human explicitly permits it in this session.

---

## Self-review

- **Spec coverage:** external mpv + `--wid` (Tasks 2/4), one window (Task 3/5), no bundling (Global Constraints), no new crate (Task 2 Step 1), Win32 surface + DPI (Tasks 2/3), named command + fail-closed (Task 4), no DTO/parity change (Task 4 Step 6), tests (Tasks 2–6), live spike gate (Tasks 1/6), docs (Task 7). Every spec section maps to a task.
- **Placeholder scan:** no TBD/TODO. The `/* existing fixture shape */` in Task 5 Step 6 points to a fixture already in `PlayerPage.test.tsx`; the executor must copy it, not invent data. Task 1 Step 1 is an explicit stop condition, not a placeholder.
- **Type consistency:** `SurfaceRect` (i32 fields) is produced by `validate_rect` and consumed by `to_physical`, `Surface::create` and `Surface::position`. `attach_surface` takes four `f64` plus `dpi: u32` and `parent: usize`; the command derives `dpi` from `scale_factor()` and `parent` from `window.hwnd()`. `surface_hwnd() -> Result<usize, PlayerError>` feeds `spawn_with_parent(_, Option<usize>)`. No renamed symbols between tasks.

**Execution note:** Task 1 is the blocking quality gate but is independent of Tasks 2–5's code, so those write surfaces can proceed in parallel lanes once the mpv binary question is settled; do not merge or report complete before Task 1 passes.
