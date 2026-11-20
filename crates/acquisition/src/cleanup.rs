//! Coordinated cleanup of watched, app-managed media (Task 6, ADR-0019).
//!
//! Policy and coordination only. Every path here is a host-internal container
//! path used by `core` to delete an app-acquired file; no path ever enters a
//! renderer DTO. `core` has only a read-only `/downloads` bind, so this module
//! never deletes a file itself: deletion is delegated to qBittorrent through
//! `TorrentApi::remove(hash, true)`, which owns the read/write bind. The
//! injected `exists` closure is used only to VERIFY what qB did, and keeps the
//! data-loss tests local.

use std::time::Duration;

use app_core::domain::playback::{is_complete, PlaybackConfig};

use crate::orchestrator::SAVE_PATH;
use crate::qbittorrent::TorrentApi;

/// G-5 replay grace: a confirmed completion is not cleaned up for 24 h.
pub const REPLAY_GRACE: Duration = Duration::from_secs(24 * 60 * 60);

/// Why an item was not eligible. Kept closed so callers cannot guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotEligibleReason {
    CompletionNotConfirmed,
    NotAppManaged,
    OutsideManagedRoot,
    WithinReplayGrace,
    ActivelyPlaying,
    PrefetchTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupDecision {
    Eligible,
    NotEligible(NotEligibleReason),
}

/// One app-managed file. `path` is a container path (host-internal), never a
/// renderer DTO field. Comes from a `file_mappings` row with a torrent hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedFile {
    pub path: String,
    pub torrent_hash: String,
    pub media_id: Option<String>,
    pub episode_no: Option<i64>,
}

/// Pure input to the eligibility rule. Timestamps are Unix seconds so the
/// decision is deterministic and testable without a clock.
#[derive(Debug, Clone, Copy)]
pub struct CleanupCandidate {
    pub completion_confirmed: bool,
    pub app_managed: bool,
    pub completed_at: i64,
    pub now: i64,
    pub grace_seconds: i64,
    pub actively_playing: bool,
    pub prefetch_target: bool,
}

/// Eligible iff completion is confirmed by B5, the item is app-managed, the
/// replay grace has elapsed, it is not playing and it is not a pending prefetch
/// target (ADR-0019 §2). Order matters for the diagnostic reason, not the
/// outcome: any single failing signal is enough to refuse deletion.
pub fn is_eligible(candidate: &CleanupCandidate) -> CleanupDecision {
    use NotEligibleReason::*;
    if !candidate.completion_confirmed {
        return CleanupDecision::NotEligible(CompletionNotConfirmed);
    }
    if !candidate.app_managed {
        return CleanupDecision::NotEligible(NotAppManaged);
    }
    if candidate.now.saturating_sub(candidate.completed_at).max(0) < candidate.grace_seconds {
        return CleanupDecision::NotEligible(WithinReplayGrace);
    }
    if candidate.actively_playing {
        return CleanupDecision::NotEligible(ActivelyPlaying);
    }
    if candidate.prefetch_target {
        return CleanupDecision::NotEligible(PrefetchTarget);
    }
    CleanupDecision::Eligible
}

/// Confirm completion using the existing B5 policy verbatim — do not duplicate
/// its thresholds. Seeking (no EOF) and a user stop/close never confirm.
pub fn completion_confirmed_by_b5(
    watched_seconds: f64,
    duration: f64,
    position: f64,
    ended_normally: bool,
    user_stopped: bool,
) -> bool {
    is_complete(
        watched_seconds,
        duration,
        position,
        ended_normally,
        user_stopped,
        &PlaybackConfig::default(),
    )
}

/// A container path is deletable only inside the app-managed download root.
/// Anything under a user's library root is refused here, even if a stale row
/// claims otherwise (ADR-0019 §1, "never delete source media").
pub fn is_managed_path(path: &str) -> bool {
    let prefix = format!("{SAVE_PATH}/");
    path.starts_with(&prefix) && !path.contains("..") && !path.contains('\0')
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoveMode {
    /// Another episode still needs the torrent's files: remove the task only.
    KeepFiles,
    /// No other episode needs them: qB removes the task and its files.
    DeleteFiles,
}

/// A mapping row whose `media_id` is NULL cannot be proven disposable. Fail
/// closed: treat it as still needed regardless of what the caller's predicate
/// says. `file_mappings.media_id` is nullable (migration 0004), so relying on
/// the caller alone could make qB delete an unidentified sibling's media.
pub fn is_still_needed(file: &ManagedFile, still_needed: &dyn Fn(&ManagedFile) -> bool) -> bool {
    file.media_id.is_none() || still_needed(file)
}

/// Decide qB's `deleteFiles` flag for a torrent. If any sibling file is still
/// needed, keep all qB files and delete only genuinely orphaned ones ourselves.
pub fn removal_mode(
    target: &ManagedFile,
    torrent_files: &[ManagedFile],
    still_needed: &dyn Fn(&ManagedFile) -> bool,
) -> RemoveMode {
    let other_needed = torrent_files
        .iter()
        .any(|file| file.path != target.path && is_still_needed(file, still_needed));
    if other_needed {
        RemoveMode::KeepFiles
    } else {
        RemoveMode::DeleteFiles
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CleanupError {
    #[error("media is not an app-managed download")]
    NotAppManaged,
    #[error("managed file has no torrent hash")]
    MissingTorrentHash,
    #[error("managed file is outside the download root")]
    OutsideManagedRoot,
    #[error("torrent task removal failed")]
    TorrentRemoveFailed,
    #[error("managed file deletion failed")]
    FileDeleteFailed,
    #[error("managed file is still present after deletion")]
    FileStillPresent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupOutcome {
    pub removed_files: usize,
    pub mode: RemoveMode,
}

/// The only two honest terminal states for a cleanup attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupState {
    Removed,
    Failed,
}

/// Pure reconciliation advice written to `cleanup_entries`. A failed attempt
/// can NEVER map to `Removed`, so the UI never claims a deletion that did not
/// happen (ADR-0019 §4). A successful qB call that removed ZERO files is also
/// not a removal: reporting "removed 0 file(s)" would be a false success
/// (AC-10). Only a positive, verified deletion count maps to `Removed`.
pub fn reconcile(result: &Result<CleanupOutcome, CleanupError>) -> (CleanupState, String) {
    match result {
        Ok(outcome) if outcome.removed_files > 0 => (
            CleanupState::Removed,
            format!("removed {} file(s)", outcome.removed_files),
        ),
        Ok(_) => (
            CleanupState::Failed,
            "no files deleted; kept for still-needed episodes".to_string(),
        ),
        Err(error) => (CleanupState::Failed, error.to_string()),
    }
}

/// Drives a coordinated cleanup through a fakeable `TorrentApi`.
pub struct CleanupWorker<A: TorrentApi> {
    api: A,
    grace_seconds: i64,
}

impl<A: TorrentApi> CleanupWorker<A> {
    pub fn new(api: A) -> Self {
        Self {
            api,
            grace_seconds: REPLAY_GRACE.as_secs() as i64,
        }
    }

    pub fn with_grace(mut self, grace: Duration) -> Self {
        self.grace_seconds = grace.as_secs() as i64;
        self
    }

    /// The named interface from the brief; delegates to the shared pure rule.
    pub fn evaluate(&self, candidate: &CleanupCandidate) -> CleanupDecision {
        is_eligible(candidate)
    }

    /// Execute a cleanup for one app-managed file.
    ///
    /// - `torrent_files` must be ALL `file_mappings` rows sharing the target's
    ///   hash. An empty list means the media has no mapping row: zero deletions.
    /// - `still_needed` reports whether another episode's file must be kept.
    /// - `exists` verifies what qB did. It is injected so the data-loss tests
    ///   run against fakes, never real media. There is deliberately NO deleter:
    ///   `core` is read-only and only qB can remove files (see module docs).
    pub fn execute<E>(
        &self,
        target: &ManagedFile,
        torrent_files: &[ManagedFile],
        still_needed: &dyn Fn(&ManagedFile) -> bool,
        exists: E,
    ) -> Result<CleanupOutcome, CleanupError>
    where
        E: Fn(&str) -> bool,
    {
        // Trust boundary: no mapping row, no hash, or a path outside the managed
        // root refuses the whole operation before a single file is touched.
        if !torrent_files.iter().any(|file| file.path == target.path) {
            return Err(CleanupError::NotAppManaged);
        }
        if target.torrent_hash.trim().is_empty() {
            return Err(CleanupError::MissingTorrentHash);
        }
        if torrent_files
            .iter()
            .any(|file| !is_managed_path(&file.path))
        {
            return Err(CleanupError::OutsideManagedRoot);
        }

        let mode = removal_mode(target, torrent_files, still_needed);
        match mode {
            RemoveMode::DeleteFiles => {
                // No other episode needs the torrent. qB removes the task AND
                // its files (it owns the rw bind); `core` deletes nothing.
                // Verify EVERY mapping is gone — a partial qB deletion must
                // never be reported as a removal. The count is what `reconcile`
                // requires to be positive before it reports `Removed`.
                self.api
                    .remove(&target.torrent_hash, true)
                    .map_err(|_| CleanupError::TorrentRemoveFailed)?;
                let removed = torrent_files
                    .iter()
                    .filter(|file| !exists(&file.path))
                    .count();
                if removed != torrent_files.len() {
                    return Err(CleanupError::FileStillPresent);
                }
                Ok(CleanupOutcome {
                    removed_files: removed,
                    mode,
                })
            }
            RemoveMode::KeepFiles => {
                // Another episode still needs this torrent's files. `core`
                // cannot delete individual files (read-only bind), so keep
                // them all: qB drops only the task with deleteFiles=false.
                // The shared guard still keeps NULL-media_id rows regardless of
                // what the caller claims. ZERO files are deleted here, so
                // `reconcile` can never report `Removed` for this branch.
                self.api
                    .remove(&target.torrent_hash, false)
                    .map_err(|_| CleanupError::TorrentRemoveFailed)?;
                Ok(CleanupOutcome {
                    removed_files: 0,
                    mode,
                })
            }
        }
    }
}
