//! Rolling two-episode buffer (spec §9, Task 5).
//!
//! All decisions here are pure and synchronous: ordering, skip-existing and the
//! G-8 quota check. The backend owns I/O and policy (ADR-0002). An enqueue
//! failure is logged and degraded here; it can never fail a playback session.

use app_core::domain::DownloadItem;
pub use app_core::domain::{
    is_buffer_paused, BUFFER_PAUSED_NOTICE as BUFFER_PAUSED_MESSAGE, DEFAULT_BUFFER_LIMIT_BYTES,
};

use crate::orchestrator::AcquireError;

/// A library episode identity, no filesystem path. `ordered` must be sorted by
/// (season, episode) — the order SQLite returns from `media_items`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpisodeRef {
    pub media_id: String,
    pub season: i64,
    pub episode: i64,
}

/// The next two episodes after `current_id`, stopping at the season boundary
/// and at the final episode (spec §9 open-question default). An unknown id
/// yields nothing — never guess an ordering.
pub fn next_two_in_season(ordered: &[EpisodeRef], current_id: &str) -> Vec<EpisodeRef> {
    let Some(index) = ordered.iter().position(|ep| ep.media_id == current_id) else {
        return Vec::new();
    };
    let season = ordered[index].season;
    ordered[index + 1..]
        .iter()
        .take_while(|ep| ep.season == season)
        .take(2)
        .cloned()
        .collect()
}

/// Upcoming episodes that still need acquisition: ready/downloading episodes
/// are reused, only missing ones are enqueued (spec §9 steps 2–4, R-11).
pub fn missing_episodes(upcoming: &[EpisodeRef], available: &[String]) -> Vec<String> {
    upcoming
        .iter()
        .filter(|ep| !available.iter().any(|id| id == &ep.media_id))
        .map(|ep| ep.media_id.clone())
        .collect()
}

/// Host-internal Downloads status message, or `None` while the buffer runs.
/// Contains no path or credential and never enters a renderer DTO directly.
pub fn buffer_status(used_bytes: u64, limit_bytes: u64) -> Option<&'static str> {
    if is_buffer_paused(used_bytes, limit_bytes) {
        Some(BUFFER_PAUSED_MESSAGE)
    } else {
        None
    }
}

/// Applies a pure plan through an enqueue closure. Enqueue failures are logged
/// and skipped: a failed prefetch must never fail playback (plan risk A-6).
pub struct PrefetchScheduler<F> {
    enqueue: F,
    limit_bytes: u64,
}

impl<F> PrefetchScheduler<F>
where
    F: Fn(&str) -> Result<DownloadItem, AcquireError>,
{
    pub fn new(enqueue: F) -> Self {
        Self {
            enqueue,
            limit_bytes: DEFAULT_BUFFER_LIMIT_BYTES,
        }
    }

    pub fn with_limit(mut self, limit_bytes: u64) -> Self {
        self.limit_bytes = limit_bytes;
        self
    }

    /// Returns only the newly enqueued items. Never returns an error.
    pub fn on_playback_start(
        &self,
        ordered: &[EpisodeRef],
        current_id: &str,
        available: &[String],
        used_bytes: u64,
    ) -> Vec<DownloadItem> {
        if is_buffer_paused(used_bytes, self.limit_bytes) {
            eprintln!("prefetch: {BUFFER_PAUSED_MESSAGE}");
            return Vec::new();
        }
        let upcoming = next_two_in_season(ordered, current_id);
        let mut queued = Vec::new();
        for media_id in missing_episodes(&upcoming, available) {
            match (self.enqueue)(&media_id) {
                Ok(item) => queued.push(item),
                Err(error) => {
                    eprintln!("prefetch: enqueue for {media_id} failed ({error}); degrading");
                }
            }
        }
        queued
    }
}
