//! Host-side playback state and ADR-0015 source resolution.
//!
//! The backend's host-internal `GET /playback_source/:media_id` returns a
//! *container-path identity*. It is resolved here, inside the host, through
//! A9's `RenderOutput::host_path_map()` and `media_playback::resolve_source`.
//! The resolved host file is handed only to mpv. Nothing in this module
//! serialises a path or a token into a renderer-facing DTO: `PlayerStateEvent`
//! carries display-only numbers and the media id the renderer already owns.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use media_playback::ipc::ConnectedIpc;
use media_playback::mpv::{MpvPlayer, Player, PlayerError as MpvError, ProcessChild, Status};
use media_playback::{resolve_source, SourceError};
use serde::Serialize;
use tauri::Emitter;

/// Renderer event carrying engine state only (no path, no token).
pub const PLAYER_STATE_EVENT: &str = "player://state";
const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Host-internal failures. Never serialised to the renderer.
#[derive(Debug, thiserror::Error)]
pub enum PlayerError {
    #[error("source unavailable: {0}")]
    Source(#[from] SourceError),
    #[error("mpv: {0}")]
    Mpv(#[from] MpvError),
    #[error("the player is not open")]
    Idle,
    #[error("{0} needs a numeric value")]
    NeedValue(&'static str),
    #[error("unknown player action: {0}")]
    UnknownAction(String),
    #[error("player lock poisoned")]
    Lock,
    #[error("player surface unavailable")]
    NoSurface,
    #[error("invalid surface rectangle")]
    InvalidSurface,
    #[error("video surface: {0}")]
    Surface(#[from] crate::win_surface::SurfaceError),
}

/// Resolve a container-path identity through A9's host map. Fails closed on a
/// stale path, an escape, or a missing file; no guess and no path back to the
/// renderer (ADR-0015).
pub fn resolve_for_media(
    host_map: &BTreeMap<String, PathBuf>,
    container_path: &str,
) -> Result<PathBuf, PlayerError> {
    resolve_source(container_path, host_map).map_err(PlayerError::from)
}

/// The display-only payload emitted on `player://state`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerStateEvent {
    pub time_pos: f64,
    pub pause: bool,
    pub volume: f64,
    pub media_id: String,
}

/// Build a renderer-safe event from the engine's status. Deliberately drops
/// everything the engine knows except three numbers and the media id.
pub fn status_event(status: &Status, media_id: &str) -> PlayerStateEvent {
    PlayerStateEvent {
        time_pos: status.time_pos,
        pause: status.pause,
        volume: status.volume,
        media_id: media_id.to_string(),
    }
}

/// Checkpoints arrive every few seconds; a larger forward jump is a seek and
/// must not be counted as watched time (B5 refuses seek-to-end completion).
const MAX_CHECKPOINT_STEP_SECS: f64 = 10.0;

/// Host-only progress tracking for B5 completion (review finding 3). Never
/// serialised: only aggregate numbers leave this struct, and the backend
/// re-checks them with the shared B5 policy before creating a cleanup entry.
#[derive(Default)]
struct Progress {
    media_id: Option<String>,
    duration: f64,
    last_position: f64,
    watched: f64,
    ended: bool,
}

/// The single mpv instance, owned by Tauri managed state. `Clone` shares the
/// same mutex so the status poller and commands see one player.
#[derive(Clone)]
pub struct PlayerState {
    inner: Arc<Mutex<Option<MpvPlayer<ConnectedIpc, ProcessChild>>>>,
    mpv_exe: Arc<PathBuf>,
    progress: Arc<Mutex<Progress>>,
    // Created/positioned/hidden only from sync commands on the main thread:
    // Win32 windows have thread affinity and Drop destroys the HWND.
    surface: Arc<Mutex<Option<crate::win_surface::Surface>>>,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayerState {
    /// mpv comes from `MEDIAHAVEN_MPV` when set, else `mpv` on PATH. No install
    /// is performed by the app.
    pub fn new() -> Self {
        let mpv_exe = std::env::var_os("MEDIAHAVEN_MPV")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("mpv"));
        Self {
            inner: Arc::new(Mutex::new(None)),
            mpv_exe: Arc::new(mpv_exe),
            progress: Arc::new(Mutex::new(Progress::default())),
            surface: Arc::new(Mutex::new(None)),
        }
    }

    /// Start tracking a newly opened media id.
    pub fn begin(&self, media_id: &str) {
        if let Ok(mut progress) = self.progress.lock() {
            *progress = Progress {
                media_id: Some(media_id.to_string()),
                ..Progress::default()
            };
        }
    }

    /// Forget progress when playback stops (a user stop never confirms B5).
    pub fn clear_progress(&self) {
        if let Ok(mut progress) = self.progress.lock() {
            *progress = Progress::default();
        }
    }

    /// Accumulate watched seconds from checkpoints. Only forward movement
    /// counts, so a seek cannot inflate `watched`. The normal-end flag also
    /// requires the 90% B5 threshold, so a seek-to-end never confirms.
    pub fn observe_checkpoint(&self, media_id: &str, position: f64, duration: f64) {
        if !position.is_finite() || !duration.is_finite() || position < 0.0 || duration <= 0.0 {
            return;
        }
        if let Ok(mut progress) = self.progress.lock() {
            if progress.media_id.as_deref() != Some(media_id) {
                *progress = Progress {
                    media_id: Some(media_id.to_string()),
                    duration,
                    last_position: position,
                    ..Progress::default()
                };
                return;
            }
            progress.duration = duration;
            if position > progress.last_position {
                // Cap the increment: a big jump is a seek, not watched time.
                progress.watched +=
                    (position - progress.last_position).min(MAX_CHECKPOINT_STEP_SECS);
            }
            progress.last_position = position;
            if position >= duration - 30.0 && progress.watched >= duration * 0.9 {
                progress.ended = true;
            }
        }
    }

    /// Take a pending completion signal once, or `None`. The backend re-checks
    /// it with B5; the host never decides completion on its own.
    pub fn take_completion(&self) -> Option<(String, f64, f64, f64)> {
        let Ok(mut progress) = self.progress.lock() else {
            return None;
        };
        if !progress.ended {
            return None;
        }
        progress.ended = false;
        let media_id = progress.media_id.clone()?;
        Some((
            media_id,
            progress.watched,
            progress.duration,
            progress.last_position,
        ))
    }

    /// Create or reposition the child video surface inside the host window.
    /// Called only from the sync `player_attach_surface` command (main thread).
    pub fn attach_surface(
        &self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        dpi: u32,
        parent: usize,
    ) -> Result<(), PlayerError> {
        let rect = crate::win_surface::validate_rect(x, y, width, height)
            .ok_or(PlayerError::InvalidSurface)?;
        let mut guard = self.surface.lock().map_err(|_| PlayerError::Lock)?;
        match guard.as_mut() {
            Some(surface) => surface.position(rect, dpi),
            None => {
                *guard = Some(crate::win_surface::Surface::create(parent, rect, dpi)?);
            }
        }
        if let Some(surface) = guard.as_mut() {
            surface.show(true);
        }
        Ok(())
    }

    /// HWND of the attached surface. Without one, playback fails closed rather
    /// than letting mpv create its own top-level window.
    pub fn surface_hwnd(&self) -> Result<usize, PlayerError> {
        let guard = self.surface.lock().map_err(|_| PlayerError::Lock)?;
        guard
            .as_ref()
            .map(|surface| surface.hwnd())
            .ok_or(PlayerError::NoSurface)
    }

    /// Re-show an attached surface that a previous `stop()` hid. No-op when no
    /// surface is stored; the fail-closed check lives in `surface_hwnd`.
    pub fn show_surface(&self) {
        if let Ok(guard) = self.surface.lock() {
            if let Some(surface) = guard.as_ref() {
                surface.show(true);
            }
        }
    }

    /// Spawn mpv if idle and load a host-resolved file at the resume position.
    /// `host_path` is host-internal and never appears in a DTO.
    pub fn open_media(&self, host_path: &Path, start_secs: f64) -> Result<(), PlayerError> {
        let parent = self.surface_hwnd()?;
        self.show_surface();
        let mut guard = self.inner.lock().map_err(|_| PlayerError::Lock)?;
        if guard.is_none() {
            *guard = Some(MpvPlayer::spawn_with_parent(&self.mpv_exe, Some(parent))?);
        }
        guard
            .as_mut()
            .ok_or(PlayerError::Idle)?
            .open(host_path, start_secs)?;
        Ok(())
    }

    /// Named controls only. No generic engine-passthrough command exists.
    pub fn control(&self, action: &str, value: Option<f64>) -> Result<(), PlayerError> {
        // `stop` tears down the player and hides the surface; it must be safe
        // and idempotent while idle, before the `Idle` check below.
        if action == "stop" {
            self.stop();
            return Ok(());
        }
        let mut guard = self.inner.lock().map_err(|_| PlayerError::Lock)?;
        let player = guard.as_mut().ok_or(PlayerError::Idle)?;
        match action {
            "play" => player.play()?,
            "pause" => player.pause()?,
            "seek" => player.seek(value.ok_or(PlayerError::NeedValue("seek"))?)?,
            "volume" => player.set_volume(value.ok_or(PlayerError::NeedValue("volume"))?)?,
            other => return Err(PlayerError::UnknownAction(other.to_string())),
        }
        Ok(())
    }

    /// Stop playback and drop the player. `MpvPlayer::drop` kills the child, so
    /// app exit / window close never leaves an orphan.
    pub fn stop(&self) {
        if let Ok(mut guard) = self.inner.lock() {
            if let Some(player) = guard.as_mut() {
                let _ = player.stop();
            }
            *guard = None;
        }
        // Hide, never drop: re-attach reuses the same HWND. Main thread only.
        if let Ok(guard) = self.surface.lock() {
            if let Some(surface) = guard.as_ref() {
                surface.show(false);
            }
        }
    }

    fn status(&self) -> Result<Option<Status>, PlayerError> {
        let mut guard = self.inner.lock().map_err(|_| PlayerError::Lock)?;
        match guard.as_mut() {
            Some(player) => player.status().map(Some).map_err(PlayerError::from),
            None => Ok(None),
        }
    }
}

/// Emit `player://state` every 500 ms until the player is stopped, the IPC
/// fails or the WebView goes away. Every payload is display-only.
pub fn spawn_status_poller(state: PlayerState, app: tauri::AppHandle, media_id: String) {
    std::thread::spawn(move || loop {
        std::thread::sleep(POLL_INTERVAL);
        let status = match state.status() {
            Ok(Some(status)) => status,
            Ok(None) => break,
            Err(_) => break,
        };
        if app
            .emit(PLAYER_STATE_EVENT, status_event(&status, &media_id))
            .is_err()
        {
            break;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQ: AtomicU64 = AtomicU64::new(0);

    fn temp_root(label: &str) -> PathBuf {
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("mediahaven-player-{label}-{n}"));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create temp root");
        path
    }

    #[test]
    fn host_helper_resolves_declared_path_and_fails_closed_on_stale() {
        let base = temp_root("resolve");
        let root = base.join("movies");
        std::fs::create_dir_all(root.join("sub")).unwrap();
        let file = root.join("sub").join("hello.mkv");
        std::fs::write(&file, b"fake media").unwrap();
        let map = BTreeMap::from([("/media/root-0".to_string(), root.clone())]);

        let resolved = resolve_for_media(&map, "/media/root-0/sub/hello.mkv")
            .expect("a real file under a declared root resolves");
        assert_eq!(resolved, std::fs::canonicalize(&file).unwrap());

        // A moved/deleted file or an unmapped container root never resolves.
        assert!(resolve_for_media(&map, "/media/root-0/gone.mkv").is_err());
        assert!(resolve_for_media(&map, "/media/root-9/hello.mkv").is_err());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn completion_signal_needs_watched_progress_not_a_seek_to_end() {
        let state = PlayerState::new();
        state.begin("m-cleanup");
        // A jump straight to the end with no watched time must not confirm.
        state.observe_checkpoint("m-cleanup", 3595.0, 3600.0);
        assert!(state.take_completion().is_none());

        // Incremental watching (10 s checkpoint spacing) to near the end
        // produces one B5 signal; large jumps above were refused as seeks.
        state.clear_progress();
        state.begin("m-cleanup");
        let mut position: f64 = 0.0;
        while position < 3580.0 {
            position = (position + 10.0).min(3580.0);
            state.observe_checkpoint("m-cleanup", position, 3600.0);
        }
        let (media_id, watched, duration, position) = state
            .take_completion()
            .expect("watched completion confirmed");
        assert_eq!(media_id, "m-cleanup");
        assert!(
            watched >= duration * 0.9,
            "watched {watched}s of {duration}s"
        );
        assert!(position >= duration - 30.0);
        assert!(state.take_completion().is_none(), "signal fires only once");

        state.clear_progress();
        state.observe_checkpoint("m-cleanup", 3580.0, 3600.0);
        assert!(state.take_completion().is_none());
    }

    #[test]
    fn no_surface_fails_closed_instead_of_opening_a_window() {
        let state = PlayerState::new();
        let err = state.surface_hwnd().expect_err("no attached surface");
        assert!(matches!(err, PlayerError::NoSurface));
    }

    #[test]
    fn stop_action_is_idempotent_and_safe_when_idle() {
        let state = PlayerState::new();
        // No mpv spawned: `stop` must not surface PlayerError::Idle.
        state.control("stop", None).expect("stop is safe when idle");
        // A repeated stop (e.g. from a second unmount) is also a no-op.
        state.control("stop", None).expect("stop is idempotent");
    }

    #[test]
    fn open_media_reveals_the_surface_and_stays_fail_closed() {
        let state = PlayerState::new();
        // No fake Surface exists in unit tests (one needs a real Win32 HWND),
        // so assert the replay helper is callable and safe with no surface.
        state.show_surface();
        let err = state
            .open_media(Path::new("unused.mkv"), 0.0)
            .expect_err("no surface means no mpv spawn");
        assert!(matches!(err, PlayerError::NoSurface));
    }

    #[test]
    fn status_event_has_no_path_or_token_keys() {
        let status = Status {
            time_pos: 42.5,
            pause: false,
            volume: 80.0,
        };
        let value = serde_json::to_value(status_event(&status, "m1")).expect("serialise event");
        let object = value.as_object().expect("event is an object");
        assert_eq!(object.len(), 4);
        for key in ["timePos", "pause", "volume", "mediaId"] {
            assert!(object.contains_key(key), "missing display-only key: {key}");
        }
        let raw = value.to_string().to_ascii_lowercase();
        assert!(!raw.contains("path"));
        assert!(!raw.contains("token"));
    }
}
