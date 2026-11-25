//! Windows Defender pre-play gate (Task 2 of the downloaded-media plan).
//!
//! A downloaded file is not playable until Windows Defender has scanned it and
//! reported no threat. Absence of the scanner, a timeout, or any parse
//! ambiguity fails closed to `Unavailable` — never `Clean`. The file is never
//! executed here: `MpCmdRun.exe` receives the host path as a single argument in
//! an argument vector, with no shell and no path in any renderer DTO.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Bounded so a hung scan can never block playback indefinitely.
const SCAN_TIMEOUT: Duration = Duration::from_secs(120);
const POLL_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefenderState {
    NotScanned,
    Clean,
    Threat,
    Unavailable,
}

impl DefenderState {
    /// Stable, path-free spelling used in storage and display state.
    pub fn as_str(self) -> &'static str {
        match self {
            DefenderState::NotScanned => "not_scanned",
            DefenderState::Clean => "clean",
            DefenderState::Threat => "threat",
            DefenderState::Unavailable => "unavailable",
        }
    }

    pub fn from_stored(value: &str) -> Self {
        match value {
            "clean" => DefenderState::Clean,
            "threat" => DefenderState::Threat,
            "unavailable" => DefenderState::Unavailable,
            // Unknown, empty and missing values all fail closed.
            _ => DefenderState::NotScanned,
        }
    }

    pub fn is_clean(self) -> bool {
        matches!(self, DefenderState::Clean)
    }
}

/// Injectable seam: tests supply a fake, production uses `MpCmdRunScanner`.
pub trait Scanner {
    fn scan(&self, host_path: &str) -> DefenderState;
}

/// Real scanner. Windows-only in effect; elsewhere it fails closed.
pub struct MpCmdRunScanner;

impl Scanner for MpCmdRunScanner {
    fn scan(&self, host_path: &str) -> DefenderState {
        defender_scan(host_path)
    }
}

/// Scan `host_path` with the host's Windows Defender CLI.
/// Missing binary, non-Windows host, spawn error or timeout => `Unavailable`.
pub fn defender_scan(host_path: &str) -> DefenderState {
    if !cfg!(windows) || host_path.trim().is_empty() {
        return DefenderState::Unavailable;
    }
    let Some(exe) = locate_mpcmdrun() else {
        return DefenderState::Unavailable;
    };
    run_scan(&exe, host_path, SCAN_TIMEOUT)
}

/// The same gate with an injected scanner. Never substitutes a fake in
/// production; exists so logic can be proved without a live Defender call.
pub fn defender_scan_with<S: Scanner + ?Sized>(scanner: &S, host_path: &str) -> DefenderState {
    scanner.scan(host_path)
}

/// True only when BOTH gates (Task 1 probe, Task 2 Defender) are clean.
pub fn both_gates_clean(probe_clean: bool, defender: DefenderState) -> bool {
    probe_clean && defender.is_clean()
}

/// Path-free refusal reason for an honest UI. `None` means playable.
/// A threat or an unavailable scanner is never reported as playable.
pub fn refusal_reason(probe_clean: bool, defender: DefenderState) -> Option<&'static str> {
    if both_gates_clean(probe_clean, defender) {
        return None;
    }
    if !probe_clean {
        return Some("This file did not pass the media inspection check.");
    }
    match defender {
        DefenderState::Threat => Some("Windows Defender flagged this file."),
        DefenderState::Unavailable => Some("Windows Defender could not scan this file."),
        DefenderState::NotScanned => Some("This file has not been scanned yet."),
        DefenderState::Clean => Some("This file is not ready to play."),
    }
}

fn locate_mpcmdrun() -> Option<PathBuf> {
    for root in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Ok(dir) = std::env::var(root) {
            let candidate = Path::new(&dir)
                .join("Windows Defender")
                .join("MpCmdRun.exe");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// This host's MpCmdRun accepts `-Scan`, not the older `-ScanFile` spelling.
fn scan_args(host_path: &str) -> [&str; 5] {
    ["-Scan", "-ScanType", "3", "-File", host_path]
}

fn run_scan(exe: &Path, host_path: &str, timeout: Duration) -> DefenderState {
    // Argument vector only — no shell, no concatenation, no execution of the
    // scanned file. `-ScanType 3` is Microsoft's "scan a specific file" mode.
    let mut child = match Command::new(exe)
        .args(scan_args(host_path))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return DefenderState::Unavailable,
    };

    // Drain both pipes on separate threads: a hung Defender must not deadlock
    // on a full pipe while the main thread waits for its bounded deadline.
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let out_thread = thread::spawn(move || read_all(stdout));
    let err_thread = thread::spawn(move || read_all(stderr));

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < deadline => thread::sleep(POLL_INTERVAL),
            Ok(None) => {
                let _ = child.kill();
                break None;
            }
            Err(_) => {
                let _ = child.kill();
                break None;
            }
        }
    };
    let _ = child.wait();

    let mut output = out_thread.join().unwrap_or_default();
    output.extend_from_slice(&err_thread.join().unwrap_or_default());
    interpret(status.and_then(|s| s.code()), &output)
}

fn read_all<R: Read>(mut reader: Option<R>) -> Vec<u8> {
    let mut bytes = Vec::new();
    if let Some(handle) = reader.as_mut() {
        let _ = handle.read_to_end(&mut bytes);
    }
    bytes
}

/// Fail-closed interpretation of a completed scan. Only a clean exit with no
/// positive or contradictory threat signal becomes `Clean`; everything else is
/// Threat or Unavailable. Defender's wording varies by version, so ambiguity
/// fails closed rather than trying to be clever about the output format.
fn interpret(exit_code: Option<i32>, output: &[u8]) -> DefenderState {
    let mut rest = String::from_utf8_lossy(output).to_ascii_lowercase();
    // "No threats detected" contains "threats detected". Remove absence
    // phrases first so a plain clean result cannot masquerade as a detection,
    // while a genuine threat phrase elsewhere still registers.
    let mut absence = false;
    for phrase in [
        "no threats detected",
        "no threat detected",
        "no threats",
        "no threat",
        "0 threats",
        "0 threat",
    ] {
        if rest.contains(phrase) {
            absence = true;
            rest = rest.replace(phrase, " ");
        }
    }
    let threat_phrase = [
        "threat detected",
        "threats detected",
        "threat found",
        "infected",
        "malware detected",
    ]
    .iter()
    .any(|phrase| rest.contains(phrase));

    if (threat_phrase && !absence) || exit_code == Some(2) {
        DefenderState::Threat
    } else if threat_phrase && absence {
        // Both "threat detected" and "no threat" in a zero-exit output: the
        // signals contradict, so this is NOT Clean — fail closed.
        DefenderState::Unavailable
    } else if exit_code == Some(0) {
        DefenderState::Clean
    } else {
        // Missing exit code (killed/timeout) or unexpected code: fail closed.
        DefenderState::Unavailable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_args_use_the_supported_scan_flag() {
        let args = scan_args("C:\\media\\file.mkv");
        assert_eq!(
            args,
            ["-Scan", "-ScanType", "3", "-File", "C:\\media\\file.mkv"]
        );
    }

    #[test]
    fn interpret_only_clean_on_zero_exit_with_no_threat_signal() {
        assert_eq!(interpret(Some(0), b"Scan finished."), DefenderState::Clean);
        assert_eq!(
            interpret(Some(0), b"No threats detected."),
            DefenderState::Clean
        );
    }

    #[test]
    fn interpret_threat_from_exit_or_output() {
        assert_eq!(interpret(Some(2), b"Scan finished."), DefenderState::Threat);
        assert_eq!(
            interpret(Some(0), b"Threat detected: Trojan:Win32/Sample"),
            DefenderState::Threat
        );
    }

    #[test]
    fn interpret_contradictory_zero_exit_is_not_clean() {
        // A zero exit saying both "threat detected" and "no threat" is
        // ambiguous; the absence phrase must not hide the positive one.
        assert_eq!(
            interpret(Some(0), b"Threat detected: Sample. No threats detected."),
            DefenderState::Unavailable
        );
        assert_eq!(
            interpret(Some(0), b"0 threats found. Threat detected: Sample."),
            DefenderState::Unavailable
        );
        // The plain clean case stays Clean.
        assert_eq!(
            interpret(Some(0), b"No threats detected."),
            DefenderState::Clean
        );
    }

    #[test]
    fn interpret_ambiguous_or_failed_scan_is_unavailable() {
        assert_eq!(interpret(None, b"timeout"), DefenderState::Unavailable);
        assert_eq!(interpret(Some(1), b"error"), DefenderState::Unavailable);
    }

    #[cfg(not(windows))]
    #[test]
    fn non_windows_fails_closed() {
        assert_eq!(
            defender_scan("C:\\synthetic.mkv"),
            DefenderState::Unavailable
        );
    }

    #[test]
    fn state_spelling_is_path_free() {
        for state in [
            DefenderState::NotScanned,
            DefenderState::Clean,
            DefenderState::Threat,
            DefenderState::Unavailable,
        ] {
            let as_str = state.as_str();
            assert!(!as_str.contains('\\'));
            assert!(!as_str.contains('/'));
            assert_eq!(DefenderState::from_stored(as_str), state);
        }
    }
}
