//! Task 5 evidence: ordering, skip-existing, quota pause and graceful degrade.
//! Pure logic only — no SQLite, no qBittorrent, no live source needed.

use std::cell::Cell;

use app_core::domain::{buffer_pause_notice, DownloadItem, DownloadState};
use media_acquisition::{
    buffer_status, is_buffer_paused, missing_episodes, next_two_in_season, AcquireError,
    EpisodeRef, PrefetchScheduler, BUFFER_PAUSED_MESSAGE, DEFAULT_BUFFER_LIMIT_BYTES,
};

fn ep(id: &str, season: i64, episode: i64) -> EpisodeRef {
    EpisodeRef {
        media_id: id.to_string(),
        season,
        episode,
    }
}

fn series() -> Vec<EpisodeRef> {
    vec![
        ep("s1e1", 1, 1),
        ep("s1e2", 1, 2),
        ep("s1e3", 1, 3),
        ep("s2e1", 2, 1),
        ep("s2e2", 2, 2),
    ]
}

fn item(id: &str) -> DownloadItem {
    DownloadItem {
        id: id.to_string(),
        torrent_hash: format!("hash-{id}"),
        media_id: Some(id.to_string()),
        state: DownloadState::Downloading,
        progress: 0.0,
    }
}

// Step 1 — pure ordering.

#[test]
fn middle_of_season_returns_exactly_two_upcoming_episodes() {
    let five = vec![
        ep("e1", 1, 1),
        ep("e2", 1, 2),
        ep("e3", 1, 3),
        ep("e4", 1, 4),
    ];
    assert_eq!(
        next_two_in_season(&five, "e1"),
        vec![ep("e2", 1, 2), ep("e3", 1, 3)]
    );
}

#[test]
fn only_one_episode_left_in_season_returns_one() {
    assert_eq!(
        next_two_in_season(&series(), "s1e2"),
        vec![ep("s1e3", 1, 3)]
    );
}

#[test]
fn final_episode_returns_none() {
    assert!(next_two_in_season(&series(), "s2e2").is_empty());
}

#[test]
fn season_boundary_is_not_crossed() {
    // Last episode of season 1 must not pull in season 2 episodes.
    assert!(next_two_in_season(&series(), "s1e3").is_empty());
    // A single next episode in the same season is still returned.
    assert_eq!(
        next_two_in_season(&series(), "s2e1"),
        vec![ep("s2e2", 2, 2)]
    );
}

#[test]
fn unknown_media_id_yields_no_ordering() {
    assert!(next_two_in_season(&series(), "movie-42").is_empty());
    assert!(next_two_in_season(&[], "s1e1").is_empty());
}

// Step 2 — skip existing.

#[test]
fn already_ready_episode_is_not_enqueued_again() {
    let calls = Cell::new(0);
    let scheduler = PrefetchScheduler::new(|id: &str| {
        calls.set(calls.get() + 1);
        Ok(item(id))
    });

    // From e1 the next two are e2 and e3; e2 is already available.
    let ordered = vec![ep("e1", 1, 1), ep("e2", 1, 2), ep("e3", 1, 3)];
    let available = vec!["e2".to_string()];
    let queued = scheduler.on_playback_start(&ordered, "e1", &available, 0);

    assert_eq!(calls.get(), 1, "only the missing episode is acquired");
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].media_id.as_deref(), Some("e3"));
}

#[test]
fn missing_episodes_filters_only_ready_and_downloading() {
    let upcoming = vec![ep("a", 1, 1), ep("b", 1, 2), ep("c", 1, 3)];
    let available = vec!["a".to_string(), "c".to_string()];
    assert_eq!(missing_episodes(&upcoming, &available), vec!["b"]);
}

// Step 3 — prefetch failure degrades; playback never fails.

#[test]
fn failed_enqueue_degrades_without_propagating_an_error() {
    let calls = Cell::new(0);
    let scheduler = PrefetchScheduler::new(|_id: &str| {
        calls.set(calls.get() + 1);
        Err(AcquireError::Backend)
    });
    let ordered = vec![ep("e1", 1, 1), ep("e2", 1, 2), ep("e3", 1, 3)];

    // `on_playback_start` returns no error type at all: a failed backend can
    // never fail the caller (the playback route). Assert the closure really ran
    // and errored, so this test cannot pass by never attempting an enqueue.
    let queued = scheduler.on_playback_start(&ordered, "e1", &[], 0);
    assert_eq!(
        calls.get(),
        2,
        "both missing episodes reached the enqueue closure"
    );
    assert!(queued.is_empty());
}

// Step 4 — G-8 storage cap.

#[test]
fn buffer_pauses_at_ninety_percent_of_the_limit() {
    let limit = DEFAULT_BUFFER_LIMIT_BYTES;
    assert!(!is_buffer_paused(limit * 89 / 100, limit));
    assert!(is_buffer_paused(limit * 90 / 100, limit));
    assert!(is_buffer_paused(limit, limit));
    assert!(is_buffer_paused(0, 0), "a zero limit fails closed");
    assert_eq!(buffer_status(limit, limit), Some(BUFFER_PAUSED_MESSAGE));
    assert_eq!(buffer_status(0, limit), None);

    // The Downloads DTO notice uses the same predicate as the enqueue decision.
    assert_eq!(
        buffer_pause_notice(limit as i64, Some(limit as i64)).as_deref(),
        Some(BUFFER_PAUSED_MESSAGE)
    );
    assert_eq!(buffer_pause_notice(0, Some(limit as i64)), None);
    // A missing configured cap falls back to the 20 GiB default.
    assert!(buffer_pause_notice(18 * 1024 * 1024 * 1024, None).is_some());
}

#[test]
fn paused_buffer_skips_all_enqueues() {
    let calls = Cell::new(0);
    let scheduler = PrefetchScheduler::new(|id: &str| {
        calls.set(calls.get() + 1);
        Ok(item(id))
    })
    .with_limit(100);
    let ordered = vec![ep("e1", 1, 1), ep("e2", 1, 2), ep("e3", 1, 3)];

    let queued = scheduler.on_playback_start(&ordered, "e1", &[], 90);
    assert!(queued.is_empty());
    assert_eq!(
        calls.get(),
        0,
        "no enqueue happens once the buffer is paused"
    );
}
