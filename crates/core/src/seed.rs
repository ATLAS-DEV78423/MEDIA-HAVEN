//! Fixture data for the first-run experience.
//!
//! Only runs when `MEDIAHAVEN_SEED=1`. Inserts are `INSERT OR IGNORE`, so
//! re-running against an existing database is safe and idempotent.
//!
//! This is development data for the UI, not a library import: real titles and
//! their files arrive through `library_scan`. Titles here are chosen to
//! exercise every shelf and every screen the backend serves.

use app_core::domain::{
    AppSettings, DiagnosticsSettings, OptionalServicesSettings, PlaybackSettings, Profile,
    RuntimeStatus, StorageSettings,
};
use chrono::{Duration, Utc};
use sqlx::SqlitePool;

/// Seed a small library if `MEDIAHAVEN_SEED=1`. Otherwise this is a no-op.
pub async fn seed_if_requested(pool: &SqlitePool) -> anyhow::Result<()> {
    if std::env::var("MEDIAHAVEN_SEED").as_deref() != Ok("1") {
        return Ok(());
    }
    seed(pool).await
}

struct Movie {
    id: &'static str,
    title: &'static str,
    year: i64,
    runtime: i64,
    genres: &'static str,
    synopsis: &'static str,
    /// (position, duration) in seconds for a partially watched title.
    progress: Option<(f64, f64)>,
}

struct Series {
    id: &'static str,
    title: &'static str,
    year: i64,
    genres: &'static str,
    synopsis: &'static str,
}

struct EpisodeSeed {
    id: &'static str,
    series_id: &'static str,
    season: i64,
    number: i64,
    title: &'static str,
    runtime: i64,
    progress: Option<(f64, f64)>,
}

async fn seed(pool: &SqlitePool) -> anyhow::Result<()> {
    let now = Utc::now();
    let stamp = |days: i64| (now - Duration::days(days)).to_rfc3339();

    sqlx::query(
        "INSERT OR IGNORE INTO library_roots (id, display_name, item_count, status, last_scan_at) \
         VALUES ('root-1', 'Sample library', 0, 'ok', ?)",
    )
    .bind(stamp(0))
    .execute(pool)
    .await?;

    let movies = [
        Movie {
            id: "m1",
            title: "Arrival",
            year: 2016,
            runtime: 116,
            genres: "[\"sci-fi\",\"drama\"]",
            synopsis: "A linguist is recruited to translate an alien language before the world decides what it means.",
            progress: Some((1800.0, 6960.0)),
        },
        Movie {
            id: "m-dune",
            title: "Dune",
            year: 2021,
            runtime: 155,
            genres: "[\"sci-fi\",\"adventure\"]",
            synopsis: "Paul Atreides arrives on Arrakis with his family and a duty he did not choose. The desert decides what he becomes.",
            progress: Some((4200.0, 9300.0)),
        },
        Movie {
            id: "m-br",
            title: "Blade Runner 2049",
            year: 2017,
            runtime: 164,
            genres: "[\"sci-fi\",\"mystery\"]",
            synopsis: "A young blade runner follows a buried record to a man who has been missing for thirty years.",
            progress: None,
        },
        Movie {
            id: "m-starwars",
            title: "Star Wars: The Force Awakens",
            year: 2015,
            runtime: 138,
            genres: "[\"adventure\",\"action\"]",
            synopsis: "A scavenger, a defector and a small orange droid are the only ones who know where the last Jedi went.",
            progress: None,
        },
        Movie {
            id: "m-tomorrow",
            title: "The Tomorrow War",
            year: 2021,
            runtime: 138,
            genres: "[\"action\",\"sci-fi\"]",
            synopsis: "A biology teacher is drafted into a war thirty years from now and given one week to find what the enemy fears.",
            progress: None,
        },
    ];

    let series = [
        Series {
            id: "s1",
            title: "Severance",
            year: 2022,
            genres: "[\"thriller\",\"sci-fi\"]",
            synopsis: "Employees undergo a procedure that separates work memories from personal ones.",
        },
        Series {
            id: "s-quiet",
            title: "The Quiet Earth",
            year: 2026,
            genres: "[\"sci-fi\",\"mystery\"]",
            synopsis: "After a global silence, a scientist traces a transmission that should not exist to the edge of the sea.",
        },
        Series {
            id: "s-after",
            title: "After Hours",
            year: 2025,
            genres: "[\"drama\"]",
            synopsis: "Five night-shift workers share one long evening, one closed road and a decision none of them can take back.",
        },
    ];

    for (index, movie) in movies.iter().enumerate() {
        let created = stamp(20 - index as i64);
        let (position, duration, updated) = match movie.progress {
            Some((position, duration)) => (Some(position), Some(duration), Some(created.clone())),
            None => (None, None, None),
        };
        sqlx::query(
            "INSERT OR IGNORE INTO media_items \
             (id, kind, title, year, synopsis, runtime_minutes, genres, availability_state, \
              progress_position, progress_duration, progress_updated_at, progress_completed, \
              created_at, updated_at) \
             VALUES (?, 'movie', ?, ?, ?, ?, ?, 'available', ?, ?, ?, 0, ?, ?)",
        )
        .bind(movie.id)
        .bind(movie.title)
        .bind(movie.year)
        .bind(movie.synopsis)
        .bind(movie.runtime)
        .bind(movie.genres)
        .bind(position)
        .bind(duration)
        .bind(updated)
        .bind(&created)
        .bind(&created)
        .execute(pool)
        .await?;
    }

    for (index, show) in series.iter().enumerate() {
        // The Quiet Earth is the newest series so the home hero features it.
        let created = stamp(match show.id {
            "s-quiet" => 2,
            "s-after" => 12,
            _ => 18,
        });
        let _ = index;
        sqlx::query(
            "INSERT OR IGNORE INTO media_items \
             (id, kind, title, year, synopsis, genres, availability_state, created_at, updated_at) \
             VALUES (?, 'series', ?, ?, ?, ?, 'available', ?, ?)",
        )
        .bind(show.id)
        .bind(show.title)
        .bind(show.year)
        .bind(show.synopsis)
        .bind(show.genres)
        .bind(&created)
        .bind(&created)
        .execute(pool)
        .await?;
    }

    let episodes = [
        EpisodeSeed {
            id: "e1",
            series_id: "s1",
            season: 1,
            number: 1,
            title: "Good News About Hell",
            runtime: 52,
            progress: Some((1200.0, 3120.0)),
        },
        EpisodeSeed {
            id: "e2",
            series_id: "s1",
            season: 1,
            number: 2,
            title: "Half Loop",
            runtime: 48,
            progress: None,
        },
        EpisodeSeed {
            id: "q-s1e1",
            series_id: "s-quiet",
            season: 1,
            number: 1,
            title: "The First Morning",
            runtime: 49,
            progress: Some((900.0, 2940.0)),
        },
        EpisodeSeed {
            id: "q-s1e2",
            series_id: "s-quiet",
            season: 1,
            number: 2,
            title: "Carrier Wave",
            runtime: 46,
            progress: None,
        },
        EpisodeSeed {
            id: "q-s1e3",
            series_id: "s-quiet",
            season: 1,
            number: 3,
            title: "The Signal",
            runtime: 48,
            progress: None,
        },
        EpisodeSeed {
            id: "q-s2e1",
            series_id: "s-quiet",
            season: 2,
            number: 1,
            title: "Low Tide",
            runtime: 47,
            progress: None,
        },
        EpisodeSeed {
            id: "q-s2e2",
            series_id: "s-quiet",
            season: 2,
            number: 2,
            title: "The Listeners",
            runtime: 51,
            progress: None,
        },
        EpisodeSeed {
            id: "q-s2e3",
            series_id: "s-quiet",
            season: 2,
            number: 3,
            title: "Undertow",
            runtime: 44,
            progress: None,
        },
        EpisodeSeed {
            id: "a-s1e1",
            series_id: "s-after",
            season: 1,
            number: 1,
            title: "Clock In",
            runtime: 43,
            progress: None,
        },
        EpisodeSeed {
            id: "a-s1e2",
            series_id: "s-after",
            season: 1,
            number: 2,
            title: "Last Call",
            runtime: 41,
            progress: None,
        },
        EpisodeSeed {
            id: "a-s1e3",
            series_id: "s-after",
            season: 1,
            number: 3,
            title: "First Light",
            runtime: 46,
            progress: None,
        },
    ];

    for (index, episode) in episodes.iter().enumerate() {
        let created = stamp((18 - index.min(17)) as i64);
        let (position, duration, updated) = match episode.progress {
            Some((position, duration)) => (Some(position), Some(duration), Some(created.clone())),
            None => (None, None, None),
        };
        sqlx::query(
            "INSERT OR IGNORE INTO media_items \
             (id, kind, title, year, runtime_minutes, genres, availability_state, \
              series_id, season_number, episode_number, next_up, \
              progress_position, progress_duration, progress_updated_at, progress_completed, \
              created_at, updated_at) \
             VALUES (?, 'episode', ?, 2026, ?, '[]', 'available', ?, ?, ?, 0, ?, ?, ?, 0, ?, ?)",
        )
        .bind(episode.id)
        .bind(episode.title)
        .bind(episode.runtime)
        .bind(episode.series_id)
        .bind(episode.season)
        .bind(episode.number)
        .bind(position)
        .bind(duration)
        .bind(updated)
        .bind(&created)
        .bind(&created)
        .execute(pool)
        .await?;
    }

    for (id, media_id, position) in [
        ("w1", "m1", 0_i64),
        ("w2", "m-dune", 1),
        ("w3", "s-quiet", 2),
    ] {
        sqlx::query(
            "INSERT OR IGNORE INTO watchlist_items (id, media_id, position, added_at) \
             VALUES (?, ?, ?, ?)",
        )
        .bind(id)
        .bind(media_id)
        .bind(position)
        .bind(stamp(3))
        .execute(pool)
        .await?;
    }

    sqlx::query(
        "INSERT OR IGNORE INTO queue_items (id, media_id, position, state) \
         VALUES ('q1', 's1', 0, 'pending')",
    )
    .execute(pool)
    .await?;

    // Profiles live inside the settings document. `INSERT OR IGNORE` keeps a
    // user's own settings when one already exists.
    let settings = AppSettings {
        library_roots: vec![],
        playback: PlaybackSettings {
            preferred_language: "en".to_string(),
            subtitle_size: "medium".to_string(),
            autoplay_next: true,
        },
        storage: StorageSettings {
            root_display_name: "Media".to_string(),
            limit_bytes: None,
        },
        optional_services: OptionalServicesSettings {
            downloads_enabled: false,
            metadata_provider: None,
        },
        diagnostics: DiagnosticsSettings {
            runtime_status: RuntimeStatus::Unknown,
            backend_status: RuntimeStatus::Ready,
        },
        profiles: vec![
            Profile {
                id: "profile-alex".to_string(),
                name: "Alex".to_string(),
                avatar_color: "#7E93A8".to_string(),
                created_at: stamp(30),
            },
            Profile {
                id: "profile-sam".to_string(),
                name: "Sam".to_string(),
                avatar_color: "#A88E7E".to_string(),
                created_at: stamp(29),
            },
            Profile {
                id: "profile-kids".to_string(),
                name: "Kids".to_string(),
                avatar_color: "#8AA88A".to_string(),
                created_at: stamp(28),
            },
        ],
    };
    let encoded = serde_json::to_string(&settings)?;
    sqlx::query("INSERT OR IGNORE INTO settings (id, data) VALUES (1, ?)")
        .bind(encoded)
        .execute(pool)
        .await?;

    // The sample root has no path; count the items that came from the seed so
    // the library screen does not report an empty folder beside a full shelf.
    sqlx::query(
        "UPDATE library_roots SET item_count = \
         (SELECT COUNT(*) FROM media_items WHERE root_id IS NULL) \
         WHERE id = 'root-1'",
    )
    .execute(pool)
    .await?;

    tracing::info!("seeded fixture library");
    Ok(())
}
