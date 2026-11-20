//! SQLite adapter for coordinated cleanup (Task 6, ADR-0019).
//!
//! Holds only SQL and state persistence — no deletion policy (that lives in
//! `media-acquisition`) and no HTTP. Paths returned here are host-internal
//! container paths and never become renderer DTO fields.

use app_core::domain::{AppError, AppErrorCode, CleanupEntry, DownloadState};
use sqlx::{Row, SqlitePool};

use super::pool::connect_path as pool_connect_path;

/// A `file_mappings` row with its host-internal path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedFileRow {
    pub path: String,
    pub torrent_hash: String,
    pub media_id: Option<String>,
    pub episode_no: Option<i64>,
}

/// Cleanup state access over its own pool. `core` creates one lazily; the
/// library repository behind the read ports is left untouched.
#[derive(Clone)]
pub struct CleanupRepository {
    pool: SqlitePool,
}

impl CleanupRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn connect_path(path: &std::path::Path) -> Result<Self, sqlx::Error> {
        Ok(Self::new(pool_connect_path(path).await?))
    }

    fn internal(err: sqlx::Error) -> AppError {
        AppError::new(AppErrorCode::Internal, err.to_string(), true)
    }

    /// Record a B5-confirmed completion. `eligible_at` is when the replay grace
    /// expires; state `pending` means "confirmed, awaiting the scheduled pass".
    pub async fn confirm(&self, media_id: &str, eligible_at: &str) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO cleanup_entries (media_id, eligible_at, state, outcome) \
             VALUES (?, ?, 'pending', NULL) \
             ON CONFLICT(media_id) DO UPDATE SET eligible_at = excluded.eligible_at, \
             state = 'pending', outcome = NULL",
        )
        .bind(media_id)
        .bind(eligible_at)
        .execute(&self.pool)
        .await
        .map_err(Self::internal)?;
        Ok(())
    }

    pub async fn state(&self, media_id: &str) -> Result<Option<CleanupEntry>, AppError> {
        let row = sqlx::query(
            "SELECT media_id, eligible_at, state, outcome FROM cleanup_entries WHERE media_id = ?",
        )
        .bind(media_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(Self::internal)?;
        Ok(row.map(|row| CleanupEntry {
            media_id: row.get("media_id"),
            eligible_at: row.get("eligible_at"),
            state: row.get("state"),
            outcome: row.get("outcome"),
        }))
    }

    /// Persist the terminal state and a visible, honest outcome.
    pub async fn set_state(
        &self,
        media_id: &str,
        state: &str,
        outcome: Option<&str>,
    ) -> Result<(), AppError> {
        sqlx::query("UPDATE cleanup_entries SET state = ?, outcome = ? WHERE media_id = ?")
            .bind(state)
            .bind(outcome)
            .bind(media_id)
            .execute(&self.pool)
            .await
            .map_err(Self::internal)?;
        Ok(())
    }

    pub async fn mappings_for_media(
        &self,
        media_id: &str,
    ) -> Result<Vec<ManagedFileRow>, AppError> {
        self.mappings(
            "SELECT file_path, torrent_hash, media_id, episode_no \
                       FROM file_mappings WHERE media_id = ?",
            media_id,
        )
        .await
    }

    pub async fn mappings_for_torrent(&self, hash: &str) -> Result<Vec<ManagedFileRow>, AppError> {
        self.mappings(
            "SELECT file_path, torrent_hash, media_id, episode_no \
                       FROM file_mappings WHERE torrent_hash = ?",
            hash,
        )
        .await
    }

    async fn mappings(&self, sql: &str, key: &str) -> Result<Vec<ManagedFileRow>, AppError> {
        let rows = sqlx::query(sql)
            .bind(key)
            .fetch_all(&self.pool)
            .await
            .map_err(Self::internal)?;
        Ok(rows
            .iter()
            .map(|row| ManagedFileRow {
                path: row.get("file_path"),
                torrent_hash: row.get("torrent_hash"),
                media_id: row.get("media_id"),
                episode_no: row.get("episode_no"),
            })
            .collect())
    }

    /// Current download state of an app-managed media id, if any.
    pub async fn download_state_for_media(
        &self,
        media_id: &str,
    ) -> Result<Option<DownloadState>, AppError> {
        let state: Option<String> =
            sqlx::query_scalar("SELECT state FROM download_items WHERE media_id = ? LIMIT 1")
                .bind(media_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(Self::internal)?;
        Ok(state.as_deref().map(download_state_from_str))
    }

    /// Id of the download item for a media id (used to update its state).
    pub async fn download_id_for_media(&self, media_id: &str) -> Result<Option<String>, AppError> {
        sqlx::query_scalar("SELECT id FROM download_items WHERE media_id = ? LIMIT 1")
            .bind(media_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(Self::internal)
    }

    pub async fn set_download_state(&self, id: &str, state: DownloadState) -> Result<(), AppError> {
        sqlx::query("UPDATE download_items SET state = ? WHERE id = ?")
            .bind(download_state_to_str(state))
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(Self::internal)?;
        Ok(())
    }

    /// True when a pending/active prefetch entry targets this media id.
    pub async fn is_prefetch_target(&self, media_id: &str) -> Result<bool, AppError> {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM prefetch_entries WHERE media_id = ?")
                .bind(media_id)
                .fetch_one(&self.pool)
                .await
                .map_err(Self::internal)?;
        Ok(count > 0)
    }
}

fn download_state_to_str(value: DownloadState) -> &'static str {
    match value {
        DownloadState::Downloading => "downloading",
        DownloadState::Ready => "ready",
        DownloadState::Playing => "playing",
        DownloadState::Watched => "watched",
        DownloadState::CleanupPending => "cleanup_pending",
        DownloadState::Error => "error",
    }
}

fn download_state_from_str(value: &str) -> DownloadState {
    match value {
        "ready" => DownloadState::Ready,
        "playing" => DownloadState::Playing,
        "watched" => DownloadState::Watched,
        "cleanup_pending" => DownloadState::CleanupPending,
        "error" => DownloadState::Error,
        _ => DownloadState::Downloading,
    }
}
