//! SQLite implementation of [`LibraryRepository`].
//!
//! Scope: read paths that back the frozen frontend command contract, plus the
//! mutations the contract exposes (watchlist, queue, settings, playback start).
//! Scanning and file discovery are out of scope here.

use app_core::domain::*;
use app_core::ports::LibraryRepository;
use async_trait::async_trait;
use sqlx::{Row, SqlitePool};

/// Column list kept in one place so row mapping and queries cannot drift.
const MEDIA_COLUMNS: &str = "id, kind, title, year, synopsis, runtime_minutes, genres, \
     availability_state, availability_last_seen, progress_position, progress_duration, \
     progress_updated_at, progress_completed, director, cast_members, series_id, \
     season_number, episode_number, next_up";

const MAX_LIMIT: i64 = 200;

pub struct SqliteLibraryRepository {
    pool: SqlitePool,
}

impl SqliteLibraryRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Borrow the underlying pool. Used by seeding and maintenance paths that
    /// deliberately bypass the read ports.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    fn internal(err: sqlx::Error) -> AppError {
        AppError::new(AppErrorCode::Internal, err.to_string(), true)
    }

    async fn fetch_row(&self, id: &str) -> Result<Option<MediaItemRow>, AppError> {
        let sql = format!("SELECT {MEDIA_COLUMNS} FROM media_items WHERE id = ?");
        sqlx::query_as::<_, MediaItemRow>(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(Self::internal)
    }

    async fn fetch_summary(&self, id: &str) -> Result<Option<MediaSummary>, AppError> {
        Ok(self.fetch_row(id).await?.as_ref().map(row_to_summary))
    }

    async fn seasons_for(&self, series_id: &str) -> Result<Vec<SeasonSummary>, AppError> {
        let rows = sqlx::query(
            "SELECT season_number, COUNT(*) AS episode_count \
             FROM media_items \
             WHERE kind = 'episode' AND series_id = ? AND season_number IS NOT NULL \
             GROUP BY season_number \
             ORDER BY season_number",
        )
        .bind(series_id)
        .fetch_all(&self.pool)
        .await
        .map_err(Self::internal)?;

        Ok(rows
            .iter()
            .map(|r| {
                let number: i64 = r.get("season_number");
                SeasonSummary {
                    number,
                    title: format!("Season {number}"),
                    episode_count: r.get("episode_count"),
                }
            })
            .collect())
    }

    async fn series_detail(&self, row: &MediaItemRow) -> Result<SeriesDetail, AppError> {
        Ok(SeriesDetail {
            id: row.id.clone(),
            kind: MediaKind::Series,
            title: row.title.clone(),
            year: row.year,
            synopsis: row.synopsis.clone(),
            genres: parse_list(&row.genres),
            artwork: vec![],
            seasons: self.seasons_for(&row.id).await?,
        })
    }

    async fn browse(
        &self,
        kind: MediaKind,
        query: &BrowseQuery,
    ) -> Result<Vec<MediaItemRow>, AppError> {
        let limit = normalise_limit(query.limit);
        let offset = query.offset.max(0);

        let mut sql = format!("SELECT {MEDIA_COLUMNS} FROM media_items WHERE kind = ?");
        if query.genre.is_some() {
            sql.push_str(" AND genres LIKE ?");
        }
        sql.push_str(match query.sort {
            BrowseSort::Title => " ORDER BY title COLLATE NOCASE ASC",
            BrowseSort::Year => " ORDER BY year DESC NULLS LAST, title COLLATE NOCASE ASC",
            BrowseSort::Recent => " ORDER BY created_at DESC, title COLLATE NOCASE ASC",
        });
        sql.push_str(" LIMIT ? OFFSET ?");

        let mut statement = sqlx::query_as::<_, MediaItemRow>(&sql).bind(kind_str(kind));
        if let Some(genre) = &query.genre {
            statement = statement.bind(genre_match(genre));
        }
        statement
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
            .map_err(Self::internal)
    }
}

#[derive(sqlx::FromRow)]
struct MediaItemRow {
    id: String,
    kind: String,
    title: String,
    year: Option<i64>,
    synopsis: Option<String>,
    runtime_minutes: Option<i64>,
    genres: String,
    availability_state: String,
    availability_last_seen: Option<String>,
    progress_position: Option<f64>,
    progress_duration: Option<f64>,
    progress_updated_at: Option<String>,
    progress_completed: i64,
    director: Option<String>,
    cast_members: String,
    series_id: Option<String>,
    season_number: Option<i64>,
    episode_number: Option<i64>,
    next_up: i64,
}

#[async_trait]
impl LibraryRepository for SqliteLibraryRepository {
    async fn home(&self) -> Result<HomeScreen, AppError> {
        let recent = BrowseQuery {
            sort: BrowseSort::Recent,
            genre: None,
            limit: 20,
            offset: 0,
        };
        let movies = self
            .browse(MediaKind::Movie, &recent)
            .await?
            .iter()
            .map(row_to_summary)
            .collect::<Vec<_>>();

        let series_rows = self.browse(MediaKind::Series, &recent).await?;
        let mut shows = Vec::with_capacity(series_rows.len());
        for row in &series_rows {
            shows.push(self.series_detail(row).await?);
        }

        let sql = format!(
            "SELECT {MEDIA_COLUMNS} FROM media_items \
             WHERE progress_position IS NOT NULL AND progress_completed = 0 \
             ORDER BY progress_updated_at DESC LIMIT 20"
        );
        let continue_watching = sqlx::query_as::<_, MediaItemRow>(&sql)
            .fetch_all(&self.pool)
            .await
            .map_err(Self::internal)?
            .iter()
            .map(row_to_summary)
            .collect::<Vec<_>>();

        let mut shelves = Vec::new();
        if !continue_watching.is_empty() {
            shelves.push(MediaShelf {
                id: ShelfId::ContinueWatching,
                title: "Continue Watching".to_string(),
                items: continue_watching,
            });
        }
        if !movies.is_empty() {
            shelves.push(MediaShelf {
                id: ShelfId::RecentlyAddedMovies,
                title: "Recently Added Movies".to_string(),
                items: movies,
            });
        }

        let hero = series_rows.first().map(|row| HeroFeature {
            media: MediaSummary {
                id: row.id.clone(),
                kind: MediaKind::Series,
                title: row.title.clone(),
                year: row.year,
                synopsis: row.synopsis.clone(),
                runtime_minutes: None,
                genres: parse_list(&row.genres),
                artwork: vec![],
                availability: availability(&row.availability_state, &row.availability_last_seen),
                progress: None,
            },
            headline: row.title.clone(),
            actions: vec![
                HeroAction {
                    kind: HeroActionKind::Play,
                    label: "Play".to_string(),
                },
                HeroAction {
                    kind: HeroActionKind::Details,
                    label: "Details".to_string(),
                },
            ],
        });

        Ok(HomeScreen { hero, shelves })
    }

    async fn movies(&self, query: &BrowseQuery) -> Result<Vec<MediaSummary>, AppError> {
        Ok(self
            .browse(MediaKind::Movie, query)
            .await?
            .iter()
            .map(row_to_summary)
            .collect())
    }

    async fn shows(&self, query: &BrowseQuery) -> Result<Vec<SeriesDetail>, AppError> {
        let rows = self.browse(MediaKind::Series, query).await?;
        let mut out = Vec::with_capacity(rows.len());
        for row in &rows {
            out.push(self.series_detail(row).await?);
        }
        Ok(out)
    }

    async fn detail(&self, id: &str) -> Result<MediaDetail, AppError> {
        let row = self
            .fetch_row(id)
            .await?
            .ok_or_else(|| AppError::new(AppErrorCode::NotFound, "media not found", false))?;

        match kind_from_str(&row.kind) {
            MediaKind::Movie => Ok(MediaDetail::Movie(MovieDetail {
                id: row.id.clone(),
                kind: MediaKind::Movie,
                title: row.title.clone(),
                year: row.year,
                synopsis: row.synopsis.clone(),
                runtime_minutes: row.runtime_minutes,
                genres: parse_list(&row.genres),
                artwork: vec![],
                availability: availability(&row.availability_state, &row.availability_last_seen),
                progress: progress(&row),
                director: row.director.clone(),
                cast: parse_list(&row.cast_members),
            })),
            MediaKind::Series => Ok(MediaDetail::Series(self.series_detail(&row).await?)),
            // Episodes are reached through `library_episodes`; there is no
            // single-episode detail shape in the contract.
            MediaKind::Episode => Err(AppError::new(
                AppErrorCode::NotFound,
                "episode detail is not a standalone resource",
                false,
            )),
        }
    }

    async fn episodes(
        &self,
        series_id: &str,
        season_number: i64,
    ) -> Result<Vec<EpisodeSummary>, AppError> {
        let series = self
            .fetch_row(series_id)
            .await?
            .ok_or_else(|| AppError::new(AppErrorCode::NotFound, "series not found", false))?;
        if kind_from_str(&series.kind) != MediaKind::Series {
            return Err(AppError::new(
                AppErrorCode::InvalidRequest,
                "requested id is not a series",
                false,
            ));
        }

        let sql = format!(
            "SELECT {MEDIA_COLUMNS} FROM media_items \
             WHERE kind = 'episode' AND series_id = ? AND season_number = ? \
             ORDER BY episode_number ASC"
        );
        let rows = sqlx::query_as::<_, MediaItemRow>(&sql)
            .bind(series_id)
            .bind(season_number)
            .fetch_all(&self.pool)
            .await
            .map_err(Self::internal)?;

        Ok(rows
            .iter()
            .map(|row| EpisodeSummary {
                id: row.id.clone(),
                series_id: row.series_id.clone().unwrap_or_else(|| series_id.to_string()),
                series_title: series.title.clone(),
                season_number: row.season_number.unwrap_or(season_number),
                episode_number: row.episode_number.unwrap_or(0),
                title: row.title.clone(),
                runtime_minutes: row.runtime_minutes,
                artwork: vec![],
                availability: availability(
                    &row.availability_state,
                    &row.availability_last_seen,
                ),
                progress: progress(row),
                next_up: row.next_up != 0,
            })
            .collect())
    }

    async fn search(&self, query: &SearchQuery) -> Result<SearchResults, AppError> {
        let text = query.text.trim();
        if text.is_empty() {
            return Ok(SearchResults {
                query: query.clone(),
                items: vec![],
                total: 0,
                truncated: false,
            });
        }

        let mut sql = format!("SELECT {MEDIA_COLUMNS} FROM media_items WHERE title LIKE ? ESCAPE '\\'");
        if !query.kinds.is_empty() {
            let placeholders = vec!["?"; query.kinds.len()];
            sql.push_str(&format!(" AND kind IN ({})", placeholders.join(",")));
        }
        if !query.genres.is_empty() {
            for _ in 0..query.genres.len() {
                sql.push_str(" AND genres LIKE ?");
            }
        }
        sql.push_str(" ORDER BY title COLLATE NOCASE ASC LIMIT ?");

        let mut statement = sqlx::query_as::<_, MediaItemRow>(&sql).bind(contains_match(text));
        for kind in &query.kinds {
            statement = statement.bind(kind_str(*kind));
        }
        for genre in &query.genres {
            statement = statement.bind(genre_match(genre));
        }
        let rows = statement
            .bind(MAX_LIMIT)
            .fetch_all(&self.pool)
            .await
            .map_err(Self::internal)?;

        let truncated = rows.len() as i64 >= MAX_LIMIT;
        let items = rows.iter().map(row_to_summary).collect::<Vec<_>>();
        Ok(SearchResults {
            query: query.clone(),
            total: items.len() as i64,
            items,
            truncated,
        })
    }

    async fn roots(&self) -> Result<LibraryScreen, AppError> {
        Ok(LibraryScreen {
            roots: self.load_roots().await?,
            counts: self.load_counts().await?,
        })
    }

    async fn watchlist(&self) -> Result<WatchlistScreen, AppError> {
        let rows = sqlx::query(
            "SELECT id, media_id, position, added_at FROM watchlist_items ORDER BY position ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Self::internal)?;

        let mut entries = Vec::with_capacity(rows.len());
        for row in &rows {
            let media_id: String = row.get("media_id");
            if let Some(media) = self.fetch_summary(&media_id).await? {
                entries.push(WatchlistEntry {
                    id: row.get("id"),
                    media,
                    position: row.get("position"),
                    added_at: row.get("added_at"),
                });
            }
        }
        Ok(WatchlistScreen { entries })
    }

    async fn queue(&self) -> Result<QueueScreen, AppError> {
        let rows =
            sqlx::query("SELECT id, media_id, position, state FROM queue_items ORDER BY position ASC")
                .fetch_all(&self.pool)
                .await
                .map_err(Self::internal)?;

        let mut entries = Vec::with_capacity(rows.len());
        for row in &rows {
            let media_id: String = row.get("media_id");
            if let Some(media) = self.fetch_summary(&media_id).await? {
                entries.push(QueueEntry {
                    id: row.get("id"),
                    media,
                    position: row.get("position"),
                    state: queue_state_from_str(&row.get::<String, _>("state")),
                });
            }
        }
        Ok(QueueScreen { entries })
    }

    async fn downloads(&self) -> Result<DownloadsScreen, AppError> {
        // Downloads are an optional service and are disabled until the runtime
        // and storage layers exist. The contract requires the shape, not the
        // feature.
        let storage_limit = self.settings_get().await?.storage.limit_bytes;
        Ok(DownloadsScreen {
            enabled: false,
            tasks: vec![],
            storage_used_bytes: 0,
            storage_limit_bytes: storage_limit,
        })
    }

    async fn settings_get(&self) -> Result<AppSettings, AppError> {
        let stored: Option<String> = sqlx::query_scalar("SELECT data FROM settings WHERE id = 1")
            .fetch_optional(&self.pool)
            .await
            .map_err(Self::internal)?;

        let mut settings = stored
            .and_then(|data| serde_json::from_str::<AppSettings>(&data).ok())
            .unwrap_or_else(default_settings);

        // Roots are derived from the roots table so settings and the library
        // screen cannot disagree.
        settings.library_roots = self.load_roots().await?;
        Ok(settings)
    }

    async fn settings_set(&self, patch: &SettingsPatch) -> Result<AppSettings, AppError> {
        let mut settings = self.settings_get().await?;

        if let Some(playback) = &patch.playback {
            if let Some(value) = &playback.preferred_language {
                settings.playback.preferred_language = value.clone();
            }
            if let Some(value) = &playback.subtitle_size {
                settings.playback.subtitle_size = value.clone();
            }
            if let Some(value) = playback.autoplay_next {
                settings.playback.autoplay_next = value;
            }
        }
        if let Some(storage) = &patch.storage {
            if let Some(value) = &storage.root_display_name {
                settings.storage.root_display_name = value.clone();
            }
            if let Some(value) = storage.limit_bytes {
                settings.storage.limit_bytes = Some(value);
            }
        }

        let encoded = serde_json::to_string(&settings).map_err(|e| {
            AppError::new(AppErrorCode::Internal, format!("settings encode failed: {e}"), false)
        })?;
        sqlx::query(
            "INSERT INTO settings (id, data) VALUES (1, ?) \
             ON CONFLICT(id) DO UPDATE SET data = excluded.data",
        )
        .bind(&encoded)
        .execute(&self.pool)
        .await
        .map_err(Self::internal)?;

        settings.library_roots = self.load_roots().await?;
        Ok(settings)
    }

    async fn playback_start(
        &self,
        request: &PlaybackStartRequest,
    ) -> Result<PlaybackSession, AppError> {
        let row = self
            .fetch_row(&request.media_id)
            .await?
            .ok_or_else(|| AppError::new(AppErrorCode::NotFound, "media not found", false))?;

        if let MediaAvailability::Available = availability(
            &row.availability_state,
            &row.availability_last_seen,
        ) {
            // Recording the start position is what makes Continue Watching
            // populate; it is not a "completed" signal.
            let duration = row
                .progress_duration
                .or_else(|| row.runtime_minutes.map(|m| m as f64 * 60.0))
                .unwrap_or(0.0);
            let start = request.start_position_seconds.unwrap_or(0.0).max(0.0);
            sqlx::query(
                "UPDATE media_items SET progress_position = ?, progress_duration = ?, \
                 progress_updated_at = ?, progress_completed = 0, updated_at = ? WHERE id = ?",
            )
            .bind(start)
            .bind(duration)
            .bind(now_iso())
            .bind(now_iso())
            .bind(&row.id)
            .execute(&self.pool)
            .await
            .map_err(Self::internal)?;

            return Ok(PlaybackSession {
                session_id: uuid::Uuid::new_v4().to_string(),
                media: row_to_summary(&row),
                start_position_seconds: start,
                duration_seconds: duration,
                subtitle_tracks: vec![],
                audio_tracks: vec![],
                can_resume: start > 0.0,
            });
        }

        Err(AppError::new(
            AppErrorCode::MediaMissing,
            "media file is not available for playback",
            true,
        ))
    }

    async fn playback_stop(&self) -> Result<(), AppError> {
        // Local playback is owned by the player surface, which reports progress
        // through its own path. Nothing durable to write here yet.
        Ok(())
    }

    async fn watchlist_add(&self, media_id: &str) -> Result<(), AppError> {
        if self.fetch_row(media_id).await?.is_none() {
            return Err(AppError::new(AppErrorCode::NotFound, "media not found", false));
        }
        let next: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(position), 0) + 1 FROM watchlist_items")
            .fetch_one(&self.pool)
            .await
            .map_err(Self::internal)?;
        sqlx::query(
            "INSERT INTO watchlist_items (id, media_id, position, added_at) VALUES (?, ?, ?, ?) \
             ON CONFLICT(media_id) DO NOTHING",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(media_id)
        .bind(next)
        .bind(now_iso())
        .execute(&self.pool)
        .await
        .map_err(Self::internal)?;
        Ok(())
    }

    async fn watchlist_remove(&self, media_id: &str) -> Result<(), AppError> {
        sqlx::query("DELETE FROM watchlist_items WHERE media_id = ?")
            .bind(media_id)
            .execute(&self.pool)
            .await
            .map_err(Self::internal)?;
        Ok(())
    }

    async fn watchlist_reorder(&self, ordered_media_ids: &[String]) -> Result<(), AppError> {
        let mut tx = self.pool.begin().await.map_err(Self::internal)?;
        for (position, media_id) in ordered_media_ids.iter().enumerate() {
            sqlx::query("UPDATE watchlist_items SET position = ? WHERE media_id = ?")
                .bind(position as i64)
                .bind(media_id)
                .execute(&mut *tx)
                .await
                .map_err(Self::internal)?;
        }
        tx.commit().await.map_err(Self::internal)?;
        Ok(())
    }

    async fn queue_add(&self, media_id: &str) -> Result<(), AppError> {
        if self.fetch_row(media_id).await?.is_none() {
            return Err(AppError::new(AppErrorCode::NotFound, "media not found", false));
        }
        let next: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(position), 0) + 1 FROM queue_items")
            .fetch_one(&self.pool)
            .await
            .map_err(Self::internal)?;
        sqlx::query(
            "INSERT INTO queue_items (id, media_id, position, state) VALUES (?, ?, ?, 'pending')",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(media_id)
        .bind(next)
        .execute(&self.pool)
        .await
        .map_err(Self::internal)?;
        Ok(())
    }

    async fn queue_remove(&self, queue_id: &str) -> Result<(), AppError> {
        sqlx::query("DELETE FROM queue_items WHERE id = ?")
            .bind(queue_id)
            .execute(&self.pool)
            .await
            .map_err(Self::internal)?;
        Ok(())
    }

    async fn queue_reorder(&self, ordered_queue_ids: &[String]) -> Result<(), AppError> {
        let mut tx = self.pool.begin().await.map_err(Self::internal)?;
        for (position, queue_id) in ordered_queue_ids.iter().enumerate() {
            sqlx::query("UPDATE queue_items SET position = ? WHERE id = ?")
                .bind(position as i64)
                .bind(queue_id)
                .execute(&mut *tx)
                .await
                .map_err(Self::internal)?;
        }
        tx.commit().await.map_err(Self::internal)?;
        Ok(())
    }
}

impl SqliteLibraryRepository {
    async fn load_roots(&self) -> Result<Vec<LibraryRoot>, AppError> {
        let rows = sqlx::query(
            "SELECT id, display_name, item_count, status, last_scan_at, error_message \
             FROM library_roots ORDER BY display_name COLLATE NOCASE ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Self::internal)?;

        Ok(rows
            .iter()
            .map(|row| LibraryRoot {
                id: row.get("id"),
                display_name: row.get("display_name"),
                item_count: row.get("item_count"),
                status: root_status_from_str(&row.get::<String, _>("status")),
                last_scan_at: row.get("last_scan_at"),
                error_message: row.get("error_message"),
            })
            .collect())
    }

    async fn load_counts(&self) -> Result<LibraryCounts, AppError> {
        let rows =
            sqlx::query("SELECT kind, COUNT(*) AS total FROM media_items GROUP BY kind")
                .fetch_all(&self.pool)
                .await
                .map_err(Self::internal)?;

        let mut counts = LibraryCounts {
            movies: 0,
            shows: 0,
            episodes: 0,
        };
        for row in &rows {
            let total: i64 = row.get("total");
            match kind_from_str(&row.get::<String, _>("kind")) {
                MediaKind::Movie => counts.movies = total,
                MediaKind::Series => counts.shows = total,
                MediaKind::Episode => counts.episodes = total,
            }
        }
        Ok(counts)
    }
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn normalise_limit(limit: i64) -> i64 {
    if limit <= 0 {
        50
    } else {
        limit.min(MAX_LIMIT)
    }
}

fn kind_str(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Movie => "movie",
        MediaKind::Series => "series",
        MediaKind::Episode => "episode",
    }
}

fn kind_from_str(value: &str) -> MediaKind {
    match value {
        "series" => MediaKind::Series,
        "episode" => MediaKind::Episode,
        _ => MediaKind::Movie,
    }
}

fn availability(state: &str, last_seen: &Option<String>) -> MediaAvailability {
    match state {
        "missing-file" => MediaAvailability::MissingFile {
            last_seen_at: last_seen.clone(),
        },
        "offline" => MediaAvailability::Offline,
        _ => MediaAvailability::Available,
    }
}

fn progress(row: &MediaItemRow) -> Option<PlaybackProgress> {
    row.progress_updated_at.as_ref().map(|updated_at| PlaybackProgress {
        position_seconds: row.progress_position.unwrap_or(0.0),
        duration_seconds: row.progress_duration.unwrap_or(0.0),
        updated_at: updated_at.clone(),
        completed: row.progress_completed != 0,
    })
}

fn row_to_summary(row: &MediaItemRow) -> MediaSummary {
    MediaSummary {
        id: row.id.clone(),
        kind: kind_from_str(&row.kind),
        title: row.title.clone(),
        year: row.year,
        synopsis: row.synopsis.clone(),
        runtime_minutes: row.runtime_minutes,
        genres: parse_list(&row.genres),
        artwork: vec![],
        availability: availability(&row.availability_state, &row.availability_last_seen),
        progress: progress(row),
    }
}

fn parse_list(raw: &str) -> Vec<String> {
    serde_json::from_str(raw).unwrap_or_default()
}

fn root_status_from_str(value: &str) -> LibraryRootStatus {
    match value {
        "scanning" => LibraryRootStatus::Scanning,
        "offline" => LibraryRootStatus::Offline,
        "error" => LibraryRootStatus::Error,
        _ => LibraryRootStatus::Ok,
    }
}

fn queue_state_from_str(value: &str) -> QueueEntryState {
    match value {
        "active" => QueueEntryState::Active,
        "done" => QueueEntryState::Done,
        "failed" => QueueEntryState::Failed,
        _ => QueueEntryState::Pending,
    }
}

/// Escapes LIKE metacharacters so user text is matched literally.
fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn contains_match(value: &str) -> String {
    format!("%{}%", escape_like(value))
}

/// Matches a genre inside the JSON-encoded `genres` array column.
fn genre_match(genre: &str) -> String {
    format!("%\"{}\"%", escape_like(genre))
}

fn default_settings() -> AppSettings {
    AppSettings {
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
    }
}
