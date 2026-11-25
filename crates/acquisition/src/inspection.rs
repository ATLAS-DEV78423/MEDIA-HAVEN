//! Parse-inspection gate for downloaded media (security hardening, Task 1).
//!
//! A downloaded file is hostile input. Only `ffprobe`, running inside the
//! networkless shell-less `probe` container, is allowed to open its bytes — this
//! module NEVER opens the file on the Windows host. The probe either parses the
//! file as a media container (`Clean`) or it does not; a missing/unresponsive
//! probe is `Unavailable`, never `Clean` (fail closed).
//!
//! The command is built as an argument vector only: no shell, no string
//! interpolation, and the container path is constrained to the `/downloads`
//! mount so a filename can never become an ffprobe option.

use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Image built from `infrastructure/containers/Dockerfile.probe` (no shell,
/// no token, no network). Verified with `docker image inspect`.
pub const PROBE_IMAGE: &str = "mediahaven-probe:local";
pub const FFPROBE_PATH: &str = "/usr/local/bin/ffprobe";
/// The only container path prefix inspection will accept.
pub const DOWNLOADS_PREFIX: &str = "/downloads/";
/// Container mount point shared with qB (rw) and `core` (ro) in compose.
pub const DOWNLOADS_MOUNT: &str = "/downloads";
/// Same env var the desktop supervisor and compose use for the host directory.
pub const DOWNLOAD_ROOT_ENV: &str = "MEDIAHAVEN_DOWNLOAD_ROOT";
const PROBE_TIMEOUT: Duration = Duration::from_secs(30);
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Result of inspecting one downloaded file. Stored on `download_items` as a
/// display-only string; no path is ever carried in this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectionState {
    Pending,
    Scanning,
    Clean,
    /// The probe ran but the file did not parse as media — do not play.
    Suspicious,
    /// The probe could not run (missing binary, spawn error, timeout) — do not
    /// play. Failure is never mistaken for a clean result.
    Unavailable,
}

impl InspectionState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Scanning => "scanning",
            Self::Clean => "clean",
            Self::Suspicious => "suspicious",
            Self::Unavailable => "unavailable",
        }
    }
}

/// What the runner observed. `success: false` is a completed probe that
/// rejected the file; it is distinct from `Err`, which means the probe itself
/// could not run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeOutput {
    pub success: bool,
    pub stdout: String,
}

/// The probe did not run to completion. This always maps to `Unavailable`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeError {
    Unavailable,
}

/// Injectable boundary so tests drive inspect logic without Docker or a lawfully
/// sourced media file. The production implementation is [`DockerProbeRunner`].
pub trait ProbeRunner {
    fn run(&self, container_path: &str) -> Result<ProbeOutput, ProbeError>;
}

/// Resolve the host acquisition directory from the operator-supplied env var.
/// A missing/blank/control-character value fails closed rather than producing
/// a bind to the wrong directory — the probe must see the same bytes qB wrote.
pub fn download_root_from_env() -> Result<String, ProbeError> {
    let value = std::env::var(DOWNLOAD_ROOT_ENV).map_err(|_| ProbeError::Unavailable)?;
    let root = value.trim();
    if root.is_empty() || root.chars().any(char::is_control) {
        return Err(ProbeError::Unavailable);
    }
    Ok(root.to_string())
}

/// The exact argv passed to `docker`. The host root is bound read-only at
/// `/downloads` — without it a fresh probe container cannot see the file qB
/// wrote. Exposed so tests can assert the bind and the absence of a shell
/// without invoking the engine. The host path lives only here, inside a process
/// argument vector; it is never a renderer DTO field.
pub fn probe_args(container_path: &str, host_root: &str) -> Vec<String> {
    vec![
        "run".to_string(),
        "--rm".to_string(),
        "--network=none".to_string(),
        "--entrypoint".to_string(),
        FFPROBE_PATH.to_string(),
        "-v".to_string(),
        format!("{host_root}:{DOWNLOADS_MOUNT}:ro"),
        PROBE_IMAGE.to_string(),
        "-v".to_string(),
        "error".to_string(),
        "-show_format".to_string(),
        "-of".to_string(),
        "json".to_string(),
        container_path.to_string(),
    ]
}

/// Argument-vectors-only invocation of the probe with a bounded timeout.
pub struct DockerProbeRunner;

impl ProbeRunner for DockerProbeRunner {
    fn run(&self, container_path: &str) -> Result<ProbeOutput, ProbeError> {
        let host_root = download_root_from_env()?;
        let mut child = Command::new("docker")
            .args(probe_args(container_path, &host_root))
            .stdout(Stdio::piped())
            // ffprobe at `-v error` writes nothing useful to stderr; a null sink
            // cannot fill up and block the child while we poll for completion.
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| ProbeError::Unavailable)?;

        let deadline = Instant::now() + PROBE_TIMEOUT;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let mut stdout = String::new();
                    if let Some(mut pipe) = child.stdout.take() {
                        // Output is a small JSON object; a read failure leaves an
                        // empty string, which classifies as Suspicious (fail closed).
                        let _ = pipe.read_to_string(&mut stdout);
                    }
                    return Ok(ProbeOutput {
                        success: status.success(),
                        stdout,
                    });
                }
                Ok(None) if Instant::now() < deadline => thread::sleep(POLL_INTERVAL),
                _ => {
                    // Timeout or wait error: kill the child and fail closed.
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(ProbeError::Unavailable);
                }
            }
        }
    }
}

/// Testable core. Anything other than a clean parse is refused.
pub fn inspect_with_runner(runner: &dyn ProbeRunner, container_path: &str) -> InspectionState {
    if !is_downloads_path(container_path) {
        return InspectionState::Suspicious;
    }
    let output = match runner.run(container_path) {
        Ok(output) => output,
        Err(ProbeError::Unavailable) => return InspectionState::Unavailable,
    };
    if !output.success {
        return InspectionState::Suspicious;
    }
    let value: serde_json::Value = match serde_json::from_str(output.stdout.trim()) {
        Ok(value) => value,
        Err(_) => return InspectionState::Suspicious,
    };
    if value.get("format").and_then(|f| f.as_object()).is_some() {
        InspectionState::Clean
    } else {
        InspectionState::Suspicious
    }
}

/// Production entry point. The file is never read on the host.
pub fn inspect_with_probe(container_path: &str) -> InspectionState {
    inspect_with_runner(&DockerProbeRunner, container_path)
}

/// Completion-path caller: inspect every container path of a finished download.
/// Returns one `(path, state)` pair per file; an empty list is an empty result
/// and `combine_inspection` turns it into `Unavailable` (never a false Clean).
pub fn inspect_completed(
    runner: &dyn ProbeRunner,
    container_paths: &[String],
) -> Vec<(String, InspectionState)> {
    container_paths
        .iter()
        .map(|path| (path.clone(), inspect_with_runner(runner, path)))
        .collect()
}

/// Fold per-file results into the download's state. Any unavailable or missing
/// result dominates; a suspicious file dominates pending/scanning; only an
/// all-clean set is playable. Fail closed on an empty or unknown set.
pub fn combine_inspection(results: &[(String, InspectionState)]) -> InspectionState {
    if results.is_empty() {
        return InspectionState::Unavailable;
    }
    let states: Vec<InspectionState> = results.iter().map(|(_, state)| *state).collect();
    if states.contains(&InspectionState::Unavailable) {
        return InspectionState::Unavailable;
    }
    if states.contains(&InspectionState::Suspicious) {
        return InspectionState::Suspicious;
    }
    if states.iter().all(|state| *state == InspectionState::Clean) {
        return InspectionState::Clean;
    }
    InspectionState::Unavailable
}

/// A container path is accepted only if it is a concrete file under the
/// `/downloads` mount, with no traversal, backslash or control character. This
/// also stops a hostile filename from being interpreted as an ffprobe option.
fn is_downloads_path(path: &str) -> bool {
    path.starts_with(DOWNLOADS_PREFIX)
        && path.len() > DOWNLOADS_PREFIX.len()
        && !path.ends_with('/')
        && !path.contains("..")
        && !path.contains('\\')
        && !path.chars().any(char::is_control)
}
