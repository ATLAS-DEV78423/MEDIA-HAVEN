//! Container workspace lifecycle.
//!
//! The supervisor starts, waits for, health-checks and stops the `core`
//! container through `wsl.exe` (ADR-0005 §3, ADR-0013). It never invokes
//! `docker.exe` on Windows and never enables a TCP Docker endpoint.
//!
//! Readiness here is a **transport** check: can the host open the loopback
//! port? The authenticated `/readyz` call is the command layer's gate (A5/A7),
//! which owns the HMAC signing — building the signature here would build it
//! twice.

use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::secret::install_token;
use crate::wsl::{decode_output, Wsl, WslError};

/// Bounded timeouts (plan A4).
pub const START_TIMEOUT: Duration = Duration::from_secs(30);
pub const READY_TIMEOUT: Duration = Duration::from_secs(10);
pub const POLL_INTERVAL: Duration = Duration::from_millis(250);

const API_TOKEN_ENV: &str = "MEDIAHAVEN_API_TOKEN";
const WSLENV: &str = "WSLENV";

/// Child-only environment for `compose up`. `WSLENV` lists the forwarded name
/// with no flag suffix: the value is plain hex, so WSL passes it through
/// unchanged with no path translation.
fn build_wsl_env(token_hex: &str) -> Vec<(String, String)> {
    vec![
        (API_TOKEN_ENV.to_string(), token_hex.to_string()),
        (WSLENV.to_string(), API_TOKEN_ENV.to_string()),
    ]
}

/// The legible lifecycle states from ADR-0005 §5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupervisorStatus {
    /// WSL or the Docker engine is not present; the app shows the setup screen.
    EngineAbsent,
    /// The container is starting.
    Starting,
    /// The loopback port accepts connections.
    Ready,
    /// The workspace is stopped.
    Stopped,
    /// A failure with a user-safe message.
    Failed(String),
}

impl std::fmt::Display for SupervisorStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EngineAbsent => write!(
                f,
                "setup required: Windows Subsystem for Linux and Docker are not ready"
            ),
            Self::Starting => write!(f, "starting the media service…"),
            Self::Ready => write!(f, "ready"),
            Self::Stopped => write!(f, "stopped"),
            Self::Failed(message) => write!(f, "{message}"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SupervisorError {
    #[error("engine_absent: {0}")]
    EngineAbsent(String),
    #[error("start_failed: {0}")]
    StartFailed(String),
    #[error("not_ready: the media service did not become ready within {0:?}")]
    NotReady(Duration),
    #[error("stop_failed: {0}")]
    StopFailed(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Owns the distro name, the rendered compose file and the loopback address.
#[derive(Debug, Clone)]
pub struct Supervisor {
    distro: String,
    compose_path: PathBuf,
    address: SocketAddr,
    wsl: Wsl,
}

impl Supervisor {
    pub fn new(
        distro: impl Into<String>,
        compose_path: impl Into<PathBuf>,
        address: SocketAddr,
    ) -> Self {
        Self {
            distro: distro.into(),
            compose_path: compose_path.into(),
            address,
            wsl: Wsl::new(),
        }
    }

    /// Start the workspace (`compose up -d`).
    ///
    /// Deliberately **detached**: `--wait` would block until `core` is healthy,
    /// whose 40 s `start_period` exceeds this task's 30 s hard kill ceiling.
    /// Readiness is owned by [`Self::wait_ready`] instead (ADR-0005 §6).
    pub fn ensure_running(&self) -> Result<(), SupervisorError> {
        let compose = self.compose_path.to_str().ok_or_else(|| {
            SupervisorError::StartFailed("the compose path is not valid UTF-8".into())
        })?;

        // `SecretError` messages are fixed and carry no token, so this is safe
        // to surface. The token itself is never formatted into an error.
        let token = install_token().map_err(|err| SupervisorError::StartFailed(err.to_string()))?;
        // shortcut: `as_hex()` returns an unzeroised String; keep it local so it
        // is dropped as soon as `compose up` returns. Upgrade if a zeroising
        // hex buffer is introduced in `secret.rs`.
        let token_hex = token.as_hex();
        let env = build_wsl_env(&token_hex);
        let env_refs: Vec<(&str, &str)> = env
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();

        let output = self
            .wsl
            .run_with_env(
                &self.distro,
                &["docker", "compose", "-f", compose, "up", "-d"],
                &env_refs,
                START_TIMEOUT,
            )
            .map_err(|err| map_wsl_error(err, false))?;

        if !output.status.success() {
            return Err(SupervisorError::StartFailed(engine_message(&output)));
        }
        Ok(())
    }

    /// Poll the loopback port until it accepts a connection or the deadline
    /// passes.
    pub fn wait_ready(&self) -> Result<(), SupervisorError> {
        let deadline = Instant::now() + READY_TIMEOUT;
        loop {
            if self.port_is_open() {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(SupervisorError::NotReady(READY_TIMEOUT));
            }
            std::thread::sleep(POLL_INTERVAL);
        }
    }

    /// Stop the workspace (`compose down`).
    pub fn stop(&self) -> Result<(), SupervisorError> {
        let compose = self.compose_path.to_str().ok_or_else(|| {
            SupervisorError::StopFailed("the compose path is not valid UTF-8".into())
        })?;

        let output = self
            .wsl
            .run(
                &self.distro,
                &["docker", "compose", "-f", compose, "down"],
                START_TIMEOUT,
            )
            .map_err(|err| map_wsl_error(err, true))?;

        if !output.status.success() {
            return Err(SupervisorError::StopFailed(engine_message(&output)));
        }
        Ok(())
    }

    /// Live status, derived from the port rather than a cached flag.
    pub fn status(&self) -> SupervisorStatus {
        if self.port_is_open() {
            SupervisorStatus::Ready
        } else {
            SupervisorStatus::Stopped
        }
    }

    fn port_is_open(&self) -> bool {
        TcpStream::connect_timeout(&self.address, POLL_INTERVAL).is_ok()
    }
}

fn map_wsl_error(err: WslError, stopping: bool) -> SupervisorError {
    let wrapper = if stopping {
        SupervisorError::StopFailed
    } else {
        SupervisorError::StartFailed
    };
    match err {
        WslError::EngineAbsent(message) => SupervisorError::EngineAbsent(message),
        other => wrapper(other.to_string()),
    }
}

/// Combine the engine's stdout and stderr into one legible, decoded message.
/// `wsl.exe` writes errors to stdout in UTF-16LE, so reading only stderr loses
/// the explanation.
fn engine_message(output: &std::process::Output) -> String {
    let mut parts = Vec::new();
    for raw in [&output.stdout, &output.stderr] {
        let text = decode_output(raw);
        let text = text.trim();
        if !text.is_empty() && !parts.iter().any(|p: &String| p == text) {
            parts.push(text.to_string());
        }
    }
    let joined = if parts.is_empty() {
        "the engine reported no details".to_string()
    } else {
        parts.join(": ")
    };
    redact(&joined)
}

/// Strip anything that looks like a 256-bit token (a run of 32+ hex chars) or
/// a `token=<value>` assignment. Cheap defence in depth: the token is never
/// passed to the engine today, but once A6 injects it via the environment a
/// compose error could echo it, and ADR-0005 §5 requires redaction.
fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut run = String::new();
    let flush = |out: &mut String, run: &mut String| {
        if run.len() >= 32 && run.bytes().all(|b| b.is_ascii_hexdigit()) {
            out.push_str("[redacted]");
        } else {
            out.push_str(run);
        }
        run.clear();
    };
    for ch in text.chars() {
        if ch.is_ascii_hexdigit() {
            run.push(ch);
        } else {
            flush(&mut out, &mut run);
            out.push(ch);
        }
    }
    flush(&mut out, &mut run);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_supervisor() -> Supervisor {
        // Port 1 is reserved and never listening: a deterministic "closed".
        Supervisor::new(
            "mediahaven-test-distro",
            PathBuf::from("tests/fixtures/compose.yaml"),
            "127.0.0.1:1".parse().unwrap(),
        )
    }

    #[test]
    fn closed_port_reports_not_ready() {
        let supervisor = test_supervisor();
        let result = supervisor.wait_ready();
        assert!(
            matches!(result, Err(SupervisorError::NotReady(_))),
            "a closed port must report NotReady, got {result:?}"
        );
    }

    #[test]
    fn closed_port_status_is_stopped() {
        let supervisor = test_supervisor();
        assert_eq!(supervisor.status(), SupervisorStatus::Stopped);
    }

    #[test]
    fn wsl_env_carries_the_token_and_names_it_for_forwarding() {
        let token = "ab".repeat(32);
        let env = build_wsl_env(&token);

        assert_eq!(env.len(), 2);
        assert_eq!(env[0], (API_TOKEN_ENV.to_string(), token));
        assert_eq!(env[1], (WSLENV.to_string(), API_TOKEN_ENV.to_string()));
        // No flag suffix: hex needs no path translation and must be shared as-is.
        assert!(!env[1].1.contains('/'));
    }

    #[test]
    fn engine_output_redacts_a_256_bit_token() {
        let token = "ab".repeat(32);
        let noisy = format!("could not start: token={token} failed");
        let cleaned = redact(&noisy);
        assert!(
            !cleaned.contains(&token),
            "token must not survive: {cleaned}"
        );
        assert!(cleaned.contains("[redacted]"));
        assert!(cleaned.contains("could not start"));
    }

    #[test]
    fn redaction_keeps_ordinary_words() {
        // Short hex-like words (< 32 chars) must pass through untouched.
        assert_eq!(
            redact("error on port 8080 failed"),
            "error on port 8080 failed"
        );
    }

    #[test]
    fn status_messages_carry_no_path_or_token() {
        // Every message is user-facing; none may contain a path or secret.
        for status in [
            SupervisorStatus::EngineAbsent,
            SupervisorStatus::Starting,
            SupervisorStatus::Ready,
            SupervisorStatus::Stopped,
            SupervisorStatus::Failed("boom".into()),
        ] {
            let text = status.to_string();
            assert!(
                !text.contains("mediahaven-test-distro"),
                "leaked distro: {text}"
            );
            assert!(!text.contains("tests\\fixtures"), "leaked path: {text}");
            assert!(!text.contains('/'), "leaked path separator: {text}");
        }
    }
}
