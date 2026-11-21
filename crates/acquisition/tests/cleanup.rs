//! Task 6 evidence: the exclusion matrix, the never-delete-source-media
//! file-count proof, multi-episode coordination and honest reconciliation.
//! All filesystem effects are temp fixtures or fakes — no real media is touched.

use std::cell::RefCell;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use media_acquisition::{
    completion_confirmed_by_b5, is_eligible, is_managed_path, reconcile, CleanupCandidate,
    CleanupDecision, CleanupError, CleanupState, CleanupWorker, ManagedFile, NotEligibleReason,
    QbError, RemoveMode, TorrentApi, TorrentStatus, REPLAY_GRACE,
};

#[derive(Default)]
struct FakeApi {
    removes: Rc<RefCell<Vec<(String, bool)>>>,
    fail: bool,
}

impl TorrentApi for FakeApi {
    fn add(&self, _source: &str, _save_path: &str) -> Result<String, QbError> {
        Ok("h".to_string())
    }
    fn resume(&self, _hash: &str) -> Result<(), QbError> {
        Ok(())
    }
    fn status(&self, hash: &str) -> Result<TorrentStatus, QbError> {
        Ok(TorrentStatus {
            hash: hash.to_string(),
            state: "ready".to_string(),
            progress: 1.0,
        })
    }
    fn files(&self, _hash: &str) -> Result<Vec<String>, QbError> {
        Ok(vec![])
    }
    fn remove(&self, hash: &str, delete_files: bool) -> Result<(), QbError> {
        if self.fail {
            return Err(QbError::Api(500));
        }
        self.removes
            .borrow_mut()
            .push((hash.to_string(), delete_files));
        Ok(())
    }
}

/// In-memory filesystem for the coordination branches.
struct FakeFs {
    live: RefCell<HashSet<String>>,
}

impl FakeFs {
    fn new(paths: &[&str]) -> Self {
        Self {
            live: RefCell::new(paths.iter().map(|path| path.to_string()).collect()),
        }
    }
    fn exists(&self) -> impl Fn(&str) -> bool + '_ {
        |path| self.live.borrow().contains(path)
    }
    fn count(&self) -> usize {
        self.live.borrow().len()
    }
}

fn managed(path: &str, hash: &str, media: &str, episode: i64) -> ManagedFile {
    ManagedFile {
        path: path.to_string(),
        torrent_hash: hash.to_string(),
        media_id: Some(media.to_string()),
        episode_no: Some(episode),
    }
}

fn temp_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("mh-cleanup-{tag}-{nanos}"));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn seed_files(dir: &Path, count: usize) -> Vec<PathBuf> {
    (0..count)
        .map(|index| {
            let path = dir.join(format!("fixture-{index}.mkv"));
            fs::write(&path, b"harmless temp fixture").expect("write fixture");
            path
        })
        .collect()
}

fn count_files(dir: &Path) -> usize {
    fs::read_dir(dir)
        .expect("read dir")
        .filter(|entry| entry.as_ref().map(|e| e.path().is_file()).unwrap_or(false))
        .count()
}

// --- Step 1 — eligibility exclusion matrix --------------------------------

fn eligible_candidate() -> CleanupCandidate {
    CleanupCandidate {
        completion_confirmed: true,
        app_managed: true,
        completed_at: 1_000,
        now: 1_000 + REPLAY_GRACE.as_secs() as i64,
        grace_seconds: REPLAY_GRACE.as_secs() as i64,
        actively_playing: false,
        prefetch_target: false,
    }
}

#[test]
fn eligibility_matrix_admits_only_the_fully_eligible_case() {
    use NotEligibleReason::*;
    let base = eligible_candidate();
    assert_eq!(REPLAY_GRACE.as_secs(), 24 * 3600, "G-5 grace is 24 h");
    assert_eq!(is_eligible(&base), CleanupDecision::Eligible);

    let mut c = base;
    c.now = c.completed_at + c.grace_seconds;
    assert_eq!(
        is_eligible(&c),
        CleanupDecision::Eligible,
        "grace boundary inclusive"
    );

    c = base;
    c.completion_confirmed = false;
    assert_eq!(
        is_eligible(&c),
        CleanupDecision::NotEligible(CompletionNotConfirmed)
    );

    c = base;
    c.app_managed = false;
    assert_eq!(is_eligible(&c), CleanupDecision::NotEligible(NotAppManaged));

    c = base;
    c.now = c.completed_at + c.grace_seconds - 1;
    assert_eq!(
        is_eligible(&c),
        CleanupDecision::NotEligible(WithinReplayGrace)
    );

    c = base;
    c.actively_playing = true;
    assert_eq!(
        is_eligible(&c),
        CleanupDecision::NotEligible(ActivelyPlaying)
    );

    c = base;
    c.prefetch_target = true;
    assert_eq!(
        is_eligible(&c),
        CleanupDecision::NotEligible(PrefetchTarget)
    );
}

#[test]
fn worker_evaluate_delegates_to_the_same_pure_rule() {
    let worker = CleanupWorker::new(FakeApi::default());
    let base = eligible_candidate();
    assert_eq!(worker.evaluate(&base), CleanupDecision::Eligible);
    let mut playing = base;
    playing.actively_playing = true;
    assert_eq!(
        worker.evaluate(&playing),
        CleanupDecision::NotEligible(NotEligibleReason::ActivelyPlaying)
    );
}

#[test]
fn b5_adversarial_cases_never_confirm_completion() {
    // EOF-driven normal watch confirms; seeking and a user close never do.
    assert!(!completion_confirmed_by_b5(
        3600.0, 3600.0, 3600.0, false, false
    ));
    assert!(!completion_confirmed_by_b5(
        120.0, 3600.0, 120.0, false, true
    ));
    assert!(!completion_confirmed_by_b5(30.0, 3600.0, 30.0, true, false));
    assert!(completion_confirmed_by_b5(
        3300.0, 3600.0, 3590.0, true, false
    ));

    // Queue-removal / app-close do not even reach the confirm path; with no
    // confirmed completion, eligibility refuses cleanup.
    let mut c = eligible_candidate();
    c.completion_confirmed = false;
    assert_eq!(
        is_eligible(&c),
        CleanupDecision::NotEligible(NotEligibleReason::CompletionNotConfirmed)
    );
}

// --- Step 2 — never delete source media (release blocker) ------------------

#[test]
fn media_with_no_file_mapping_row_deletes_zero_files() {
    let dir = temp_dir("unmapped");
    let files = seed_files(&dir, 3);
    let before = count_files(&dir);

    let log = Rc::new(RefCell::new(Vec::new()));
    let api = FakeApi {
        removes: log.clone(),
        fail: false,
    };
    let worker = CleanupWorker::new(api);
    // A user-imported media id: no `file_mappings` row, no torrent hash.
    let target = ManagedFile {
        path: files[0].to_string_lossy().to_string(),
        torrent_hash: String::new(),
        media_id: Some("user-imported".to_string()),
        episode_no: None,
    };

    let result = worker.execute(
        &target,
        &[], // no mapping row for this media id
        &|_| false,
        |path| Path::new(path).exists(),
    );

    assert!(matches!(result, Err(CleanupError::NotAppManaged)));
    assert_eq!(count_files(&dir), before, "ZERO files may be deleted");
    assert!(
        files.iter().all(|path| path.exists()),
        "every fixture survives"
    );
    assert!(log.borrow().is_empty(), "no torrent task is touched");
    let (state, _) = reconcile(&result);
    assert_eq!(state, CleanupState::Failed, "no false 'removed' claim");

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn outside_root_paths_are_refused_before_any_deletion() {
    assert!(is_managed_path("/downloads/show/e01.mkv"));
    assert!(!is_managed_path("/media/root-0/e01.mkv"));
    assert!(!is_managed_path("/downloads/../media/root/e01.mkv"));
    assert!(!is_managed_path("/downloads"));

    let fs = FakeFs::new(&["/media/root-0/episode.mkv"]);
    let worker = CleanupWorker::new(FakeApi::default());
    let target = managed("/media/root-0/episode.mkv", "hash-1", "media-e1", 1);
    let result = worker.execute(
        &target,
        std::slice::from_ref(&target),
        &|_| false,
        fs.exists(),
    );
    assert!(matches!(result, Err(CleanupError::OutsideManagedRoot)));
    assert_eq!(
        fs.count(),
        1,
        "source media under a user root is never deleted"
    );
}

#[test]
fn unfinished_playing_and_unverified_files_are_never_eligible() {
    // Unfinished (unconfirmed), actively playing, and unverified (no mapping)
    // are each refused by the single shared rule.
    let mut c = eligible_candidate();
    c.completion_confirmed = false;
    assert_ne!(is_eligible(&c), CleanupDecision::Eligible);
    c = eligible_candidate();
    c.actively_playing = true;
    assert_ne!(is_eligible(&c), CleanupDecision::Eligible);
    c = eligible_candidate();
    c.app_managed = false;
    assert_ne!(is_eligible(&c), CleanupDecision::Eligible);
}

// --- Step 3 — multi-episode coordination -----------------------------------

#[test]
fn multi_episode_torrent_keeps_all_files_when_a_sibling_is_needed() {
    // core has only :ro /downloads: it cannot delete the orphaned file itself.
    // With a still-needed sibling the whole torrent is kept (qB task removed,
    // files untouched), so ZERO files are deleted and no removal is claimed.
    let fs = FakeFs::new(&["/downloads/show/e01.mkv", "/downloads/show/e02.mkv"]);
    let log = Rc::new(RefCell::new(Vec::new()));
    let worker = CleanupWorker::new(FakeApi {
        removes: log.clone(),
        fail: false,
    });
    let target = managed("/downloads/show/e01.mkv", "hash-1", "media-e1", 1);
    let sibling = managed("/downloads/show/e02.mkv", "hash-1", "media-e2", 2);

    let still_needed: HashSet<&str> = ["media-e2"].into_iter().collect();
    let result = worker
        .execute(
            &target,
            &[target.clone(), sibling.clone()],
            &|file| {
                file.media_id
                    .as_deref()
                    .map(|id| still_needed.contains(id))
                    .unwrap_or(false)
            },
            fs.exists(),
        )
        .expect("keep-files branch succeeds");

    assert_eq!(result.mode, RemoveMode::KeepFiles);
    assert_eq!(result.removed_files, 0, "core cannot delete files");
    assert_eq!(*log.borrow(), vec![("hash-1".to_string(), false)]);
    assert_eq!(fs.count(), 2, "both files survive: no data loss");
    let (state, outcome) = reconcile(&Ok(result));
    assert_eq!(
        state,
        CleanupState::Failed,
        "zero deletion is not 'removed'"
    );
    assert!(!outcome.contains("removed 1"), "outcome: {outcome}");
}

#[test]
fn fully_orphaned_torrent_is_removed_with_delete_files() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let worker = CleanupWorker::new(FakeApi {
        removes: log.clone(),
        fail: false,
    });
    let target = managed("/downloads/show/e01.mkv", "hash-1", "media-e1", 1);

    // qB owns deletion; `exists` reports what it observed (all files gone).
    let result = worker
        .execute(
            &target,
            std::slice::from_ref(&target),
            &|_| false,
            |_path: &str| false,
        )
        .expect("delete-files branch succeeds");

    assert_eq!(result.mode, RemoveMode::DeleteFiles);
    assert_eq!(result.removed_files, 1, "qB's deletion is counted");
    assert_eq!(*log.borrow(), vec![("hash-1".to_string(), true)]);
    let (state, outcome) = reconcile(&Ok(result));
    assert_eq!(state, CleanupState::Removed);
    assert!(outcome.contains("removed 1 file"), "outcome: {outcome}");
}

#[test]
fn null_media_id_sibling_is_always_kept_and_forces_keep_files() {
    // A sibling row with media_id = NULL (allowed by 0004_acquisition.sql:21)
    // is unidentified. Even a caller that claims nothing is needed must not
    // let qB delete it, nor may our KeepFiles loop delete it locally.
    let fs = FakeFs::new(&["/downloads/show/e01.mkv", "/downloads/show/e02.mkv"]);
    let log = Rc::new(RefCell::new(Vec::new()));
    let worker = CleanupWorker::new(FakeApi {
        removes: log.clone(),
        fail: false,
    });
    let target = managed("/downloads/show/e01.mkv", "hash-1", "media-e1", 1);
    let mut unidentified = managed("/downloads/show/e02.mkv", "hash-1", "media-e2", 2);
    unidentified.media_id = None; // NULL in the database

    let claim_nothing_needed = |_file: &ManagedFile| false;
    assert_eq!(
        media_acquisition::removal_mode(
            &target,
            &[target.clone(), unidentified.clone()],
            &claim_nothing_needed
        ),
        RemoveMode::KeepFiles,
        "a NULL-media_id sibling must force deleteFiles=false"
    );
    assert!(media_acquisition::is_still_needed(
        &unidentified,
        &claim_nothing_needed
    ));

    let result = worker
        .execute(
            &target,
            &[target.clone(), unidentified.clone()],
            &claim_nothing_needed,
            fs.exists(),
        )
        .expect("keep-files branch succeeds");

    assert_eq!(result.mode, RemoveMode::KeepFiles);
    assert_eq!(
        result.removed_files, 0,
        "no local file deletion is possible"
    );
    assert_eq!(*log.borrow(), vec![("hash-1".to_string(), false)]);
    assert!(
        fs.exists()("/downloads/show/e01.mkv"),
        "the identified target's file also survives"
    );
    assert!(
        fs.exists()("/downloads/show/e02.mkv"),
        "the unidentified sibling's file MUST survive"
    );
}

// --- Step 4 — reconciliation: failure is never 'removed' -------------------

#[test]
fn zero_removal_never_reconciles_to_removed() {
    // AC-10 regression: a still-needed sibling forces KeepFiles. qB drops the
    // task but no file is deleted, so the outcome must NOT be `removed` even
    // though the worker returned Ok — there is no false "removed 0 file(s)".
    let fs = FakeFs::new(&["/downloads/show/e01.mkv", "/downloads/show/e02.mkv"]);
    let worker = CleanupWorker::new(FakeApi::default());
    let target = managed("/downloads/show/e01.mkv", "hash-1", "media-e1", 1);
    let sibling = managed("/downloads/show/e02.mkv", "hash-1", "media-e2", 2);
    let still_needed: HashSet<&str> = ["media-e2"].into_iter().collect();

    let result = worker.execute(
        &target,
        &[target.clone(), sibling],
        &|file| {
            file.media_id
                .as_deref()
                .map(|id| still_needed.contains(id))
                .unwrap_or(false)
        },
        fs.exists(),
    );

    assert!(result.is_ok());
    assert_eq!(result.as_ref().expect("ok").removed_files, 0);
    let (state, outcome) = reconcile(&result);
    assert_eq!(state, CleanupState::Failed);
    assert!(!outcome.contains("removed"), "outcome: {outcome}");
    assert_eq!(fs.count(), 2, "no file was deleted");
}

#[test]
fn torrent_remove_failure_reconciles_to_failed() {
    let fs = FakeFs::new(&["/downloads/show/e01.mkv"]);
    let worker = CleanupWorker::new(FakeApi {
        removes: Rc::new(RefCell::new(Vec::new())),
        fail: true,
    });
    let target = managed("/downloads/show/e01.mkv", "hash-1", "media-e1", 1);
    let result = worker.execute(
        &target,
        std::slice::from_ref(&target),
        &|_| false,
        fs.exists(),
    );
    assert!(matches!(result, Err(CleanupError::TorrentRemoveFailed)));
    let (state, _) = reconcile(&result);
    assert_eq!(state, CleanupState::Failed);
}

#[test]
fn unverified_deletion_never_marks_removed() {
    let fs = FakeFs::new(&["/downloads/show/e01.mkv"]);
    let worker = CleanupWorker::new(FakeApi::default());
    let target = managed("/downloads/show/e01.mkv", "hash-1", "media-e1", 1);

    // qB claims success but the file still exists => refuse to claim removed.
    let result = worker.execute(
        &target,
        std::slice::from_ref(&target),
        &|_| false,
        |_path| true,
    );
    assert!(matches!(result, Err(CleanupError::FileStillPresent)));
    let (state, _) = reconcile(&result);
    assert_eq!(state, CleanupState::Failed);
    assert_eq!(fs.count(), 1);
}

#[test]
fn delete_files_branch_requires_every_torrent_file_gone() {
    // qB reported success but only deleted the target; a second mapping is
    // still on disk. Verification must fail closed instead of claiming removed.
    let log = Rc::new(RefCell::new(Vec::new()));
    let worker = CleanupWorker::new(FakeApi {
        removes: log.clone(),
        fail: false,
    });
    let target = managed("/downloads/show/e01.mkv", "hash-1", "media-e1", 1);
    let sibling = managed("/downloads/show/e02.mkv", "hash-1", "media-e2", 2);

    let result = worker.execute(
        &target,
        &[target.clone(), sibling],
        &|_| false,
        |path: &str| path != "/downloads/show/e01.mkv", // sibling still present
    );

    assert!(matches!(result, Err(CleanupError::FileStillPresent)));
    let (state, _) = reconcile(&result);
    assert_eq!(state, CleanupState::Failed);
    assert_eq!(*log.borrow(), vec![("hash-1".to_string(), true)]);
}

#[test]
fn successful_cleanup_reconciles_to_removed_with_a_visible_outcome() {
    // A single-file torrent with a verified qB deletion is a real removal.
    let worker = CleanupWorker::new(FakeApi::default());
    let target = managed("/downloads/show/e01.mkv", "hash-1", "media-e1", 1);
    let result = worker
        .execute(
            &target,
            std::slice::from_ref(&target),
            &|_| false,
            |_path: &str| false, // qB removed the file
        )
        .expect("success");
    let (state, outcome) = reconcile(&Ok(result));
    assert_eq!(state, CleanupState::Removed);
    assert!(outcome.contains("removed 1 file"), "outcome: {outcome}");
}
