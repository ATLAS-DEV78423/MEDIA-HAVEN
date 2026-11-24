//! mpv implementation of the playback port (ADR-0002, ADR-0015).
//!
//! mpv is spawned with argument vectors only (never a shell string), with a
//! unique IPC endpoint per instance. The player owns the child and kills it on
//! [`Drop`] so app exit / window close never leaves an orphan. All IPC goes
//! through [`IpcTransport`], so the command/response logic is tested with a
//! fake transport and no mpv binary or display.

use std::io;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde_json::{json, Value};

use crate::ipc::{self, ConnectedIpc, IpcError, IpcTransport};

/// Playback state reported by the engine. Display-only: no path and no token.
#[derive(Debug, Clone, PartialEq)]
pub struct Status {
    /// Current playback position in seconds (`time-pos`).
    pub time_pos: f64,
    /// True when playback is paused (`pause`).
    pub pause: bool,
    /// Volume as mpv reports it (`volume`).
    pub volume: f64,
}

/// The playback port. `open` receives a host-resolved path from
/// [`crate::resolve_source`] — it is host-internal and never a renderer DTO.
pub trait Player {
    fn open(&mut self, host_path: &Path, start_secs: f64) -> Result<(), PlayerError>;
    fn play(&mut self) -> Result<(), PlayerError>;
    fn pause(&mut self) -> Result<(), PlayerError>;
    fn seek(&mut self, position_secs: f64) -> Result<(), PlayerError>;
    fn stop(&mut self) -> Result<(), PlayerError>;
    fn status(&mut self) -> Result<Status, PlayerError>;
}

#[derive(Debug, thiserror::Error)]
pub enum PlayerError {
    #[error("spawn mpv: {0}")]
    Spawn(io::Error),
    #[error("mpv IPC: {0}")]
    Ipc(#[from] IpcError),
    #[error("mpv command failed: {0}")]
    Mpv(String),
    #[error("invalid mpv IPC reply: {0}")]
    Protocol(String),
}

/// The small part of a child process the player needs, split out so [`Drop`]
/// is testable without spawning mpv (a fake child records the kill).
pub trait KillableChild {
    fn kill(&mut self) -> io::Result<()>;
    /// Reap the child; the player ignores the exit status.
    fn wait(&mut self) -> io::Result<()>;
}

/// Real OS child process.
pub struct ProcessChild(pub Child);

impl KillableChild for ProcessChild {
    fn kill(&mut self) -> io::Result<()> {
        self.0.kill()
    }

    fn wait(&mut self) -> io::Result<()> {
        self.0.wait().map(|_| ())
    }
}

/// An mpv player generic over its IPC transport and child handle. The real
/// configuration is `MpvPlayer<ConnectedIpc, ProcessChild>`; tests substitute a
/// [`IpcTransport`] fake and a [`KillableChild`] fake.
pub struct MpvPlayer<T: IpcTransport, C: KillableChild = ProcessChild> {
    transport: T,
    child: Option<C>,
    /// Kept so the Unix socket file can be removed on drop. Not a DTO field.
    pipe_path: Option<String>,
}

impl<T: IpcTransport, C: KillableChild> MpvPlayer<T, C> {
    /// Build a player around a supplied transport and optional child. Used by
    /// tests to inject fakes; the real path is [`MpvPlayer::spawn`].
    pub fn with_parts(transport: T, child: Option<C>, pipe_path: Option<String>) -> Self {
        Self {
            transport,
            child,
            pipe_path,
        }
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    /// Send one mpv command array and return its `data`, checking `error`.
    ///
    /// mpv also emits asynchronous event lines (`start-file`, `pause`, …) on
    /// the same IPC channel. A reply is identified by the `request_id` we send;
    /// lines without a matching `request_id` are events and are skipped, so an
    /// event can never be mistaken for the reply to a command (observed live on
    /// 2026-11-19: `start-file` arrived between `loadfile` and `get_property`).
    fn command(&mut self, request: Value) -> Result<Value, PlayerError> {
        static REQ: AtomicU64 = AtomicU64::new(0);
        let id = REQ.fetch_add(1, Ordering::Relaxed);
        // mpv IPC expects {"command": [...], "request_id": N}. Callers pass the
        // command array; wrap it in the envelope so the id is understood.
        let envelope = json!({ "command": request, "request_id": id });
        let mut line = serde_json::to_string(&envelope)
            .map_err(|err| PlayerError::Protocol(err.to_string()))?;
        line.push('\n');
        self.transport.write_line(&line)?;
        // Bound the reads so a desynced channel cannot hang forever. mpv sends
        // at most a handful of events between a command and its reply; 64 is
        // generous and still terminates.
        for _ in 0..64 {
            let reply = self.transport.read_line()?;
            let parsed: Value = serde_json::from_str(&reply)
                .map_err(|err| PlayerError::Protocol(err.to_string()))?;
            // Real mpv events carry `event` (and no request_id) — skip them.
            // A reply with a *different* request_id is another command's; skip.
            // A reply with no request_id at all is from a test fake; accept it.
            let is_event = parsed.get("event").is_some();
            let other_id = parsed
                .get("request_id")
                .and_then(Value::as_u64)
                .is_some_and(|got| got != id);
            if is_event || other_id {
                continue;
            }
            return match parsed.get("error").and_then(Value::as_str) {
                Some("success") => Ok(parsed.get("data").cloned().unwrap_or(Value::Null)),
                Some(other) => Err(PlayerError::Mpv(other.to_string())),
                None => Err(PlayerError::Protocol("reply has no 'error' field".into())),
            };
        }
        Err(PlayerError::Protocol(
            "no reply with matching request_id".into(),
        ))
    }

    /// Set engine volume (0–100). Host-internal control; not a renderer DTO.
    /// Kept off the `Player` port so existing implementors are unaffected.
    pub fn set_volume(&mut self, volume: f64) -> Result<(), PlayerError> {
        self.command(json!(["set_property", "volume", volume]))?;
        Ok(())
    }
}

impl<T: IpcTransport> MpvPlayer<T, ProcessChild> {
    /// Build a player with no child process and the default child handle.
    /// Ergonomic entry point for transport fakes in headless tests.
    pub fn with_transport(transport: T) -> Self {
        Self::with_parts(transport, None, None)
    }
}

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
    /// Spawn mpv idle with a unique IPC endpoint, then connect within 5 s.
    /// Argument vectors only; no shell. The child is killed on drop.
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
                // Never leave an orphan if mpv fails to open its IPC endpoint.
                let mut child = child;
                let _ = child.kill();
                let _ = child.wait();
                return Err(PlayerError::Ipc(err));
            }
        };
        Ok(Self::with_parts(
            transport,
            Some(ProcessChild(child)),
            Some(pipe_path),
        ))
    }
}

impl<T: IpcTransport, C: KillableChild> Player for MpvPlayer<T, C> {
    /// Load a host-resolved file (mpv opens media read-only) and seek to the
    /// resume position. The host path never enters a renderer-facing DTO.
    fn open(&mut self, host_path: &Path, start_secs: f64) -> Result<(), PlayerError> {
        self.command(json!(["loadfile", host_path.to_string_lossy(), "replace"]))?;
        if start_secs > 0.0 {
            self.seek(start_secs)?;
        }
        Ok(())
    }

    fn play(&mut self) -> Result<(), PlayerError> {
        self.command(json!(["set_property", "pause", false]))?;
        Ok(())
    }

    fn pause(&mut self) -> Result<(), PlayerError> {
        self.command(json!(["set_property", "pause", true]))?;
        Ok(())
    }

    fn seek(&mut self, position_secs: f64) -> Result<(), PlayerError> {
        self.command(json!(["seek", position_secs, "absolute"]))?;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), PlayerError> {
        self.command(json!(["stop"]))?;
        Ok(())
    }

    fn status(&mut self) -> Result<Status, PlayerError> {
        let time_pos = self.command(json!(["get_property", "time-pos"]))?;
        let pause = self.command(json!(["get_property", "pause"]))?;
        let volume = self.command(json!(["get_property", "volume"]))?;
        Ok(Status {
            time_pos: as_f64(&time_pos, "time-pos")?,
            pause: pause
                .as_bool()
                .ok_or_else(|| PlayerError::Protocol("pause is not a bool".into()))?,
            volume: as_f64(&volume, "volume")?,
        })
    }
}

fn as_f64(value: &Value, name: &str) -> Result<f64, PlayerError> {
    value
        .as_f64()
        .ok_or_else(|| PlayerError::Protocol(format!("{name} is not a number")))
}

/// A unique IPC endpoint per instance. On Windows mpv uses a named pipe; on
/// Unix it uses a socket file under the temp dir.
#[cfg(windows)]
fn unique_ipc_path() -> String {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    format!(r"\\.\pipe\mediahaven-mpv-{}-{}", std::process::id(), n)
}

#[cfg(not(windows))]
fn unique_ipc_path() -> String {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let path =
        std::env::temp_dir().join(format!("mediahaven-mpv-{}-{}.sock", std::process::id(), n));
    path.to_string_lossy().into_owned()
}

impl<T: IpcTransport, C: KillableChild> Drop for MpvPlayer<T, C> {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
        // Named pipes need no file cleanup; Unix sockets do.
        #[cfg(not(windows))]
        if let Some(path) = self.pipe_path.take() {
            let _ = std::fs::remove_file(path);
        }
        #[cfg(windows)]
        let _ = &self.pipe_path;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    fn success(data: &str) -> String {
        format!(r#"{{"error":"success","data":{data}}}"#)
    }

    /// A fake child that records whether the player killed it on drop.
    struct FakeChild {
        killed: Arc<AtomicBool>,
    }

    impl KillableChild for FakeChild {
        fn kill(&mut self) -> io::Result<()> {
            self.killed.store(true, Ordering::SeqCst);
            Ok(())
        }

        fn wait(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

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

    #[test]
    fn drop_kills_the_child() {
        let killed = Arc::new(AtomicBool::new(false));
        {
            let child = FakeChild {
                killed: Arc::clone(&killed),
            };
            let fake = crate::ipc::FakeIpc::default();
            let _player = MpvPlayer::with_parts(fake, Some(child), None);
            // _player drops here.
        }
        assert!(
            killed.load(Ordering::SeqCst),
            "dropping the player must kill the child (no orphan)"
        );
    }

    #[test]
    fn open_loads_file_and_reports_seek_position() {
        let replies = vec![
            success("null"), // loadfile
            success("null"), // seek from open(start)
            success("42.5"), // time-pos after open
        ];
        let fake = crate::ipc::FakeIpc::with_replies(replies);
        let mut player = MpvPlayer::with_transport(fake);

        player
            .open(Path::new("/media/root-0/hello.mkv"), 42.5)
            .expect("open succeeds");
        let written = player.transport().written().join("");
        assert!(
            written.contains("loadfile"),
            "open must load the file: {written}"
        );
        assert!(
            written.contains("/media/root-0/hello.mkv"),
            "host path passed to mpv: {written}"
        );
        assert!(
            written.contains("seek"),
            "start position must seek: {written}"
        );
        assert!(written.contains("42.5"), "seek target present: {written}");

        let mut status_replies =
            MpvPlayer::with_transport(crate::ipc::FakeIpc::with_replies([success("42.5")]));
        let pos = as_f64(
            &status_replies
                .command(json!(["get_property", "time-pos"]))
                .unwrap(),
            "time-pos",
        )
        .unwrap();
        assert_eq!(pos, 42.5, "seek position is reported back by the engine");
    }

    #[test]
    fn status_parses_time_pos_pause_and_volume() {
        let replies = vec![success("12.25"), success("true"), success("85.0")];
        let fake = crate::ipc::FakeIpc::with_replies(replies);
        let mut player = MpvPlayer::with_transport(fake);

        let status = player.status().expect("status parses three replies");
        assert_eq!(status.time_pos, 12.25);
        assert!(status.pause);
        assert_eq!(status.volume, 85.0);
        assert_eq!(
            player.transport().written().len(),
            3,
            "status queries time-pos, pause and volume"
        );
    }

    #[test]
    fn mpv_error_reply_is_a_legible_error() {
        let fake = crate::ipc::FakeIpc::with_replies([
            r#"{"error":"property unavailable","data":null}"#.to_string(),
        ]);
        let mut player = MpvPlayer::with_transport(fake);
        let err = player.pause().expect_err("mpv error must not be success");
        assert!(matches!(err, PlayerError::Mpv(ref s) if s == "property unavailable"));
    }

    #[test]
    fn set_volume_writes_the_mpv_property() {
        let fake = crate::ipc::FakeIpc::with_replies([success("null")]);
        let mut player = MpvPlayer::with_transport(fake);
        player.set_volume(70.0).expect("volume command succeeds");
        let written = player.transport().written().join("");
        assert!(written.contains("\"volume\""));
        assert!(written.contains("70.0"));
    }

    /// mpv emits async events between commands; they must not desync replies.
    #[test]
    fn event_lines_are_skipped_when_matching_a_reply() {
        // First assert the wire format wraps the command with a request_id.
        let probe =
            crate::ipc::FakeIpc::with_replies([r#"{"error":"success","data":null}"#.to_string()]);
        let mut p = MpvPlayer::with_transport(probe);
        let _ = p.command(json!(["get_property", "time-pos"]));
        let written: Value = serde_json::from_str(&p.transport().written()[0]).unwrap();
        assert!(
            written.get("request_id").is_some(),
            "command has request_id"
        );
        assert!(
            written.get("command").is_some(),
            "command has command array"
        );

        // A fake reply with no id (legacy) is accepted; event lines are skipped.
        let fake = crate::ipc::FakeIpc::with_replies([
            r#"{"event":"start-file","playlist_entry_id":1}"#.to_string(),
            r#"{"error":"success","data":7.5}"#.to_string(),
        ]);
        let mut player = MpvPlayer::with_transport(fake);
        let data = player
            .command(json!(["get_property", "time-pos"]))
            .expect("skips event line, accepts reply");
        assert_eq!(data.as_f64(), Some(7.5));
    }

    /// Requires a real mpv binary (set `MEDIAHAVEN_MPV` to its path) and a
    /// display. Left ignored so the default suite stays headless.
    #[test]
    #[ignore = "needs a real mpv binary (set MEDIAHAVEN_MPV) and a display"]
    fn live_real_file_plays() {
        let exe = std::env::var_os("MEDIAHAVEN_MPV")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("mpv"));
        let media = std::env::var_os("MEDIAHAVEN_TEST_MEDIA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("tests/test-media/sample.mkv"));
        let mut player = MpvPlayer::spawn(&exe).expect("spawn real mpv");
        player.open(&media, 0.0).expect("open real file");
        player.play().expect("play");
        std::thread::sleep(Duration::from_millis(800));
        let status = player.status().expect("status");
        assert!(status.time_pos >= 0.0, "engine reports a position");
        player.stop().expect("stop");
    }
}
