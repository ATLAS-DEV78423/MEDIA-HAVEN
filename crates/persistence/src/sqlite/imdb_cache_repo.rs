//! SQLite adapter for the opt-in IMDb/OMDb age-rating cache.
//!
//! Holds only SQL and state persistence: 30-day TTL, byte cap, oldest-first
//! eviction. No HTTP, no policy, no key storage. `fetched_at` is RFC3339 UTC.

use super::pool::connect_path as pool_connect_path;
use app_core::domain::{AppError, AppErrorCode};
use chrono::DateTime;
use sqlx::{Row, SqlitePool};
use std::path::Path;

/// 30 days, matching the metadata crate's filesystem cache TTL.
pub const IMDB_CACHE_TTL_SECONDS: i64 = 30 * 24 * 60 * 60;

#[derive(Clone)]
pub struct ImdbCacheRepository {
    pool: SqlitePool,
}

impl ImdbCacheRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn connect_path(path: &Path) -> Result<Self, sqlx::Error> {
        Ok(Self::new(pool_connect_path(path).await?))
    }

    fn internal(err: sqlx::Error) -> AppError {
        AppError::new(AppErrorCode::Internal, err.to_string(), true)
    }

    /// Fresh payload for `id`, or `None` on a miss, stale entry or bad date.
    /// `now` is an RFC3339 timestamp supplied by the caller (testable clock).
    pub async fn get(
        &self,
        id: &str,
        now: &str,
        ttl_seconds: i64,
    ) -> Result<Option<String>, AppError> {
        let row = sqlx::query("SELECT payload, fetched_at FROM imdb_cache WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(Self::internal)?;
        let Some(row) = row else {
            return Ok(None);
        };
        let fetched: String = row.get("fetched_at");
        let (Ok(fetched_at), Ok(now_at)) = (
            DateTime::parse_from_rfc3339(&fetched),
            DateTime::parse_from_rfc3339(now),
        ) else {
            return Ok(None);
        };
        let age = now_at.signed_duration_since(fetched_at).num_seconds();
        if age < 0 || age >= ttl_seconds {
            return Ok(None);
        }
        Ok(Some(row.get("payload")))
    }

    /// Upsert one entry, then evict oldest rows until the byte cap is met.
    pub async fn put(
        &self,
        id: &str,
        payload: &str,
        fetched_at: &str,
        source: &str,
        max_bytes: i64,
    ) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO imdb_cache (id, payload, fetched_at, source) VALUES (?, ?, ?, ?) \
             ON CONFLICT(id) DO UPDATE SET payload = excluded.payload, \
             fetched_at = excluded.fetched_at, source = excluded.source",
        )
        .bind(id)
        .bind(payload)
        .bind(fetched_at)
        .bind(source)
        .execute(&self.pool)
        .await
        .map_err(Self::internal)?;
        self.enforce_cap(max_bytes).await
    }

    async fn enforce_cap(&self, max_bytes: i64) -> Result<(), AppError> {
        let total: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(LENGTH(CAST(payload AS BLOB))), 0) FROM imdb_cache",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(Self::internal)?;
        if total <= max_bytes {
            return Ok(());
        }
        let mut excess = total - max_bytes;
        let rows = sqlx::query(
            "SELECT id, LENGTH(CAST(payload AS BLOB)) AS len FROM imdb_cache \
             ORDER BY fetched_at ASC, id ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Self::internal)?;
        for row in rows {
            if excess <= 0 {
                break;
            }
            let id: String = row.get("id");
            let len: i64 = row.get("len");
            sqlx::query("DELETE FROM imdb_cache WHERE id = ?")
                .bind(&id)
                .execute(&self.pool)
                .await
                .map_err(Self::internal)?;
            excess -= len;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use std::path::PathBuf;

    fn temp_db() -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("mh-imdb-cache-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        (dir.clone(), dir.join("cache.db"))
    }

    #[tokio::test]
    async fn fresh_entry_is_read_and_stale_entry_is_not() {
        let (dir, db) = temp_db();
        let repo = ImdbCacheRepository::connect_path(&db)
            .await
            .expect("connect and migrate");
        let now = Utc::now();
        let fresh = now.to_rfc3339();
        let stale = (now - Duration::days(31)).to_rfc3339();

        repo.put("tt0111161", "R", &fresh, "omdb", 1024)
            .await
            .expect("put fresh");
        assert_eq!(
            repo.get("tt0111161", &fresh, IMDB_CACHE_TTL_SECONDS)
                .await
                .expect("get fresh"),
            Some("R".to_string())
        );
        assert_eq!(
            repo.get("tt0111161", &stale, IMDB_CACHE_TTL_SECONDS)
                .await
                .expect("get stale"),
            None
        );
        assert_eq!(
            repo.get("tt0000000", &fresh, IMDB_CACHE_TTL_SECONDS)
                .await
                .expect("get unknown"),
            None
        );

        repo.pool.close().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn byte_cap_evicts_oldest_first() {
        let (dir, db) = temp_db();
        let repo = ImdbCacheRepository::connect_path(&db)
            .await
            .expect("connect and migrate");
        let now = Utc::now();
        // Three 4-byte payloads, cap 8 -> exactly two survive.
        for (id, when) in [
            ("tt0000001", now - Duration::minutes(3)),
            ("tt0000002", now - Duration::minutes(2)),
            ("tt0000003", now - Duration::minutes(1)),
        ] {
            repo.put(id, "PG13", &when.to_rfc3339(), "omdb", 8)
                .await
                .expect("put");
        }

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM imdb_cache")
            .fetch_one(&repo.pool)
            .await
            .expect("count");
        assert_eq!(count, 2);
        let oldest: Option<String> =
            sqlx::query_scalar("SELECT id FROM imdb_cache WHERE id = 'tt0000001'")
                .fetch_optional(&repo.pool)
                .await
                .expect("oldest query");
        assert!(oldest.is_none(), "oldest row should be evicted first");

        repo.pool.close().await;
        std::fs::remove_dir_all(&dir).ok();
    }
}
