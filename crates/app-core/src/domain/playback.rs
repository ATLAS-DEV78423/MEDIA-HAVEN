//! Pure playback decisions: completion and resume.
//!
//! Kept free of any player engine so the rules can be unit-tested without a
//! runtime. Thresholds are configurable with documented defaults; nothing here
//! performs I/O or mutates state.

/// Configurable thresholds for completion and resume decisions.
#[derive(Debug, Clone, Copy)]
pub struct PlaybackConfig {
    /// Minimum watched seconds before completion is even considered.
    pub min_watched_seconds: f64,
    /// Fraction of total duration that must be watched.
    pub watched_fraction: f64,
    /// Position within this many seconds of the end counts as "near end".
    pub near_end_seconds: f64,
    /// Do not resume below this position (avoids resuming at 3 seconds).
    pub resume_min_seconds: f64,
    /// Do not resume within this many seconds of the end (avoids credits).
    pub resume_max_tail_seconds: f64,
}

impl Default for PlaybackConfig {
    fn default() -> Self {
        Self {
            min_watched_seconds: 60.0,
            watched_fraction: 0.9,
            near_end_seconds: 30.0,
            resume_min_seconds: 10.0,
            resume_max_tail_seconds: 30.0,
        }
    }
}

/// Decide whether an item is "watched"/complete.
///
/// Completion requires ALL of:
/// - a normal end-of-file event (`ended_normally`),
/// - not a user stop (`!user_stopped`),
/// - watched duration >= `min_watched_seconds`,
/// - watched duration / duration >= `watched_fraction`,
/// - position within `near_end_seconds` of the end.
///
/// Seeking to the end or closing mid-episode never completes.
pub fn is_complete(
    watched_seconds: f64,
    duration: f64,
    position: f64,
    ended_normally: bool,
    user_stopped: bool,
    cfg: &PlaybackConfig,
) -> bool {
    if !ended_normally || user_stopped {
        return false;
    }
    if duration <= 0.0 || watched_seconds < cfg.min_watched_seconds {
        return false;
    }
    if watched_seconds / duration < cfg.watched_fraction {
        return false;
    }
    (duration - position).abs() <= cfg.near_end_seconds
}

/// Decide the resume position, or `None` if resuming is not sensible.
pub fn resume_position(position: f64, duration: f64, cfg: &PlaybackConfig) -> Option<f64> {
    if position < cfg.resume_min_seconds {
        return None;
    }
    if duration > 0.0 && (duration - position) <= cfg.resume_max_tail_seconds {
        return None;
    }
    Some(position)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seek_to_end_does_not_complete() {
        let cfg = PlaybackConfig::default();
        // ended_normally false (seek, not EOF) -> not complete
        assert!(!is_complete(3600.0, 3600.0, 3600.0, false, false, &cfg));
    }

    #[test]
    fn close_mid_episode_does_not_complete() {
        let cfg = PlaybackConfig::default();
        assert!(!is_complete(120.0, 3600.0, 120.0, false, true, &cfg));
    }

    #[test]
    fn normal_long_watch_completes() {
        let cfg = PlaybackConfig::default();
        assert!(is_complete(3300.0, 3600.0, 3590.0, true, false, &cfg));
    }

    #[test]
    fn short_watch_does_not_complete() {
        let cfg = PlaybackConfig::default();
        assert!(!is_complete(30.0, 3600.0, 30.0, true, false, &cfg));
    }

    #[test]
    fn resume_skips_tiny_positions() {
        let cfg = PlaybackConfig::default();
        assert_eq!(resume_position(3.0, 3600.0, &cfg), None);
    }

    #[test]
    fn resume_skips_near_end() {
        let cfg = PlaybackConfig::default();
        assert_eq!(resume_position(3590.0, 3600.0, &cfg), None);
    }

    #[test]
    fn resume_returns_position() {
        let cfg = PlaybackConfig::default();
        assert_eq!(resume_position(120.0, 3600.0, &cfg), Some(120.0));
    }
}
