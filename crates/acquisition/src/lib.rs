//! Acquisition subsystem: a loopback-only qBittorrent WebUI client (ADR-0018).
//!
//! All results are path-free container identities. Host path resolution stays
//! in the desktop host's A9 mount map; no path and no credential reaches a
//! renderer DTO.

pub mod cleanup;
pub mod defender;
pub mod executables;
pub mod inspection;
pub mod orchestrator;
pub mod prefetch;
pub mod qbittorrent;

pub use cleanup::{
    completion_confirmed_by_b5, is_eligible, is_managed_path, is_still_needed, reconcile,
    removal_mode, CleanupCandidate, CleanupDecision, CleanupError, CleanupOutcome, CleanupState,
    CleanupWorker, ManagedFile, NotEligibleReason, RemoveMode, REPLAY_GRACE,
};
pub use defender::{
    both_gates_clean, defender_scan, defender_scan_with, refusal_reason, DefenderState,
    MpCmdRunScanner, Scanner,
};
pub use executables::{has_executable_file, EXECUTABLE_EXTENSIONS};
pub use inspection::{
    inspect_with_probe, inspect_with_runner, probe_args, DockerProbeRunner, InspectionState,
    ProbeError, ProbeOutput, ProbeRunner, DOWNLOADS_PREFIX, FFPROBE_PATH, PROBE_IMAGE,
};
pub use orchestrator::{
    validate_source, AcquireError, AcquisitionState, DownloadStore, InMemoryDownloadStore,
    Orchestrator, SAVE_PATH,
};
pub use prefetch::{
    buffer_status, is_buffer_paused, missing_episodes, next_two_in_season, EpisodeRef,
    PrefetchScheduler, BUFFER_PAUSED_MESSAGE, DEFAULT_BUFFER_LIMIT_BYTES,
};
pub use qbittorrent::{
    QbClient, QbError, TorrentApi, TorrentStatus, DEFAULT_BASE, DEFAULT_USERNAME,
};
