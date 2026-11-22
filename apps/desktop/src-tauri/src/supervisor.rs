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

use crate::secret::{
    install_token, load_secret, PROWLARR_KEY_ACCOUNT, PROWLARR_URL_ACCOUNT, TMDB_ACCOUNT,
    TMDB_SERVICE, TOKEN_SERVICE,
};
use crate::wsl::{decode_output, Wsl, WslError};

/// Bounded timeouts (plan A4).
pub const START_TIMEOUT: Duration = Duration::from_secs(30);
pub const READY_TIMEOUT: Duration = Duration::from_secs(10);
pub const POLL_INTERVAL: Duration = Duration::from_millis(250);

const API_TOKEN_ENV: &str = "MEDIAHAVEN_API_TOKEN";
const WSLENV: &str = "WSLENV";
const TMDB_KEY_ENV: &str = "MEDIAHAVEN_TMDB_API_KEY";
const PROWLARR_URL_ENV: &str = "MEDIAHAVEN_PROWLARR_URL";
const PROWLARR_KEY_ENV: &str = "MEDIAHAVEN_PROWLARR_API_KEY";

/// Child-only environment for `compose up`. `WSLENV` lists each forwarded name
/// with no flag suffix: the values are plain text, so WSL passes them through
/// unchanged with no path translation. Optional operator inputs are added only
/// when configured, leaving the no-input case byte-for-byte unchanged.
fn build_wsl_env(
    token_hex: &str,
    tmdb: Option<&str>,
    prowlarr_url: Option<&str>,
    prowlarr_key: Option<&str>,
) -> Vec<(String, String)> {
    let mut env = vec![
        (API_TOKEN_ENV.to_string(), token_hex.to_string()),
        (WSLENV.to_string(), API_TOKEN_ENV.to_string()),
    ];
    for (name, value) in [
        (TMDB_KEY_ENV, tmdb),
        (PROWLARR_URL_ENV, prowlarr_url),
        (PROWLARR_KEY_ENV, prowlarr_key),
    ] {
        if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
            env.push((name.to_string(), value.to_string()));
            if let Some((_, wslenv)) = env.iter_mut().find(|(k, _)| k == WSLENV) {
                wslenv.push(':');
                wslenv.push_str(name);
            }
        }
    }
    env
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

/// Map a lifecycle outcome to one fixed, user-safe startup status. No path,
/// distro name, env value or engine detail may enter this value.
pub fn startup_status(result: &Result<(), SupervisorError>) -> &'static str {
    match result {
        Ok(()) => "ready",
        Err(SupervisorError::EngineAbsent(_)) => "setup-required",
        Err(_) => "failed",
    }
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
        // Optional operator inputs: absent means no env entry, corrupt fails
        // closed. `SecretError` text is fixed and carries no secret.
        let tmdb_token = load_secret(TMDB_SERVICE, TMDB_ACCOUNT)
            .map_err(|err| SupervisorError::StartFailed(err.to_string()))?;
        let prowlarr_url = load_secret(TOKEN_SERVICE, PROWLARR_URL_ACCOUNT)
            .map_err(|err| SupervisorError::StartFailed(err.to_string()))?;
        let prowlarr_key = load_secret(TOKEN_SERVICE, PROWLARR_KEY_ACCOUNT)
            .map_err(|err| SupervisorError::StartFailed(err.to_string()))?;
        // shortcut: none of these Strings are zeroised on drop; keep them local
        // so they are dropped as soon as `compose up` returns, as with token_hex.
        let env = build_wsl_env(
            &token_hex,
            tmdb_token.as_deref(),
            prowlarr_url.as_deref(),
            prowlarr_key.as_deref(),
        );
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

/// Strip secret values from engine output before it becomes a user-facing
/// error: `<MEDIAHAVEN_*_ENV>=<value>` and `token=<value>` assignments, plus any
/// remaining run of 32+ hex chars. Operator keys have dots/underscores and only
/// short hex segments, so the hex pass alone would let them through. Cheap
/// defence in depth: every secret is passed via the environment, so a compose
/// error could echo one, and ADR-0005 §5 requires redaction.
fn redact(text: &str) -> String {
    const MARKER: &str = "[redacted]";
    let mut masked = String::with_capacity(text.len());
    let mut pos = 0;
    for (at, _) in text.match_indices('=') {
        // An '=' inside a value masked on an earlier pass lies before pos.
        if at < pos {
            continue;
        }
        let key = text[pos..at]
            .rsplit(|c: char| c.is_whitespace() || c == ',' || c == '"' || c == '\'')
            .next()
            .unwrap_or("");
        if key != "token"
            && key != TMDB_KEY_ENV
            && key != PROWLARR_URL_ENV
            && key != PROWLARR_KEY_ENV
        {
            continue;
        }
        let after = at + 1;
        // A quoted value keeps its quotes around the marker.
        let (start, quote) = match text[after..].chars().next() {
            Some(c @ ('"' | '\'')) => (after + c.len_utf8(), Some(c)),
            _ => (after, None),
        };
        let end = text[start..]
            .find(|c: char| {
                c.is_whitespace() || matches!(c, ',' | ')' | ']' | '}') || Some(c) == quote
            })
            .map(|offset| start + offset)
            .unwrap_or(text.len());
        masked.push_str(&text[pos..start]);
        masked.push_str(MARKER);
        pos = end;
    }
    masked.push_str(&text[pos..]);

    let mut out = String::with_capacity(masked.len());
    let mut run = String::new();
    let flush = |out: &mut String, run: &mut String| {
        if run.len() >= 32 && run.bytes().all(|b| b.is_ascii_hexdigit()) {
            out.push_str(MARKER);
        } else {
            out.push_str(run);
        }
        run.clear();
    };
    for ch in masked.chars() {
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
    fn startup_status_maps_every_outcome_to_a_fixed_string() {
        const SENTINEL: &str = "SENTINEL-ENV-PATH-DETAIL";
        assert_eq!(startup_status(&Ok(())), "ready");
        assert_eq!(
            startup_status(&Err(SupervisorError::EngineAbsent(SENTINEL.into()))),
            "setup-required"
        );
        for error in [
            SupervisorError::StartFailed(SENTINEL.into()),
            SupervisorError::NotReady(READY_TIMEOUT),
            SupervisorError::StopFailed(SENTINEL.into()),
            SupervisorError::Io(std::io::Error::other(SENTINEL)),
        ] {
            let status = startup_status(&Err(error));
            assert_eq!(status, "failed");
            assert!(!status.contains(SENTINEL), "leaked detail: {status}");
            assert!(!status.contains("MEDIAHAVEN_WSL_DISTRO"));
            assert!(!status.contains("compose.yaml"));
            assert!(!status.contains("127.0.0.1"));
        }
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
        let env = build_wsl_env(&token, None, None, None);

        assert_eq!(env.len(), 2);
        assert_eq!(env[0], (API_TOKEN_ENV.to_string(), token));
        assert_eq!(env[1], (WSLENV.to_string(), API_TOKEN_ENV.to_string()));
        // No flag suffix: hex needs no path translation and must be shared as-is.
        assert!(!env[1].1.contains('/'));
    }

    #[test]
    fn wsl_env_includes_tmdb_when_present() {
        let env = build_wsl_env("install-token", Some("tmdb-token"), None, None);
        assert!(env
            .iter()
            .any(|(k, v)| k == "MEDIAHAVEN_TMDB_API_KEY" && v == "tmdb-token"));
        assert!(env
            .iter()
            .any(|(k, v)| k == WSLENV && v.contains("MEDIAHAVEN_TMDB_API_KEY")));

        // No token means exactly the two pre-existing entries, unchanged.
        assert_eq!(build_wsl_env("install-token", None, None, None).len(), 2);
    }

    #[test]
    fn wsl_env_includes_prowlarr_inputs_only_when_present() {
        let env = build_wsl_env(
            "install-token",
            None,
            Some("http://127.0.0.1:9696"),
            Some("a-prowlarr-key"),
        );
        assert!(env
            .iter()
            .any(|(k, v)| k == PROWLARR_URL_ENV && v == "http://127.0.0.1:9696"));
        assert!(env
            .iter()
            .any(|(k, v)| k == PROWLARR_KEY_ENV && v == "a-prowlarr-key"));
        let wslenv = env.iter().find(|(k, _)| k == WSLENV).unwrap();
        assert!(wslenv.1.contains(PROWLARR_URL_ENV));
        assert!(wslenv.1.contains(PROWLARR_KEY_ENV));

        // Blank or absent inputs add nothing, so the no-input case is unchanged.
        assert_eq!(
            build_wsl_env("install-token", None, Some("  "), Some("")).len(),
            2
        );
        assert_eq!(build_wsl_env("install-token", None, None, None).len(), 2);
    }

    #[test]
    fn engine_output_redacts_prowlarr_inputs_in_an_assignment() {
        const SYNTHETIC: &str = "a-prowlarr-sentinel";
        for noisy in [
            format!("compose error: MEDIAHAVEN_PROWLARR_URL={SYNTHETIC} failed"),
            format!("compose error: MEDIAHAVEN_PROWLARR_API_KEY=\"{SYNTHETIC}\" failed"),
        ] {
            let cleaned = redact(&noisy);
            assert!(!cleaned.contains(SYNTHETIC), "value survived: {cleaned}");
            assert!(cleaned.contains("[redacted]"), "no marker: {cleaned}");
        }
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
    fn engine_output_redacts_a_tmdb_style_token_in_an_assignment() {
        // A TMDB read token has dots/underscores and only short hex segments;
        // the generic 32-hex pass alone would let it survive verbatim.
        const SYNTHETIC: &str =
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
        for noisy in [
            format!("compose error: MEDIAHAVEN_TMDB_API_KEY={SYNTHETIC} failed"),
            format!("compose error: token={SYNTHETIC} failed"),
            format!("compose error: token=\"{SYNTHETIC}\" failed"),
        ] {
            let cleaned = redact(&noisy);
            assert!(!cleaned.contains(SYNTHETIC), "token survived: {cleaned}");
            assert!(cleaned.contains("[redacted]"), "no marker: {cleaned}");
            assert!(cleaned.contains("compose error:"), "prefix lost: {cleaned}");
            assert!(cleaned.contains(" failed"), "tail lost: {cleaned}");
        }
    }

    #[test]
    fn redaction_handles_a_value_containing_an_equals_sign() {
        // The second '=' sits inside the value already masked by the first;
        // slicing text[pos..at] for it must not panic or leak the value.
        let cleaned = redact("token=a=b failed");
        assert!(!cleaned.contains("a=b"), "value survived: {cleaned}");
        assert!(cleaned.contains("[redacted]"), "no marker: {cleaned}");
        assert!(cleaned.contains(" failed"), "tail lost: {cleaned}");
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
