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

/// Muted profile palette. Mirrors the `--tone-*` presentation tokens so a
/// profile keeps the same colour whichever layer supplies it, but the stored
/// value is a real hex colour rather than a CSS variable.
const PROFILE_PALETTE: [&str; 4] = ["#5f7183", "#7d6a7a", "#63786c", "#82755f"];

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
            imdb_rating: None,
            vote_count: None,
            age_rating: None,
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
                imdb_rating: None,
                vote_count: None,
                age_rating: None,
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
        // Downloaded items must pass the probe gate before the UI may even
        // present their detail. Library imports have no download row and stay
        // allowed; the reason returned here is display-only (never a path).
        self.refuse_unverified(id).await?;
        // Task 2: Defender half of the combined gate, alongside Task 1's probe
        // check above. A flagged download gets an honest blocked state, not a
        // normal detail view; the reason never contains a path.
        self.refuse_unscanned_download(id).await?;

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
                imdb_rating: None,
                vote_count: None,
                age_rating: None,
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
                series_id: row
                    .series_id
                    .clone()
                    .unwrap_or_else(|| series_id.to_string()),
                series_title: series.title.clone(),
                season_number: row.season_number.unwrap_or(season_number),
                episode_number: row.episode_number.unwrap_or(0),
                title: row.title.clone(),
                runtime_minutes: row.runtime_minutes,
                artwork: vec![],
                availability: availability(&row.availability_state, &row.availability_last_seen),
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

        let mut sql =
            format!("SELECT {MEDIA_COLUMNS} FROM media_items WHERE title LIKE ? ESCAPE '\\'");
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

    async fn root_add(&self, display_name: &str, path: &str) -> Result<LibraryRoot, AppError> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO library_roots (id, display_name, path, item_count, status) VALUES (?, ?, ?, 0, 'ok')",
        )
        .bind(&id)
        .bind(display_name)
        .bind(path)
        .execute(&self.pool)
        .await
        .map_err(Self::internal)?;
        Ok(LibraryRoot {
            id,
            display_name: display_name.to_string(),
            item_count: 0,
            status: LibraryRootStatus::Ok,
            last_scan_at: None,
            error_message: None,
            path: Some(path.to_string()),
        })
    }

    async fn root_remove(&self, id: &str) -> Result<(), AppError> {
        // Remove root + its media items (cascade via root_id SET NULL keeps
        // media rows but detaches them; we explicitly delete to keep tidy).
        sqlx::query("DELETE FROM media_items WHERE root_id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(Self::internal)?;
        sqlx::query("DELETE FROM library_roots WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(Self::internal)?;
        Ok(())
    }

    async fn root_update(
        &self,
        id: &str,
        item_count: i64,
        status: LibraryRootStatus,
        last_scan_at: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<(), AppError> {
        let status_str = root_status_to_str(status);
        sqlx::query(
            "UPDATE library_roots SET item_count = ?, status = ?, last_scan_at = ?, error_message = ? WHERE id = ?",
        )
        .bind(item_count)
        .bind(status_str)
        .bind(last_scan_at)
        .bind(error_message)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(Self::internal)?;
        Ok(())
    }

    async fn recover_interrupted_scans(&self) -> Result<usize, AppError> {
        let result = sqlx::query(
            "UPDATE library_roots SET status = 'error', \
             error_message = 'Scan interrupted by restart.' \
             WHERE status = 'scanning'",
        )
        .execute(&self.pool)
        .await
        .map_err(Self::internal)?;
        Ok(result.rows_affected() as usize)
    }

    async fn media_replace_for_root(
        &self,
        root_id: &str,
        items: &[MediaUpsert],
    ) -> Result<(), AppError> {
        // An id-less item would create a duplicate row on every scan, so reject
        // the batch before anything is written.
        if let Some(item) = items.iter().find(|item| item.id.is_empty()) {
            return Err(AppError::new(
                AppErrorCode::InvalidRequest,
                format!("media item {:?} has no stable id", item.title),
                false,
            ));
        }

        let now = chrono::Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await.map_err(Self::internal)?;

        // Drop the previous file rows for this root in one pass; they are
        // rewritten below, so rescans do not accumulate duplicates.
        sqlx::query(
            "DELETE FROM media_files \
             WHERE media_id IN (SELECT id FROM media_items WHERE root_id = ?)",
        )
        .bind(root_id)
        .execute(&mut *tx)
        .await
        .map_err(Self::internal)?;

        for item in items {
            let metadata = item.metadata.as_ref();
            let genres = metadata
                .map(|m| serde_json::to_string(&m.genres).unwrap_or_else(|_| "[]".to_string()))
                .unwrap_or_else(|| "[]".to_string());
            // COALESCE keeps what a previous scan or the user already supplied:
            // a file with no sidecar must not blank its own metadata on rescan.
            sqlx::query(
                "INSERT INTO media_items \
                 (id, kind, title, year, synopsis, runtime_minutes, genres, root_id, created_at, updated_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
                 ON CONFLICT(id) DO UPDATE SET kind = excluded.kind, title = excluded.title, \
                 year = COALESCE(excluded.year, media_items.year), \
                 synopsis = COALESCE(excluded.synopsis, media_items.synopsis), \
                 runtime_minutes = COALESCE(excluded.runtime_minutes, media_items.runtime_minutes), \
                 genres = CASE WHEN excluded.genres = '[]' THEN media_items.genres \
                 ELSE excluded.genres END, \
                 root_id = excluded.root_id, updated_at = excluded.updated_at",
            )
            .bind(&item.id)
            .bind(kind_str(item.kind))
            .bind(&item.title)
            .bind(metadata.and_then(|m| m.year))
            .bind(metadata.and_then(|m| m.synopsis.clone()))
            .bind(metadata.and_then(|m| m.runtime_minutes))
            .bind(&genres)
            .bind(root_id)
            .bind(&now)
            .bind(&now)
            .execute(&mut *tx)
            .await
            .map_err(Self::internal)?;

            if let Some(path) = &item.path {
                sqlx::query(
                    "INSERT INTO media_files (id, media_id, size_bytes, hash, path, created_at) \
                     VALUES (?, ?, ?, ?, ?, ?)",
                )
                .bind(uuid::Uuid::new_v4().to_string())
                .bind(&item.id)
                .bind(item.size_bytes)
                .bind(&item.hash)
                .bind(path)
                .bind(&now)
                .execute(&mut *tx)
                .await
                .map_err(Self::internal)?;
            }
        }

        // Rows for this root that the scan no longer sees.
        let incoming: std::collections::HashSet<&str> =
            items.iter().map(|item| item.id.as_str()).collect();
        let existing: Vec<String> = sqlx::query("SELECT id FROM media_items WHERE root_id = ?")
            .bind(root_id)
            .fetch_all(&mut *tx)
            .await
            .map_err(Self::internal)?
            .iter()
            .map(|row| row.get("id"))
            .collect();
        let stale: Vec<&String> = existing
            .iter()
            .filter(|id| !incoming.contains(id.as_str()))
            .collect();
        for chunk in stale.chunks(500) {
            let placeholders = vec!["?"; chunk.len()].join(",");
            let sql = format!("DELETE FROM media_items WHERE id IN ({placeholders})");
            let mut query = sqlx::query(&sql);
            for id in chunk {
                query = query.bind(*id);
            }
            query.execute(&mut *tx).await.map_err(Self::internal)?;
        }

        tx.commit().await.map_err(Self::internal)
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
        let rows = sqlx::query(
            "SELECT id, media_id, position, state FROM queue_items ORDER BY position ASC",
        )
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
        // The acquisition runtime now exists. The switch is an operator
        // setting, not a hard-coded "off": reporting false here while the
        // status route shows live tasks made the Downloads screen lie.
        let settings = self.settings_get().await?;
        let storage_limit = settings.storage.limit_bytes;
        let rows = sqlx::query(
            "SELECT d.id, COALESCE(m.title, d.torrent_hash) AS title, d.state, t.progress \
             FROM download_items d \
             LEFT JOIN media_items m ON m.id = d.media_id \
             LEFT JOIN torrent_tasks t ON t.hash = d.torrent_hash \
             ORDER BY d.created_at DESC LIMIT ?",
        )
        .bind(MAX_LIMIT)
        .fetch_all(&self.pool)
        .await
        .map_err(Self::internal)?;
        let tasks = rows
            .iter()
            .map(|row| {
                let state = row.get::<String, _>("state");
                DownloadTask {
                    id: row.get("id"),
                    title: row.get("title"),
                    state: legacy_download_state(&state),
                    progress: row.try_get("progress").unwrap_or(0.0),
                    error_message: (state == "error").then(|| "The download failed.".to_string()),
                }
            })
            .collect();
        // No file-size column exists on download_items, so used bytes stay
        // zero until the host reports the acquisition folder. The buffer
        // notice uses the same honest zero rather than guessing a size.
        let used = 0;
        Ok(DownloadsScreen {
            enabled: settings.optional_services.downloads_enabled,
            tasks,
            storage_used_bytes: used,
            storage_limit_bytes: storage_limit,
            // Same G-8 predicate the prefetch scheduler uses; a message string
            // only, never a path (renderer DTO boundary).
            buffer_notice: buffer_pause_notice(used, storage_limit),
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

        if settings.profiles.is_empty() {
            settings.profiles = vec![Profile {
                id: uuid::Uuid::new_v4().to_string(),
                name: "You".to_string(),
                avatar_color: "#E50914".to_string(),
                created_at: now_iso(),
            }];
        }
        // Rows written before colours were normalised may hold non-colour
        // values; repair them on read so the API only returns hex colours.
        normalise_profiles(&mut settings.profiles);

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
        if let Some(profiles) = &patch.profiles {
            settings.profiles = profiles.clone();
        }
        normalise_profiles(&mut settings.profiles);

        let encoded = serde_json::to_string(&settings).map_err(|e| {
            AppError::new(
                AppErrorCode::Internal,
                format!("settings encode failed: {e}"),
                false,
            )
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
        // The authoritative gate: mpv never receives a downloaded file that
        // has not been parsed by the probe. Enforced here (not in React) so no
        // caller of the read port can bypass it.
        self.refuse_unverified(&request.media_id).await?;
        // Task 2: Defender half of the combined pre-play gate, alongside Task 1's
        // probe check above. No path or scanner detail reaches the caller.
        self.refuse_unscanned_download(&request.media_id).await?;

        if let MediaAvailability::Available =
            availability(&row.availability_state, &row.availability_last_seen)
        {
            // Recording the start position is what makes Continue Watching
            // populate; it is not a "completed" signal.
            let duration = row
                .progress_duration
                .or_else(|| row.runtime_minutes.map(|m| m as f64 * 60.0))
                .unwrap_or(0.0);
            // With no explicit start, continue from the stored checkpoint. A
            // null start used to reset a resume position to zero, which made
            // every Play from a shelf lose its place.
            let start = request
                .start_position_seconds
                .or(row.progress_position)
                .unwrap_or(0.0)
                .max(0.0);
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

    async fn playback_checkpoint(
        &self,
        request: &PlaybackCheckpointRequest,
    ) -> Result<(), AppError> {
        let row = self.fetch_row(&request.media_id).await?;
        if row.is_none() {
            return Err(AppError::new(
                AppErrorCode::MediaMissing,
                "media not found",
                false,
            ));
        }
        let position = request.position_seconds.max(0.0);
        let duration = request.duration_seconds.unwrap_or(0.0).max(0.0);
        let now = now_iso();
        sqlx::query(
            "UPDATE media_items SET progress_position = ?, progress_duration = ?, \
             progress_updated_at = ?, progress_completed = 0, updated_at = ? WHERE id = ?",
        )
        .bind(position)
        .bind(duration)
        .bind(&now)
        .bind(&now)
        .bind(&request.media_id)
        .execute(&self.pool)
        .await
        .map_err(Self::internal)?;
        Ok(())
    }

    async fn media_source_path(&self, id: &str) -> Result<Option<String>, AppError> {
        // Exact, bound match: LIKE metacharacters in an id are literal here.
        // A missing row (no file ever scanned, or the file was removed from
        // the library) is `None` — the caller fails closed (ADR-0015).
        //
        // An acquired download has no `media_files` row: the scanner only
        // writes library roots. Its container path lives on the download row
        // (`/downloads/<name>`, ADR-0018), which the host resolves through the
        // `/downloads` mount entry exactly like a scanned `/media/root-N`
        // path. Without this fallback a confirmed, downloaded source could
        // never resolve for playback.
        sqlx::query_scalar(
            "SELECT path FROM (\
                SELECT path AS path, 0 AS prio FROM media_files WHERE media_id = ? \
                UNION ALL \
                SELECT file_path AS path, 1 AS prio FROM download_items \
                WHERE media_id = ? AND file_path IS NOT NULL AND file_path <> '' \
            ) ORDER BY prio LIMIT 1",
        )
        .bind(id)
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(Self::internal)
    }

    async fn watchlist_add(&self, media_id: &str) -> Result<(), AppError> {
        if self.fetch_row(media_id).await?.is_none() {
            return Err(AppError::new(
                AppErrorCode::NotFound,
                "media not found",
                false,
            ));
        }
        let next: i64 =
            sqlx::query_scalar("SELECT COALESCE(MAX(position), 0) + 1 FROM watchlist_items")
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
            return Err(AppError::new(
                AppErrorCode::NotFound,
                "media not found",
                false,
            ));
        }
        let next: i64 =
            sqlx::query_scalar("SELECT COALESCE(MAX(position), 0) + 1 FROM queue_items")
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
        // C2: history is written on removal only; completion-history is deferred to
        // the playback B workstream, where a completion event exists.
        // Archive before deleting so a removed (or already completed) entry is
        // never silently lost. Insert + delete share one transaction: a crash
        // leaves either the queue row or its history row, never neither.
        let mut tx = self.pool.begin().await.map_err(Self::internal)?;
        let row = sqlx::query(
            "SELECT q.media_id, q.state, q.position, m.title \
             FROM queue_items q LEFT JOIN media_items m ON m.id = q.media_id \
             WHERE q.id = ?",
        )
        .bind(queue_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(Self::internal)?;

        if let Some(row) = row {
            sqlx::query(
                "INSERT INTO queue_history (queue_id, media_id, title, state, position, recorded_at) \
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(queue_id)
            .bind(row.get::<String, _>("media_id"))
            .bind(row.get::<Option<String>, _>("title"))
            .bind(row.get::<String, _>("state"))
            .bind(row.get::<i64, _>("position"))
            .bind(now_iso())
            .execute(&mut *tx)
            .await
            .map_err(Self::internal)?;

            sqlx::query("DELETE FROM queue_items WHERE id = ?")
                .bind(queue_id)
                .execute(&mut *tx)
                .await
                .map_err(Self::internal)?;
        }
        tx.commit().await.map_err(Self::internal)
    }

    async fn queue_reorder(&self, ordered_queue_ids: &[String]) -> Result<(), AppError> {
        // Pure rule first: a duplicated id is never a valid order.
        validate_queue_order(ordered_queue_ids)?;

        let mut tx = self.pool.begin().await.map_err(Self::internal)?;
        let existing: Vec<String> = sqlx::query("SELECT id FROM queue_items")
            .fetch_all(&mut *tx)
            .await
            .map_err(Self::internal)?
            .iter()
            .map(|row| row.get("id"))
            .collect();

        // A reorder must be an exact permutation of the current queue. Unknown
        // ids or a partial list would leave entries stranded on stale positions
        // and break dense renumbering, so reject instead of guessing.
        if ordered_queue_ids.len() != existing.len() {
            return Err(AppError::new(
                AppErrorCode::InvalidRequest,
                "reorder must include every queue entry exactly once",
                false,
            ));
        }
        let known: std::collections::HashSet<&str> = existing.iter().map(String::as_str).collect();
        if ordered_queue_ids
            .iter()
            .any(|id| !known.contains(id.as_str()))
        {
            return Err(AppError::new(
                AppErrorCode::InvalidRequest,
                "unknown queue id in reorder",
                false,
            ));
        }

        // Single transaction: positions are renumbered densely (0..n-1) and
        // only `position` changes — never `state`, so nothing advances on its
        // own. A crash mid-loop rolls the whole reorder back.
        for (position, queue_id) in ordered_queue_ids.iter().enumerate() {
            sqlx::query("UPDATE queue_items SET position = ? WHERE id = ?")
                .bind(position as i64)
                .bind(queue_id)
                .execute(&mut *tx)
                .await
                .map_err(Self::internal)?;
        }
        tx.commit().await.map_err(Self::internal)
    }

    async fn download_add(
        &self,
        item: &DownloadItem,
        file_path: &str,
        source: &str,
        save_path: &str,
    ) -> Result<(), AppError> {
        let now = now_iso();
        let state = download_state_to_str(item.state);
        let mut tx = self.pool.begin().await.map_err(Self::internal)?;

        sqlx::query(
            "INSERT INTO torrent_tasks \
             (hash, magnet_or_source, state, progress, save_path, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(hash) DO UPDATE SET state = excluded.state, \
             progress = excluded.progress, updated_at = excluded.updated_at",
        )
        .bind(&item.torrent_hash)
        .bind(source)
        .bind(state)
        .bind(item.progress)
        .bind(save_path)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(Self::internal)?;

        sqlx::query(
            "INSERT INTO download_items \
             (id, torrent_hash, media_id, file_path, state, created_at) \
             VALUES (?, ?, ?, ?, ?, ?) \
             ON CONFLICT(id) DO UPDATE SET media_id = excluded.media_id, \
             state = excluded.state",
        )
        .bind(&item.id)
        .bind(&item.torrent_hash)
        .bind(&item.media_id)
        .bind(file_path)
        .bind(state)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(Self::internal)?;

        // Record the app-managed file in the same transaction. Cleanup relies
        // on file_mappings to recognise an acquired file (app_managed=true);
        // without this row no production code ever creates the mapping and a
        // genuine acquisition could never be reconciled as removable.
        // `media_id` may be NULL until the file is identified (migration 0004).
        sqlx::query(
            "INSERT INTO file_mappings (file_path, torrent_hash, media_id, episode_no) \
             VALUES (?, ?, ?, NULL) \
             ON CONFLICT(file_path) DO UPDATE SET torrent_hash = excluded.torrent_hash, \
             media_id = excluded.media_id",
        )
        .bind(file_path)
        .bind(&item.torrent_hash)
        .bind(&item.media_id)
        .execute(&mut *tx)
        .await
        .map_err(Self::internal)?;

        tx.commit().await.map_err(Self::internal)
    }

    async fn download_status(&self, id: &str) -> Result<Option<DownloadItem>, AppError> {
        let row = sqlx::query(
            "SELECT d.id, d.torrent_hash, d.media_id, d.state, t.progress \
             FROM download_items d JOIN torrent_tasks t ON t.hash = d.torrent_hash \
             WHERE d.id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(Self::internal)?;

        Ok(row.map(|row| DownloadItem {
            id: row.get("id"),
            torrent_hash: row.get("torrent_hash"),
            media_id: row.get("media_id"),
            state: download_state_from_str(&row.get::<String, _>("state")),
            progress: row.get("progress"),
        }))
    }

    async fn acquisition_rows(&self) -> Result<Vec<(DownloadItem, String)>, AppError> {
        let rows = sqlx::query(
            "SELECT d.id, d.torrent_hash, d.media_id, d.state, t.progress, \
             t.magnet_or_source \
             FROM download_items d JOIN torrent_tasks t ON t.hash = d.torrent_hash",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Self::internal)?;
        Ok(rows
            .into_iter()
            .map(|row| {
                (
                    DownloadItem {
                        id: row.get("id"),
                        torrent_hash: row.get("torrent_hash"),
                        media_id: row.get("media_id"),
                        state: download_state_from_str(&row.get::<String, _>("state")),
                        progress: row.get("progress"),
                    },
                    row.get("magnet_or_source"),
                )
            })
            .collect())
    }

    async fn download_update(&self, item: &DownloadItem) -> Result<(), AppError> {
        // A status poll reports the *engine's* transport state. It must not
        // clobber the playback lifecycle state owned by the cleanup worker,
        // or a file that is playing/watched/cleanup-pending becomes eligible
        // for premature removal.
        let now = now_iso();
        let state = download_state_to_str(item.state);
        let mut tx = self.pool.begin().await.map_err(Self::internal)?;
        let current: Option<String> =
            sqlx::query_scalar("SELECT state FROM download_items WHERE id = ?")
                .bind(&item.id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(Self::internal)?;
        let Some(current) = current else {
            // No persisted row: never create one from a status poll alone.
            return Ok(());
        };
        if matches!(current.as_str(), "playing" | "watched" | "cleanup_pending") {
            sqlx::query("UPDATE download_items SET media_id = COALESCE(?, media_id) WHERE id = ?")
                .bind(&item.media_id)
                .bind(&item.id)
                .execute(&mut *tx)
                .await
                .map_err(Self::internal)?;
        } else {
            sqlx::query(
                "UPDATE download_items SET state = ?, media_id = COALESCE(?, media_id) WHERE id = ?",
            )
            .bind(state)
            .bind(&item.media_id)
            .bind(&item.id)
            .execute(&mut *tx)
            .await
            .map_err(Self::internal)?;
        }
        sqlx::query(
            "UPDATE torrent_tasks SET state = ?, progress = ?, updated_at = ? WHERE hash = ?",
        )
        .bind(state)
        .bind(item.progress)
        .bind(&now)
        .bind(&item.torrent_hash)
        .execute(&mut *tx)
        .await
        .map_err(Self::internal)?;
        tx.commit().await.map_err(Self::internal)
    }

    async fn download_set_paths(&self, id: &str, paths: &[String]) -> Result<(), AppError> {
        if paths.is_empty() {
            return Ok(());
        }
        let row = sqlx::query("SELECT torrent_hash, media_id FROM download_items WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(Self::internal)?;
        let Some(row) = row else {
            // A stale poll must not recreate a removed acquisition.
            return Ok(());
        };
        let hash: String = row.get("torrent_hash");
        let media_id: Option<String> = row.get("media_id");
        let mut tx = self.pool.begin().await.map_err(Self::internal)?;

        // Replace only auto-created mappings. Episode mappings supplied by a
        // library scan carry an episode_no and are left untouched.
        sqlx::query("DELETE FROM file_mappings WHERE torrent_hash = ? AND episode_no IS NULL")
            .bind(&hash)
            .execute(&mut *tx)
            .await
            .map_err(Self::internal)?;
        for path in paths {
            sqlx::query(
                "INSERT INTO file_mappings (file_path, torrent_hash, media_id, episode_no) \
                 VALUES (?, ?, ?, NULL) \
                 ON CONFLICT(file_path) DO UPDATE SET \
                 torrent_hash = excluded.torrent_hash, \
                 media_id = COALESCE(file_mappings.media_id, excluded.media_id)",
            )
            .bind(path)
            .bind(&hash)
            .bind(&media_id)
            .execute(&mut *tx)
            .await
            .map_err(Self::internal)?;
        }
        sqlx::query("UPDATE download_items SET file_path = ? WHERE id = ?")
            .bind(&paths[0])
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(Self::internal)?;
        tx.commit().await.map_err(Self::internal)
    }

    async fn download_remove(&self, id: &str) -> Result<(), AppError> {
        let hash: Option<String> =
            sqlx::query_scalar("SELECT torrent_hash FROM download_items WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(Self::internal)?;
        let Some(hash) = hash else {
            return Ok(());
        };

        let mut tx = self.pool.begin().await.map_err(Self::internal)?;
        sqlx::query("DELETE FROM download_items WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(Self::internal)?;

        // Keep the torrent task while another item or episode mapping still
        // needs it: removing one episode must not remove the other's media.
        let remaining: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM download_items WHERE torrent_hash = ?")
                .bind(&hash)
                .fetch_one(&mut *tx)
                .await
                .map_err(Self::internal)?;
        let mapped: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM file_mappings WHERE torrent_hash = ?")
                .bind(&hash)
                .fetch_one(&mut *tx)
                .await
                .map_err(Self::internal)?;
        if remaining == 0 && mapped == 0 {
            sqlx::query("DELETE FROM torrent_tasks WHERE hash = ?")
                .bind(&hash)
                .execute(&mut *tx)
                .await
                .map_err(Self::internal)?;
        }
        tx.commit().await.map_err(Self::internal)
    }

    async fn prefetch_upsert(&self, entry: &PrefetchEntry) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO prefetch_entries (media_id, requested_for, priority, state) \
             VALUES (?, ?, ?, ?) \
             ON CONFLICT(media_id) DO UPDATE SET requested_for = excluded.requested_for, \
             priority = excluded.priority, state = excluded.state",
        )
        .bind(&entry.media_id)
        .bind(&entry.requested_for)
        .bind(entry.priority)
        .bind(&entry.state)
        .execute(&self.pool)
        .await
        .map_err(Self::internal)?;
        Ok(())
    }

    async fn cleanup_eligible(&self) -> Result<Vec<CleanupEntry>, AppError> {
        let rows = sqlx::query(
            "SELECT media_id, eligible_at, state, outcome FROM cleanup_entries \
             WHERE state = 'pending' AND eligible_at <= ? ORDER BY eligible_at ASC",
        )
        .bind(now_iso())
        .fetch_all(&self.pool)
        .await
        .map_err(Self::internal)?;

        Ok(rows
            .iter()
            .map(|row| CleanupEntry {
                media_id: row.get("media_id"),
                eligible_at: row.get("eligible_at"),
                state: row.get("state"),
                outcome: row.get("outcome"),
            })
            .collect())
    }

    async fn source_for_media(&self, media_id: &str) -> Result<Option<String>, AppError> {
        sqlx::query_scalar::<_, String>("SELECT source FROM item_sources WHERE media_id = ?")
            .bind(media_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(Self::internal)
    }

    async fn set_item_source(&self, media_id: &str, source: &str) -> Result<(), AppError> {
        if media_id.trim().is_empty() {
            return Err(AppError::new(
                AppErrorCode::InvalidRequest,
                "A media id is required.",
                false,
            ));
        }
        // Rust repeats the renderer's validation: this is the trust boundary.
        let source = validate_item_source(source)?;
        let name = item_source_name(&source);
        let mut tx = self.pool.begin().await.map_err(Self::internal)?;
        sqlx::query(
            "INSERT INTO item_sources (media_id, source, source_name) VALUES (?, ?, ?) \
             ON CONFLICT(media_id) DO UPDATE SET source = excluded.source, \
             source_name = excluded.source_name",
        )
        .bind(media_id)
        .bind(&source)
        .bind(&name)
        .execute(&mut *tx)
        .await
        .map_err(Self::internal)?;
        // Tie the source to any download already tracked for this hash, so the
        // existing verification/cleanup gates can see the media id (R-01).
        sqlx::query(
            "UPDATE download_items SET media_id = ? WHERE torrent_hash IN \
             (SELECT hash FROM torrent_tasks WHERE magnet_or_source = ?)",
        )
        .bind(media_id)
        .bind(&source)
        .execute(&mut *tx)
        .await
        .map_err(Self::internal)?;
        if let Some(hash) = item_source_torrent_hash(&source) {
            sqlx::query(
                "UPDATE download_items SET media_id = ? WHERE lower(torrent_hash) = lower(?)",
            )
            .bind(media_id)
            .bind(hash)
            .execute(&mut *tx)
            .await
            .map_err(Self::internal)?;
        }
        tx.commit().await.map_err(Self::internal)?;
        Ok(())
    }

    async fn wanted_add(
        &self,
        title: &str,
        year: Option<i64>,
        kind: &str,
        id: &str,
    ) -> Result<(), AppError> {
        // A catalog pick starts unavailable with no genres and no file. The id
        // is caller-supplied and stable; ON CONFLICT keeps repeat picks a no-op.
        let now = now_iso();
        sqlx::query(
            "INSERT INTO media_items \
             (id, kind, title, year, genres, availability_state, created_at, updated_at) \
             VALUES (?, ?, ?, ?, '[]', 'missing-file', ?, ?) \
             ON CONFLICT(id) DO NOTHING",
        )
        .bind(id)
        .bind(kind)
        .bind(title)
        .bind(year)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(Self::internal)?;
        Ok(())
    }
}

impl SqliteLibraryRepository {
    async fn load_roots(&self) -> Result<Vec<LibraryRoot>, AppError> {
        let rows = sqlx::query(
            "SELECT id, display_name, item_count, status, last_scan_at, error_message, path \
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
                path: row.get("path"),
            })
            .collect())
    }

    async fn load_counts(&self) -> Result<LibraryCounts, AppError> {
        let rows = sqlx::query("SELECT kind, COUNT(*) AS total FROM media_items GROUP BY kind")
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

/// Guarantees the settings document only stores real colours: accepted hex is
/// lower-cased and anything else is assigned from the palette by position.
fn normalise_profiles(profiles: &mut [Profile]) {
    for (index, profile) in profiles.iter_mut().enumerate() {
        let value = profile.avatar_color.trim();
        profile.avatar_color = if is_hex_colour(value) {
            value.to_ascii_lowercase()
        } else {
            PROFILE_PALETTE[index % PROFILE_PALETTE.len()].to_string()
        };
    }
}

fn is_hex_colour(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 7 && bytes[0] == b'#' && bytes[1..].iter().all(u8::is_ascii_hexdigit)
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
        "available" => MediaAvailability::Available,
        "missing-file" => MediaAvailability::MissingFile {
            last_seen_at: last_seen.clone(),
        },
        "offline" => MediaAvailability::Offline,
        // Unknown/legacy values (including a catalog pick's former
        // 'unavailable' state) must never be presented as playable.
        _ => MediaAvailability::MissingFile { last_seen_at: None },
    }
}

fn progress(row: &MediaItemRow) -> Option<PlaybackProgress> {
    row.progress_updated_at
        .as_ref()
        .map(|updated_at| PlaybackProgress {
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
        imdb_rating: None,
        vote_count: None,
        age_rating: None,
    }
}

fn parse_list(raw: &str) -> Vec<String> {
    serde_json::from_str(raw).unwrap_or_default()
}

fn root_status_to_str(value: LibraryRootStatus) -> &'static str {
    match value {
        LibraryRootStatus::Ok => "ok",
        LibraryRootStatus::Scanning => "scanning",
        LibraryRootStatus::Offline => "offline",
        LibraryRootStatus::Error => "error",
    }
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

/// Map the live acquisition state onto the older display-only task state used
/// by `DownloadsScreen`. There is no distinct "playing" legacy state; an
/// owned, ready file remains available to its owner.
fn legacy_download_state(value: &str) -> LegacyDownloadState {
    match download_state_from_str(value) {
        DownloadState::Downloading => LegacyDownloadState::Downloading,
        DownloadState::Error => LegacyDownloadState::Failed,
        DownloadState::Ready
        | DownloadState::Playing
        | DownloadState::Watched
        | DownloadState::CleanupPending => LegacyDownloadState::Available,
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
        profiles: vec![Profile {
            id: uuid::Uuid::new_v4().to_string(),
            name: "You".to_string(),
            avatar_color: "#E50914".to_string(),
            created_at: now_iso(),
        }],
    }
}

/// Task 2: Defender verification columns on `download_items`. This block owns
/// only `defender_state`/`defender_detail`; Task 1's `verification_*` columns
/// and methods are deliberately untouched.
impl SqliteLibraryRepository {
    /// Persist the Defender result for one download. `detail` is a host-only
    /// diagnostic; callers must pass a path-free reason for display surfaces.
    pub async fn set_defender_verification(
        &self,
        id: &str,
        state: &str,
        detail: Option<&str>,
    ) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE download_items SET defender_state = ?, defender_detail = ? WHERE id = ?",
        )
        .bind(state)
        .bind(detail)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(Self::internal)?;
        Ok(())
    }

    /// Persist a Defender result using the media id the host already owns.
    /// A library import has no download row and needs no scan; that is a no-op
    /// rather than an error, so the host can call one route for every play.
    pub async fn set_defender_verification_for_media(
        &self,
        media_id: &str,
        state: &str,
        detail: Option<&str>,
    ) -> Result<(), AppError> {
        let id: Option<String> =
            sqlx::query_scalar("SELECT id FROM download_items WHERE media_id = ? LIMIT 1")
                .bind(media_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(Self::internal)?;
        if let Some(id) = id {
            self.set_defender_verification(&id, state, detail).await?;
        }
        Ok(())
    }

    /// Defender state for a media id. `None` means no download row at all
    /// (a library import). A download row with a NULL state is refused by
    /// `refuse_unscanned_download`, not mistaken for a clean scan.
    pub async fn defender_state_for_media(
        &self,
        media_id: &str,
    ) -> Result<Option<String>, AppError> {
        sqlx::query_scalar::<_, String>(
            "SELECT defender_state FROM download_items WHERE media_id = ? LIMIT 1",
        )
        .bind(media_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(Self::internal)
    }

    /// Refuse a downloaded item until Defender reports clean. A library import
    /// (no download row) is unaffected; every other state fails closed. The
    /// reason is a fixed display string, never a path or a scanner detail.
    async fn refuse_unscanned_download(&self, media_id: &str) -> Result<(), AppError> {
        // Read the first BAD row, not an arbitrary row: one flagged download
        // must hide a clean sibling.
        let state = sqlx::query_scalar::<_, String>(
            "SELECT COALESCE(defender_state, 'not_scanned') FROM download_items \
             WHERE media_id = ? AND (defender_state IS NULL OR defender_state <> 'clean') \
             LIMIT 1",
        )
        .bind(media_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(Self::internal)?;
        let reason = match state.as_deref() {
            None => return Ok(()),
            Some("threat") => "Windows Defender flagged this file.",
            Some("unavailable") => "Windows Defender could not scan this file.",
            Some(_) => "This file has not been scanned yet.",
        };
        Err(AppError::new(AppErrorCode::MediaUnsupported, reason, false))
    }
}

#[cfg(test)]
mod defender_tests {
    use super::*;
    use crate::sqlite::connect_path;
    use app_core::application::LibraryService;

    fn temp_db(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("mh-defender-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir.join("library.db")
    }

    async fn insert_item(repo: &SqliteLibraryRepository, id: &str, media_id: &str) {
        let pool = repo.pool();
        sqlx::query(
            "INSERT INTO torrent_tasks (hash, magnet_or_source, state, save_path, created_at, updated_at) \
             VALUES (?, ?, 'ready', '/downloads', '', '')",
        )
        .bind(format!("hash-{id}"))
        .bind("magnet:?xt=urn:btih:test")
        .execute(pool)
        .await
        .expect("insert torrent task");
        sqlx::query(
            "INSERT INTO download_items (id, torrent_hash, media_id, file_path, state, created_at) \
             VALUES (?, ?, ?, ?, 'ready', '')",
        )
        .bind(id)
        .bind(format!("hash-{id}"))
        .bind(media_id)
        .bind(format!("/downloads/{id}.mkv"))
        .execute(pool)
        .await
        .expect("insert download item");
    }

    #[tokio::test]
    async fn defender_state_defaults_to_not_scanned_and_is_settable() {
        let db = temp_db("default");
        let pool = connect_path(&db).await.expect("connect and migrate");
        let repo = SqliteLibraryRepository::new(pool);
        insert_item(&repo, "d-def", "m-def").await;

        assert_eq!(
            repo.defender_state_for_media("m-def").await.expect("read"),
            Some("not_scanned".to_string()),
            "a fresh download fails closed"
        );
        repo.set_defender_verification("d-def", "threat", None)
            .await
            .expect("set");
        assert_eq!(
            repo.defender_state_for_media("m-def").await.expect("read"),
            Some("threat".to_string())
        );

        // No mapping for an unknown media id fails closed too.
        assert_eq!(
            repo.defender_state_for_media("m-unknown")
                .await
                .expect("read"),
            None
        );
    }

    #[tokio::test]
    async fn threat_is_never_playable_or_visible_even_when_probe_is_clean() {
        let db = temp_db("playback");
        let pool = connect_path(&db).await.expect("connect and migrate");
        let repo = SqliteLibraryRepository::new(pool);
        let now = now_iso();
        sqlx::query(
            "INSERT INTO media_items (id, kind, title, created_at, updated_at) \
             VALUES ('m-threat', 'movie', 'title', ?, ?)",
        )
        .bind(&now)
        .bind(&now)
        .execute(repo.pool())
        .await
        .expect("insert media");
        sqlx::query(
            "INSERT INTO torrent_tasks (hash, magnet_or_source, state, save_path, created_at, updated_at) \
             VALUES ('h-threat', 'magnet:?xt=urn:btih:test', 'ready', '/downloads', ?, ?)",
        )
        .bind(&now)
        .bind(&now)
        .execute(repo.pool())
        .await
        .expect("insert torrent task");
        sqlx::query(
            "INSERT INTO download_items \
             (id, torrent_hash, media_id, file_path, state, created_at, verification_state, defender_state) \
             VALUES ('d-threat', 'h-threat', 'm-threat', '/downloads/m-threat.mkv', 'ready', ?, 'clean', 'threat')",
        )
        .bind(&now)
        .execute(repo.pool())
        .await
        .expect("insert download item");

        let service = LibraryService::new(SqliteLibraryRepository::new(repo.pool().clone()));

        // A probe-clean but Defender-flagged download must not present a normal
        // detail view, and the refusal must never leak a path.
        let detail_error = service
            .detail("m-threat")
            .await
            .expect_err("a Defender threat must not show a detail view");
        assert_eq!(detail_error.code, AppErrorCode::MediaUnsupported);
        assert!(detail_error.message.contains("Defender"));
        assert!(!detail_error.message.contains("/downloads"));

        let error = service
            .playback_start(&PlaybackStartRequest {
                media_id: "m-threat".to_string(),
                start_position_seconds: None,
            })
            .await
            .expect_err("a Defender threat must never be playable");
        assert_eq!(error.code, AppErrorCode::MediaUnsupported);
        assert!(error.message.contains("Defender"));
        assert!(!error.message.contains("/downloads"));
    }

    #[tokio::test]
    async fn one_flagged_row_hides_a_clean_sibling() {
        let db = temp_db("siblings");
        let pool = connect_path(&db).await.expect("connect and migrate");
        let repo = SqliteLibraryRepository::new(pool);
        let now = now_iso();
        sqlx::query(
            "INSERT INTO media_items (id, kind, title, created_at, updated_at) \
             VALUES ('m-two', 'movie', 'title', ?, ?)",
        )
        .bind(&now)
        .bind(&now)
        .execute(repo.pool())
        .await
        .expect("insert media");
        for (hash, id, defender) in [
            ("h-clean", "d-clean", "clean"),
            ("h-bad", "d-bad", "threat"),
        ] {
            sqlx::query(
                "INSERT INTO torrent_tasks (hash, magnet_or_source, state, save_path, created_at, updated_at) \
                 VALUES (?, 'magnet:?xt=urn:btih:test', 'ready', '/downloads', ?, ?)",
            )
            .bind(hash)
            .bind(&now)
            .bind(&now)
            .execute(repo.pool())
            .await
            .expect("insert torrent task");
            sqlx::query(
                "INSERT INTO download_items \
                 (id, torrent_hash, media_id, file_path, state, created_at, verification_state, defender_state) \
                 VALUES (?, ?, 'm-two', '/downloads/x.mkv', 'ready', ?, 'clean', ?)",
            )
            .bind(id)
            .bind(hash)
            .bind(&now)
            .bind(defender)
            .execute(repo.pool())
            .await
            .expect("insert download item");
        }

        assert!(!repo.gates_clean("m-two").await.expect("gates"));
        let service = LibraryService::new(SqliteLibraryRepository::new(repo.pool().clone()));
        let error = service
            .playback_start(&PlaybackStartRequest {
                media_id: "m-two".to_string(),
                start_position_seconds: None,
            })
            .await
            .expect_err("a flagged sibling must hide a clean one");
        assert_eq!(error.code, AppErrorCode::MediaUnsupported);
        assert!(error.message.contains("Defender"));
    }
}

// ---------------------------------------------------------------------------
// Downloaded-media verification gate (security hardening, Task 1).
// ---------------------------------------------------------------------------

impl SqliteLibraryRepository {
    /// Verification state for the download that owns `media_id`. `None` means
    /// no download row is associated; `refuse_unverified` still refuses when a
    /// download row exists with a NULL state.
    pub async fn verification_state_for_media(
        &self,
        media_id: &str,
    ) -> Result<Option<String>, AppError> {
        sqlx::query_scalar(
            "SELECT verification_state FROM download_items WHERE media_id = ? LIMIT 1",
        )
        .bind(media_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(Self::internal)
    }

    /// A completed download makes its bound wanted item playable. Only the
    /// ready path calls this; a missing-file item stays missing otherwise.
    pub async fn set_media_available(&self, media_id: &str) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE media_items SET availability_state = 'available', \
             availability_last_seen = ? WHERE id = ?",
        )
        .bind(now_iso())
        .bind(media_id)
        .execute(&self.pool)
        .await
        .map_err(Self::internal)?;
        Ok(())
    }

    /// Record a probe result for a download. `detail` is metadata only and must
    /// never contain a filesystem path.
    pub async fn set_verification_state(
        &self,
        download_id: &str,
        state: &str,
        detail: Option<&str>,
    ) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE download_items SET verification_state = ?, verification_detail = ? WHERE id = ?",
        )
        .bind(state)
        .bind(detail)
        .bind(download_id)
        .execute(&self.pool)
        .await
        .map_err(Self::internal)?;
        Ok(())
    }

    /// True only when a download row is associated with this media id. Used to
    /// distinguish a library import (allowed) from a download with a NULL gate
    /// state (refused).
    pub async fn has_download_row(&self, media_id: &str) -> Result<bool, AppError> {
        let found = sqlx::query("SELECT 1 FROM download_items WHERE media_id = ? LIMIT 1")
            .bind(media_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(Self::internal)?;
        Ok(found.is_some())
    }

    /// Refuse a downloaded item until the probe has parsed it as media. The
    /// message is a fixed display string: no path reaches the renderer.
    async fn refuse_unverified(&self, media_id: &str) -> Result<(), AppError> {
        // Only bad rows are returned. A clean sibling cannot hide a flagged one.
        let state = sqlx::query_scalar::<_, String>(
            "SELECT COALESCE(verification_state, 'scanning') FROM download_items \
             WHERE media_id = ? \
             AND (verification_state IS NULL OR verification_state <> 'clean') LIMIT 1",
        )
        .bind(media_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(Self::internal)?;
        if let Some(reason) = playback_block_reason(state.as_deref()) {
            return Err(AppError::new(AppErrorCode::MediaUnsupported, reason, false));
        }
        Ok(())
    }

    /// Both gates are clean only when every download row bound to the item has
    /// passed. Used before an item is shown as playable.
    pub async fn gates_clean(&self, media_id: &str) -> Result<bool, AppError> {
        let bad: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM download_items WHERE media_id = ? \
             AND (verification_state IS NULL OR verification_state <> 'clean' \
             OR defender_state IS NULL OR defender_state <> 'clean')",
        )
        .bind(media_id)
        .fetch_one(&self.pool)
        .await
        .map_err(Self::internal)?;
        Ok(bad == 0)
    }
}

/// Map a stored verification state to a display-only refusal reason. `None`
/// is allowed only for a library import (no download row) — callers must check
/// `has_download_row` before treating it as clean. Every other unknown value
/// fails closed.
pub fn playback_block_reason(state: Option<&str>) -> Option<&'static str> {
    match state {
        None | Some("clean") => None,
        Some("scanning") => Some("This download is still being checked."),
        Some("suspicious") => Some("This download failed verification and cannot be played."),
        Some("unavailable") => Some("This download could not be verified and cannot be played."),
        Some(_) => Some("This download has not passed verification yet."),
    }
}

#[cfg(test)]
mod verification_tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::sqlite::pool::connect_path;
    use app_core::application::LibraryService;

    async fn seeded_db(tag: &str) -> (SqliteLibraryRepository, std::path::PathBuf) {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("mh-verify-{tag}-{nanos}"));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let db = dir.join("library.db");
        let pool = connect_path(&db).await.expect("connect and migrate");
        (SqliteLibraryRepository::new(pool), db)
    }

    async fn seed_media(repo: &SqliteLibraryRepository, media_id: &str) {
        let now = now_iso();
        sqlx::query(
            "INSERT INTO media_items (id, kind, title, created_at, updated_at) \
             VALUES (?, 'movie', ?, ?, ?)",
        )
        .bind(media_id)
        .bind(format!("title-{media_id}"))
        .bind(&now)
        .bind(&now)
        .execute(repo.pool())
        .await
        .expect("insert media");
    }

    async fn seed_download(repo: &SqliteLibraryRepository, media_id: &str, state: &str) {
        seed_media(repo, media_id).await;
        let now = now_iso();
        sqlx::query(
            "INSERT INTO torrent_tasks (hash, magnet_or_source, state, progress, save_path, created_at, updated_at) \
             VALUES (?, 'magnet:?xt=urn:btih:test', 'ready', 1.0, '/downloads', ?, ?)",
        )
        .bind(format!("hash-{media_id}"))
        .bind(&now)
        .bind(&now)
        .execute(repo.pool())
        .await
        .expect("insert torrent task");
        sqlx::query(
            "INSERT INTO download_items (id, torrent_hash, media_id, file_path, state, created_at, verification_state) \
             VALUES (?, ?, ?, ?, 'ready', ?, ?)",
        )
        .bind(format!("d-{media_id}"))
        .bind(format!("hash-{media_id}"))
        .bind(media_id)
        .bind(format!("/downloads/{media_id}.mkv"))
        .bind(&now)
        .bind(state)
        .execute(repo.pool())
        .await
        .expect("insert download item");
    }

    fn service(repo: &SqliteLibraryRepository) -> LibraryService<SqliteLibraryRepository> {
        LibraryService::new(SqliteLibraryRepository::new(repo.pool().clone()))
    }

    #[test]
    fn refusal_reasons_are_display_only_and_never_a_path() {
        assert_eq!(playback_block_reason(None), None, "library import allowed");
        assert_eq!(playback_block_reason(Some("clean")), None);
        for state in ["pending", "scanning", "suspicious", "unavailable", "weird"] {
            let reason = playback_block_reason(Some(state)).expect("non-clean is refused");
            assert!(!reason.contains('/'), "no path in reason: {reason}");
            assert!(!reason.contains('\\'), "no path in reason: {reason}");
        }
    }

    #[tokio::test]
    async fn suspicious_download_is_refused_by_detail_and_playback_start() {
        let (repo, _db) = seeded_db("suspicious").await;
        seed_download(&repo, "m-bad", "suspicious").await;

        let detail_error = service(&repo)
            .detail("m-bad")
            .await
            .expect_err("detail refused");
        assert_eq!(detail_error.code, AppErrorCode::MediaUnsupported);
        assert!(detail_error.message.contains("failed verification"));
        assert!(!detail_error.message.contains("/downloads"));

        let playback_error = service(&repo)
            .playback_start(&PlaybackStartRequest {
                media_id: "m-bad".to_string(),
                start_position_seconds: None,
            })
            .await
            .expect_err("playback refused");
        assert_eq!(playback_error.code, AppErrorCode::MediaUnsupported);
        assert!(!playback_error.message.contains("/downloads"));
    }

    #[tokio::test]
    async fn every_non_clean_state_fails_closed_through_the_gate() {
        let (repo, _db) = seeded_db("states").await;
        for (index, state) in [
            "pending",
            "scanning",
            "suspicious",
            "unavailable",
            "unknown",
        ]
        .iter()
        .enumerate()
        {
            let media_id = format!("m-{index}");
            seed_download(&repo, &media_id, state).await;
            let error = service(&repo).detail(&media_id).await.unwrap_err();
            assert_eq!(error.code, AppErrorCode::MediaUnsupported, "state: {state}");
        }
    }

    #[tokio::test]
    async fn clean_download_and_library_import_are_allowed() {
        let (repo, _db) = seeded_db("allowed").await;
        seed_download(&repo, "m-clean", "clean").await;
        // Task 2: a downloaded file needs BOTH gates clean; a library import
        // (m-library) has no download row and stays scanner-trusted.
        repo.set_defender_verification("d-m-clean", "clean", None)
            .await
            .expect("set defender clean");
        seed_media(&repo, "m-library").await;
        assert!(service(&repo).detail("m-clean").await.is_ok());
        assert!(service(&repo).detail("m-library").await.is_ok());
        // Task 2 added a second (Defender) gate on playback_start. A probe-clean
        // download only plays once that gate is clean too; seed that state here.
        sqlx::query("UPDATE download_items SET defender_state = 'clean' WHERE id = 'd-m-clean'")
            .execute(repo.pool())
            .await
            .expect("set defender clean for the integrated test");
        let session = service(&repo)
            .playback_start(&PlaybackStartRequest {
                media_id: "m-clean".to_string(),
                start_position_seconds: None,
            })
            .await
            .expect("clean download plays");
        assert_eq!(session.media.id, "m-clean");
    }

    #[tokio::test]
    async fn verification_state_starts_pending_and_can_flip_to_clean_then_back() {
        let (repo, _db) = seeded_db("lifecycle").await;
        seed_download(&repo, "m-life", "pending").await;
        assert_eq!(
            repo.verification_state_for_media("m-life")
                .await
                .unwrap()
                .as_deref(),
            Some("pending")
        );
        assert_eq!(
            service(&repo).detail("m-life").await.unwrap_err().code,
            AppErrorCode::MediaUnsupported
        );

        repo.set_verification_state("d-m-life", "clean", None)
            .await
            .expect("record clean");
        // Task 2: detail now needs both gates clean; set the Defender half too.
        repo.set_defender_verification("d-m-life", "clean", None)
            .await
            .expect("record defender clean");
        assert!(service(&repo).detail("m-life").await.is_ok());

        // A later failure must immediately revoke playability: no stale trust.
        repo.set_verification_state("d-m-life", "unavailable", Some("probe: unavailable"))
            .await
            .expect("record unavailable");
        assert_eq!(
            service(&repo).detail("m-life").await.unwrap_err().code,
            AppErrorCode::MediaUnsupported
        );
    }
}

#[cfg(test)]
mod wanted_tests {
    use super::*;
    use crate::sqlite::connect_path;

    #[tokio::test]
    async fn wanted_add_is_idempotent_and_creates_no_file() {
        let dir = std::env::temp_dir().join(format!(
            "mh-wanted-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let pool = connect_path(&dir.join("library.db"))
            .await
            .expect("connect and migrate");
        let repo = SqliteLibraryRepository::new(pool.clone());

        let id = "catalog-movie-0123456789abcdef0123456789abcdef";
        repo.wanted_add("A Wanted Film", Some(2024), "movie", id)
            .await
            .expect("first insert");
        // A repeat pick with the same stable id must be a no-op.
        repo.wanted_add("A Wanted Film", Some(2024), "movie", id)
            .await
            .expect("repeat insert");

        let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM media_items WHERE id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .expect("count media");
        assert_eq!(rows, 1, "idempotent: exactly one media row");

        let row = sqlx::query(
            "SELECT kind, title, year, genres, availability_state, created_at, updated_at \
             FROM media_items WHERE id = ?",
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .expect("read media row");
        assert_eq!(row.get::<String, _>("kind"), "movie");
        assert_eq!(row.get::<String, _>("title"), "A Wanted Film");
        assert_eq!(row.get::<Option<i64>, _>("year"), Some(2024));
        assert_eq!(row.get::<String, _>("genres"), "[]");
        assert_eq!(row.get::<String, _>("availability_state"), "missing-file");
        assert!(!row.get::<String, _>("created_at").is_empty());
        assert!(!row.get::<String, _>("updated_at").is_empty());

        let files: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM media_files WHERE media_id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .expect("count files");
        assert_eq!(files, 0, "a wanted item has no media file");

        let detail = repo.detail(id).await.expect("wanted detail");
        match detail {
            MediaDetail::Movie(movie) => assert!(
                matches!(movie.availability, MediaAvailability::MissingFile { .. }),
                "a file-less catalog pick must not be offered as available"
            ),
            other => panic!("wanted item kind changed: {other:?}"),
        }

        // A completed download flips the same wanted row to available so the
        // backend playback gate can pass; no media_files row is fabricated.
        repo.set_media_available(id).await.expect("mark available");
        let state: String =
            sqlx::query_scalar("SELECT availability_state FROM media_items WHERE id = ?")
                .bind(id)
                .fetch_one(&pool)
                .await
                .expect("read availability");
        assert_eq!(state, "available");
        let files: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM media_files WHERE media_id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .expect("count files");
        assert_eq!(files, 0, "availability does not invent a media file");

        pool.close().await;
        std::fs::remove_dir_all(&dir).ok();
    }
}
