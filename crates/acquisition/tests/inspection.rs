//! Task 1 evidence: the probe gate is fail-closed and builds an exact argument
//! vector. Every probe observation is faked — no Docker engine and no lawfully
//! sourced media file are available here, so the live probe scan is NOT RUN.

use app_core::domain::{DownloadItem, DownloadState, DownloadsScreen};
use media_acquisition::inspection::{
    combine_inspection, download_root_from_env, inspect_completed, DOWNLOADS_MOUNT,
    DOWNLOAD_ROOT_ENV,
};
use media_acquisition::{
    inspect_with_runner, probe_args, InspectionState, ProbeError, ProbeOutput, ProbeRunner,
    FFPROBE_PATH, PROBE_IMAGE,
};

/// A host path used only in internal argv assertions. It must never appear in
/// any renderer DTO.
const HOST_ROOT: &str = r"C:\MediaHaven\downloads";

struct FakeProbe {
    result: Result<ProbeOutput, ProbeError>,
}

impl ProbeRunner for FakeProbe {
    fn run(&self, _container_path: &str) -> Result<ProbeOutput, ProbeError> {
        self.result.clone()
    }
}

fn accepting() -> FakeProbe {
    FakeProbe {
        result: Ok(ProbeOutput {
            success: true,
            stdout: r#"{"format":{"filename":"/downloads/movie.mkv","format_name":"matroska","duration":"90.5"}}"#
                .to_string(),
        }),
    }
}

#[test]
fn valid_media_fixture_classifies_clean() {
    assert_eq!(
        inspect_with_runner(&accepting(), "/downloads/movie.mkv"),
        InspectionState::Clean
    );
}

#[test]
fn text_file_named_mkv_is_suspicious_not_clean() {
    // ffprobe exits non-zero with no format JSON on a text file renamed .mkv.
    let probe = FakeProbe {
        result: Ok(ProbeOutput {
            success: false,
            stdout: String::new(),
        }),
    };
    let state = inspect_with_runner(&probe, "/downloads/fake.mkv");
    assert_eq!(state, InspectionState::Suspicious);
    assert_ne!(
        state,
        InspectionState::Clean,
        "a non-media file is unplayable"
    );
}

#[test]
fn unparseable_probe_output_is_suspicious() {
    let probe = FakeProbe {
        result: Ok(ProbeOutput {
            success: true,
            stdout: "this is not ffprobe json".to_string(),
        }),
    };
    assert_eq!(
        inspect_with_runner(&probe, "/downloads/fake.mkv"),
        InspectionState::Suspicious
    );
}

#[test]
fn unavailable_probe_fails_closed_and_is_never_clean() {
    let probe = FakeProbe {
        result: Err(ProbeError::Unavailable),
    };
    let state = inspect_with_runner(&probe, "/downloads/movie.mkv");
    assert_eq!(state, InspectionState::Unavailable);
    assert_ne!(
        state,
        InspectionState::Clean,
        "fail closed when probe cannot run"
    );
}

#[test]
fn no_non_clean_state_renders_as_clean() {
    for state in [
        InspectionState::Pending,
        InspectionState::Scanning,
        InspectionState::Suspicious,
        InspectionState::Unavailable,
    ] {
        assert_ne!(state.as_str(), "clean");
    }
    assert_eq!(InspectionState::Clean.as_str(), "clean");
}

#[test]
fn path_outside_downloads_is_refused_even_if_the_probe_would_accept_it() {
    // No host file is opened; a path outside the managed mount never reaches
    // the runner at all. The runner's `run` is the accepting one on purpose.
    assert_eq!(
        inspect_with_runner(&accepting(), "/etc/passwd"),
        InspectionState::Suspicious
    );
    assert_eq!(
        inspect_with_runner(&accepting(), "/downloads/../secret.mkv"),
        InspectionState::Suspicious
    );
}

#[test]
fn docker_invocation_binds_the_host_root_read_only_and_has_no_shell() {
    let args = probe_args("/downloads/show/e01.mkv", HOST_ROOT);
    assert_eq!(
        args,
        vec![
            "run".to_string(),
            "--rm".to_string(),
            "--network=none".to_string(),
            "--entrypoint".to_string(),
            FFPROBE_PATH.to_string(),
            "-v".to_string(),
            format!("{HOST_ROOT}:{DOWNLOADS_MOUNT}:ro"),
            PROBE_IMAGE.to_string(),
            "-v".to_string(),
            "error".to_string(),
            "-show_format".to_string(),
            "-of".to_string(),
            "json".to_string(),
            "/downloads/show/e01.mkv".to_string(),
        ]
    );
    // The bind is read-only and precedes the image; ffprobe's own -v error
    // follows it. A fresh probe container can now see what qB wrote.
    let bind = args
        .iter()
        .find(|arg| arg.ends_with(":/downloads:ro"))
        .expect("bind present");
    assert!(bind.starts_with(HOST_ROOT));
    assert!(
        args.iter().position(|a| a == "-v").unwrap()
            < args.iter().position(|a| a == PROBE_IMAGE).unwrap()
    );
    // Absence assertions: no shell, no command string, no network-bound flag,
    // no writable /downloads bind.
    assert!(!args.iter().any(|arg| arg == "sh" || arg == "cmd"));
    assert!(!args
        .iter()
        .any(|arg| arg.contains(';') || arg.contains('`')));
    assert!(!args.iter().any(|arg| arg.ends_with(":/downloads:rw")));
    assert!(!args
        .iter()
        .any(|arg| arg.starts_with("--network=") && arg != "--network=none"));
}

#[test]
fn host_root_comes_from_the_supervisor_env_var_and_fails_closed_when_absent() {
    // Mutating process env is safe here: this is the only test that reads it,
    // and it restores the previous value before returning.
    let previous = std::env::var_os(DOWNLOAD_ROOT_ENV);
    std::env::remove_var(DOWNLOAD_ROOT_ENV);
    assert_eq!(download_root_from_env(), Err(ProbeError::Unavailable));
    std::env::set_var(DOWNLOAD_ROOT_ENV, HOST_ROOT);
    assert_eq!(download_root_from_env(), Ok(HOST_ROOT.to_string()));
    std::env::set_var(DOWNLOAD_ROOT_ENV, "   ");
    assert_eq!(download_root_from_env(), Err(ProbeError::Unavailable));
    match previous {
        Some(value) => std::env::set_var(DOWNLOAD_ROOT_ENV, value),
        None => std::env::remove_var(DOWNLOAD_ROOT_ENV),
    }
}

#[test]
fn renderer_dtos_never_carry_the_host_download_root() {
    // Representative path-free renderer DTOs. The host root lives only in the
    // internal argv above; no DTO field can carry it.
    let item = DownloadItem {
        id: "d-1".to_string(),
        torrent_hash: "h-1".to_string(),
        media_id: None,
        state: DownloadState::Ready,
        progress: 1.0,
    };
    let screen = DownloadsScreen {
        enabled: true,
        tasks: vec![],
        storage_used_bytes: 0,
        storage_limit_bytes: None,
        buffer_notice: None,
    };
    let json = serde_json::to_string(&(&item, &screen)).expect("serialize DTOs");
    assert!(!json.contains(HOST_ROOT), "no host path in renderer DTO");
    for forbidden in ["hostPath", "containerPath", "filePath", "downloadRoot"] {
        assert!(!json.contains(forbidden), "no path field: {forbidden}");
    }
    // Positive control: the same root is present in the internal argv only.
    assert!(probe_args("/downloads/e01.mkv", HOST_ROOT)
        .iter()
        .any(|arg| arg.contains(HOST_ROOT)));
}

#[test]
fn completion_inspection_aggregates_and_fails_closed() {
    let paths = vec![
        "/downloads/e01.mkv".to_string(),
        "/downloads/e02.mkv".to_string(),
    ];
    let results = inspect_completed(&accepting(), &paths);
    assert_eq!(results.len(), 2);
    assert_eq!(combine_inspection(&results), InspectionState::Clean);

    let refusing = FakeProbe {
        result: Ok(ProbeOutput {
            success: false,
            stdout: String::new(),
        }),
    };
    let mixed = vec![
        ("/downloads/e01.mkv".to_string(), InspectionState::Clean),
        (
            "/downloads/e02.mkv".to_string(),
            inspect_with_runner(&refusing, "/downloads/e02.mkv"),
        ),
    ];
    assert_eq!(combine_inspection(&mixed), InspectionState::Suspicious);
    assert_eq!(
        combine_inspection(&[]),
        InspectionState::Unavailable,
        "no files is never Clean"
    );
}
