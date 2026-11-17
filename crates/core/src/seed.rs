//! Fixture data for the first-run experience.
//!
//! Only runs when `MEDIAHAVEN_SEED=1`. Inserts are `INSERT OR IGNORE`, so
//! re-running against an existing database is safe and idempotent.

use chrono::Utc;
use sqlx::SqlitePool;

/// Seed a small library if `MEDIAHAVEN_SEED=1`. Otherwise this is a no-op.
pub async fn seed_if_requested(pool: &SqlitePool) -> anyhow::Result<()> {
    if std::env::var("MEDIAHAVEN_SEED").as_deref() != Ok("1") {
        return Ok(());
    }
    seed(pool).await
}

async fn seed(pool: &SqlitePool) -> anyhow::Result<()> {
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT OR IGNORE INTO library_roots (id, display_name, item_count, status, last_scan_at) \
         VALUES ('root-1', 'Movies', 3, 'ok', ?)",
    )
    .bind(&now)
    .execute(pool)
    .await?;

    // One movie, partially watched so it lands in "Continue Watching".
    sqlx::query(
        "INSERT OR IGNORE INTO media_items \
         (id, kind, title, year, synopsis, runtime_minutes, genres, availability_state, \
          progress_position, progress_duration, progress_updated_at, progress_completed, \
          created_at, updated_at) \
         VALUES ('m1', 'movie', 'Arrival', 2016, 'A linguist is recruited to translate alien language.', \
                 116, '[\"sci-fi\",\"drama\"]', 'available', 1800.0, 6960.0, ?, 0, ?, ?)",
    )
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    // One series with two episodes.
    sqlx::query(
        "INSERT OR IGNORE INTO media_items \
         (id, kind, title, year, synopsis, genres, availability_state, created_at, updated_at) \
         VALUES ('s1', 'series', 'Severance', 2022, 'Employees undergo a procedure to separate work and personal memories.', \
                 '[\"thriller\",\"sci-fi\"]', 'available', ?, ?)",
    )
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    for (id, number, title, runtime, position, completed) in
        [("e1", 1_i64, "Good News About Hell", 52_i64, Some(1200.0_f64), 0_i64),
         ("e2", 2_i64, "Half Loop", 48_i64, None, 0_i64)]
    {
        sqlx::query(
            "INSERT OR IGNORE INTO media_items \
             (id, kind, title, year, runtime_minutes, genres, availability_state, \
              series_id, season_number, episode_number, next_up, \
              progress_position, progress_duration, progress_updated_at, progress_completed, \
              created_at, updated_at) \
             VALUES (?, 'episode', ?, 2022, ?, '[]', 'available', 's1', 1, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(title)
        .bind(runtime)
        .bind(number)
        .bind(if number == 1 { 1_i64 } else { 0_i64 })
        .bind(position)
        .bind(position.map(|_| runtime as f64 * 60.0))
        .bind(if position.is_some() { Some(now.clone()) } else { None })
        .bind(completed)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await?;
    }

    // A watchlist entry and a queue entry so both screens are non-empty.
    sqlx::query(
        "INSERT OR IGNORE INTO watchlist_items (id, media_id, position, added_at) \
         VALUES ('w1', 'm1', 0, ?)",
    )
    .bind(&now)
    .execute(pool)
    .await?;

    sqlx::query(
        "INSERT OR IGNORE INTO queue_items (id, media_id, position, state) \
         VALUES ('q1', 's1', 0, 'pending')",
    )
    .execute(pool)
    .await?;

    tracing::info!("seeded fixture library");
    Ok(())
}
