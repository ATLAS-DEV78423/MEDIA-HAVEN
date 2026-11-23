//! Container workspace lifecycle.
//!
//! By default the supervisor starts, waits for, health-checks and stops the
//! `core` container through `wsl.exe` (ADR-0005 §3, ADR-0013). An opt-in
//! `MEDIAHAVEN_ENGINE=windows` uses the Windows Docker CLI instead. No TCP
//! Docker endpoint is ever enabled.
//!
//! Readiness here is a **transport** check: can the host open the loopback
//! port? The authenticated `/readyz` call is the command layer's gate (A5/A7),
//! which owns the HMAC signing — building the signature here would build it
//! twice.

use std::io::Read;
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
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
const ENGINE_ENV: &str = "MEDIAHAVEN_ENGINE";

// Fixed, user-safe messages: no path, CLI error text or host detail.
const WINDOWS_SPAWN_FAILED: &str = "the Windows Docker CLI could not be started";
const WINDOWS_IO_FAILED: &str = "the Windows Docker CLI could not be run";
const WINDOWS_TIMEOUT: &str = "the Windows Docker CLI did not finish in time";

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

/// Which host CLI drives the engine. WSL stays the default (ADR-0001).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EngineMode {
    Wsl,
    Windows,
}

impl EngineMode {
    fn from_env() -> Self {
        engine_mode_from(std::env::var(ENGINE_ENV).ok().as_deref())
    }
}

/// Only the exact value `windows` opts in; every other value keeps ADR-0001.
fn engine_mode_from(value: Option<&str>) -> EngineMode {
    match value {
        Some("windows") => EngineMode::Windows,
        _ => EngineMode::Wsl,
    }
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
    engine_mode: EngineMode,
    /// Optional rendered mount override. When present it is passed as a second
    /// `-f` immediately after the base compose so its mounts override/merge in.
    override_path: Option<PathBuf>,
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
            engine_mode: EngineMode::from_env(),
            override_path: None,
        }
    }

    /// Attach a rendered host-mounted override file (builder: `new` stays
    /// unchanged for existing callers and tests).
    pub fn with_override(mut self, path: impl Into<PathBuf>) -> Self {
        self.override_path = Some(path.into());
        self
    }

    /// Build the `docker compose -f <path> [-f <override>] …` argv for the
    /// active engine. Only WSL gets the `/mnt/<drive>/…` form; Windows keeps
    /// both paths native.
    fn compose_args(&self, compose: &str, rest: &[&str]) -> Vec<String> {
        let map_path = |raw: &str| match self.engine_mode {
            EngineMode::Wsl => wsl_path(raw),
            EngineMode::Windows => raw.to_string(),
        };
        let mut args = vec![
            "docker".to_string(),
            "compose".to_string(),
            "-f".to_string(),
            map_path(compose),
        ];
        if let Some(over) = &self.override_path {
            let over = over.to_string_lossy().into_owned();
            args.push("-f".to_string());
            args.push(map_path(&over));
        }
        args.extend(rest.iter().map(|arg| (*arg).to_string()));
        args
    }

    /// Run `docker compose` through the active engine with a hard timeout.
    fn run_compose(
        &self,
        compose: &str,
        rest: &[&str],
        env: &[(&str, &str)],
        stopping: bool,
    ) -> Result<Output, SupervisorError> {
        let args = self.compose_args(compose, rest);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        match self.engine_mode {
            EngineMode::Wsl => self
                .wsl
                .run_with_env(&self.distro, &refs, env, START_TIMEOUT)
                .map_err(|err| map_wsl_error(err, stopping)),
            EngineMode::Windows => run_windows(&refs, env, START_TIMEOUT, stopping),
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

        let output = self.run_compose(compose, &["up", "-d"], &env_refs, false)?;

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

        let output = self.run_compose(compose, &["down"], &[], true)?;

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

/// Convert a Windows absolute path into the WSL form used by the distro.
/// `std::env::temp_dir()` yields a normal drive path; WSL mounts drives at
/// `/mnt/<lowercase-letter>/...`. Already-WSL paths pass through unchanged.
fn wsl_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let bytes = normalized.as_bytes();
    if bytes.len() >= 3 && bytes[1] == b':' && bytes[2] == b'/' && bytes[0].is_ascii_alphabetic() {
        let drive = (bytes[0] as char).to_ascii_lowercase();
        return format!("/mnt/{drive}{}", &normalized[2..]);
    }
    normalized
}

/// Build the Windows invocation from the shared argv: element 0 is the program.
/// Kept separate from spawning so the program/arg split is testable without an
/// engine.
fn build_windows_command(args: &[&str], env: &[(&str, &str)]) -> Command {
    let mut cmd = Command::new(args[0]);
    cmd.args(&args[1..]).envs(env.iter().copied());
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd
}

/// Run the Windows Docker CLI with the same hard timeout as the WSL driver.
fn run_windows(
    args: &[&str],
    env: &[(&str, &str)],
    timeout: Duration,
    stopping: bool,
) -> Result<Output, SupervisorError> {
    let wrapper = if stopping {
        SupervisorError::StopFailed
    } else {
        SupervisorError::StartFailed
    };
    let mut child = build_windows_command(args, env)
        .spawn()
        .map_err(|_| wrapper(WINDOWS_SPAWN_FAILED.into()))?;
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();
                if let Some(mut pipe) = child.stdout.take() {
                    pipe.read_to_end(&mut stdout)
                        .map_err(|_| wrapper(WINDOWS_IO_FAILED.into()))?;
                }
                if let Some(mut pipe) = child.stderr.take() {
                    pipe.read_to_end(&mut stderr)
                        .map_err(|_| wrapper(WINDOWS_IO_FAILED.into()))?;
                }
                return Ok(Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(wrapper(WINDOWS_TIMEOUT.into()));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => return Err(wrapper(WINDOWS_IO_FAILED.into())),
        }
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
    fn engine_mode_selects_windows_only_for_the_exact_value() {
        assert_eq!(engine_mode_from(Some("windows")), EngineMode::Windows);
        assert_eq!(engine_mode_from(None), EngineMode::Wsl);
        assert_eq!(engine_mode_from(Some("wsl")), EngineMode::Wsl);
        assert_eq!(engine_mode_from(Some("Windows")), EngineMode::Wsl);
    }

    #[test]
    fn windows_mode_keeps_the_native_compose_path() {
        const NATIVE: &str = r"C:\Users\Stanley\AppData\Local\Temp\mediahaven-compose.yaml";
        let mut supervisor = test_supervisor();
        supervisor.engine_mode = EngineMode::Windows;
        let args = supervisor.compose_args(NATIVE, &["up", "-d"]);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        assert_eq!(refs, ["docker", "compose", "-f", NATIVE, "up", "-d"]);

        // The default WSL mode still converts the exact same path.
        let args = test_supervisor().compose_args(NATIVE, &["down"]);
        assert_eq!(
            args[3],
            "/mnt/c/Users/Stanley/AppData/Local/Temp/mediahaven-compose.yaml"
        );
    }

    #[test]
    fn override_follows_the_base_compose_in_both_engine_modes() {
        const NATIVE: &str = r"C:\Users\Stanley\AppData\Local\Temp\mediahaven-compose.yaml";
        const OVER: &str = r"C:\Users\Stanley\AppData\Local\Temp\override.yaml";

        let mut wsl = test_supervisor().with_override(OVER);
        wsl.engine_mode = EngineMode::Wsl;
        let args = wsl.compose_args(NATIVE, &["up", "-d"]);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        assert_eq!(
            refs,
            [
                "docker",
                "compose",
                "-f",
                "/mnt/c/Users/Stanley/AppData/Local/Temp/mediahaven-compose.yaml",
                "-f",
                "/mnt/c/Users/Stanley/AppData/Local/Temp/override.yaml",
                "up",
                "-d",
            ]
        );

        let mut windows = test_supervisor().with_override(OVER);
        windows.engine_mode = EngineMode::Windows;
        let args = windows.compose_args(NATIVE, &["down"]);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        assert_eq!(
            refs,
            ["docker", "compose", "-f", NATIVE, "-f", OVER, "down"]
        );

        // No override keeps the historical argv byte-for-byte.
        let mut windows_plain = test_supervisor();
        windows_plain.engine_mode = EngineMode::Windows;
        let args = windows_plain.compose_args(NATIVE, &["down"]);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        assert_eq!(refs, ["docker", "compose", "-f", NATIVE, "down"]);
    }

    #[test]
    fn windows_command_runs_docker_compose_once() {
        const NATIVE: &str = r"C:\Users\Stanley\AppData\Local\Temp\mediahaven-compose.yaml";
        let mut supervisor = test_supervisor();
        supervisor.engine_mode = EngineMode::Windows;
        let args = supervisor.compose_args(NATIVE, &["up", "-d"]);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();

        let cmd = build_windows_command(&refs, &[]);
        assert_eq!(cmd.get_program(), "docker");
        let argv: Vec<&str> = cmd.get_args().map(|arg| arg.to_str().unwrap()).collect();
        assert_eq!(argv, ["compose", "-f", NATIVE, "up", "-d"]);
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
    fn wsl_path_converts_a_windows_temp_path() {
        assert_eq!(
            wsl_path(r"C:\Users\Stanley\AppData\Local\Temp\mediahaven-compose.yaml"),
            "/mnt/c/Users/Stanley/AppData/Local/Temp/mediahaven-compose.yaml"
        );
    }

    #[test]
    fn wsl_path_leaves_an_existing_wsl_path_unchanged() {
        assert_eq!(
            wsl_path("/tmp/mediahaven-compose.yaml"),
            "/tmp/mediahaven-compose.yaml"
        );
    }

    #[test]
    fn wsl_path_lowercases_the_drive_letter() {
        assert_eq!(
            wsl_path(r"d:\temp\compose.yaml"),
            "/mnt/d/temp/compose.yaml"
        );
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
