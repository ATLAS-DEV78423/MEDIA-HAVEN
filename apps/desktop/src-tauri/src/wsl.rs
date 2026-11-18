//! Minimal `wsl.exe` driver.
//!
//! Every invocation is an **argument vector** — never a shell string. The host
//! never calls `docker.exe` on Windows and never enables a TCP Docker endpoint
//! (ADR-0005 §3); all container work happens inside the dedicated WSL2 distro.
//!
//! `wsl.exe` has no built-in timeout, so the driver polls `try_wait` against a
//! deadline and kills the child on expiry. A killed `wsl.exe` may leave the
//! container running; Task A8 owns the orphan-cleanup proof.

use std::io::Read;
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

/// A legible WSL failure. The strings are safe to show the user: they carry no
/// token, path outside the compose file, or host detail.
#[derive(Debug, thiserror::Error)]
pub enum WslError {
    /// `wsl.exe` could not be started at all: the engine or WSL is absent.
    #[error("engine_absent: Windows Subsystem for Linux is not available ({0})")]
    EngineAbsent(String),

    /// `wsl.exe` ran and exited non-zero. `stderr` is the engine's own text.
    #[error("engine_failed: the WSL distro reported: {stderr}")]
    EngineFailed { stderr: String },

    /// The command exceeded its deadline and was killed.
    #[error("engine_timeout: the operation did not finish within {0:?}")]
    EngineTimeout(Duration),

    /// An I/O failure while spawning or reading the child.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Runs `wsl.exe -d <distro> <args…>` with a hard timeout.
#[derive(Debug, Clone)]
pub struct Wsl;

impl Wsl {
    pub fn new() -> Self {
        Self
    }

    /// Run a command inside `distro`, returning its captured output.
    ///
    /// `args` are appended verbatim; the caller is responsible for never
    /// interpolating a path or token into a single argument.
    pub fn run(&self, distro: &str, args: &[&str], timeout: Duration) -> Result<Output, WslError> {
        self.run_with_env(distro, args, &[], timeout)
    }

    /// Run a command inside `distro` with child-only environment additions.
    ///
    /// `env` is applied with `Command::env`, so the variables exist only in the
    /// spawned `wsl.exe` process — never in this process's global environment.
    /// `wsl.exe` forwards the ones named in its `WSLENV` value into the distro.
    /// No argument is ever built from an env value, so secrets stay out of the
    /// Windows process list.
    pub fn run_with_env(
        &self,
        distro: &str,
        args: &[&str],
        env: &[(&str, &str)],
        timeout: Duration,
    ) -> Result<Output, WslError> {
        run_with_timeout(build_command(distro, args, env), timeout)
    }
}

/// Build the exact `wsl.exe` invocation. Kept separate from spawning so the
/// argument/environment split can be asserted without an engine (the token is
/// in `env`, never in `args`).
fn build_command(distro: &str, args: &[&str], env: &[(&str, &str)]) -> Command {
    let mut cmd = Command::new("wsl.exe");
    cmd.arg("-d").arg(distro);
    for arg in args {
        cmd.arg(arg);
    }
    for (key, value) in env {
        cmd.env(key, value);
    }
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd
}

impl Default for Wsl {
    fn default() -> Self {
        Self::new()
    }
}

fn run_with_timeout(cmd: Command, timeout: Duration) -> Result<Output, WslError> {
    let mut child = spawn(cmd)?;
    let deadline = Instant::now() + timeout;

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();
                if let Some(mut pipe) = child.stdout.take() {
                    pipe.read_to_end(&mut stdout)?;
                }
                if let Some(mut pipe) = child.stderr.take() {
                    pipe.read_to_end(&mut stderr)?;
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
                    return Err(WslError::EngineTimeout(timeout));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(err) => return Err(WslError::Io(err)),
        }
    }
}

fn spawn(mut cmd: Command) -> Result<Child, WslError> {
    cmd.spawn()
        .map_err(|err| WslError::EngineAbsent(err.to_string()))
}

/// Decode a `wsl.exe` output buffer.
///
/// `wsl.exe` writes **UTF-16LE** on Windows, not UTF-8. Decoding it as UTF-8
/// yields NUL-separated gibberish, so the driver sniffs the encoding and
/// converts. A UTF-8 buffer passes through unchanged.
pub fn decode_output(bytes: &[u8]) -> String {
    let looks_utf16 = bytes.len() >= 2
        && bytes.len().is_multiple_of(2)
        && bytes.iter().skip(1).step_by(2).filter(|b| **b == 0).count() * 4 > bytes.len();
    if looks_utf16 {
        let (pairs, _) = bytes.as_chunks::<2>();
        let units: Vec<u16> = pairs
            .iter()
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The forced-failure case the plan requires: a distro name that does not
    /// exist must produce a legible error and must not start anything.
    ///
    /// This is environment-tolerant: on a machine with no WSL at all the
    /// driver reports `EngineAbsent`; on a machine with WSL it reports
    /// `EngineFailed`. Both are legible; a panic or a silent `Ok` is not.
    #[test]
    fn absent_distro_is_a_legible_engine_failure() {
        let wsl = Wsl::new();
        let result = wsl.run(
            "mediahaven-distro-that-does-not-exist",
            &["true"],
            Duration::from_secs(30),
        );
        match result {
            Ok(output) => {
                // `wsl.exe` reports the failure as a non-zero exit with the
                // message on stdout (UTF-16LE). Both are legible.
                assert!(
                    !output.status.success(),
                    "a nonexistent distro must exit non-zero"
                );
                let message = decode_output(&output.stdout);
                assert!(
                    message.contains("no distribution")
                        || message.contains("WSL_E_DISTRO_NOT_FOUND"),
                    "the engine must report why the distro is unknown, got {message:?}"
                );
            }
            Err(WslError::EngineAbsent(_)) => {
                // No WSL on this machine: still a legible, non-panicking state.
            }
            Err(WslError::EngineTimeout(_)) => {
                panic!("a nonexistent distro must fail fast, not time out")
            }
            Err(other) => panic!("expected a legible engine failure, got {other:?}"),
        }
    }

    #[test]
    fn utf16_output_is_decoded_to_readable_text() {
        // "There is no distribution" in UTF-16LE, as wsl.exe emits.
        let text = "There is no distribution";
        let mut bytes = Vec::new();
        for unit in text.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        let decoded = decode_output(&bytes);
        assert_eq!(decoded, text);
    }

    #[test]
    fn utf8_output_passes_through() {
        let decoded = decode_output(b"plain utf8");
        assert_eq!(decoded, "plain utf8");
    }

    /// The injected token must live in the child environment only: `get_envs`
    /// must carry it, `get_args` must not, and this process's environment must
    /// be left untouched. No WSL engine is needed for this assertion.
    #[test]
    fn token_is_in_child_env_only_never_argv_or_process_env() {
        use std::ffi::OsStr;

        let token = "ab".repeat(32); // 64 hex chars, as `SecretToken::as_hex` returns
        let env = [
            ("MEDIAHAVEN_API_TOKEN", token.as_str()),
            ("WSLENV", "MEDIAHAVEN_API_TOKEN"),
        ];
        let args = ["docker", "compose", "-f", "compose.yaml", "up", "-d"];

        let before = std::env::var_os("MEDIAHAVEN_API_TOKEN");
        let cmd = build_command("mediahaven-test-distro", &args, &env);
        assert_eq!(
            std::env::var_os("MEDIAHAVEN_API_TOKEN"),
            before,
            "building the command must not mutate the global process environment"
        );

        let envs: Vec<_> = cmd.get_envs().collect();
        assert!(
            envs.contains(&(OsStr::new("MEDIAHAVEN_API_TOKEN"), Some(OsStr::new(&token)))),
            "the token must be in the child env"
        );
        assert!(
            envs.contains(&(
                OsStr::new("WSLENV"),
                Some(OsStr::new("MEDIAHAVEN_API_TOKEN"))
            )),
            "WSLENV must name the forwarded variable"
        );
        for arg in cmd.get_args() {
            assert!(
                !arg.to_string_lossy().contains(&token),
                "no argv element may contain the token"
            );
        }
    }
}
