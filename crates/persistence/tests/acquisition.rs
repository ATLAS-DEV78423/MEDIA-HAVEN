//! Acquisition records: one torrent can map to several episode files, and a
//! migration added later must stay idempotent on a reopened database.

use app_core::domain::*;
use app_core::ports::LibraryRepository;
use media_persistence::sqlite::{connect_path, SqliteLibraryRepository};

fn temp_db_path() -> (std::path::PathBuf, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("mediahaven-acq-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    (dir.clone(), dir.join("library.db"))
}

#[tokio::test]
async fn acquisition_rows_restore_the_item_and_its_operator_source() {
    let (dir, db) = temp_db_path();
    let pool = connect_path(&db).await.expect("connect and migrate");
    let repo = SqliteLibraryRepository::new(pool.clone());
    let now = "2026-11-24T00:00:00+00:00";
    let source = "magnet:?xt=urn:btih:RESTORE&dn=episode";

    sqlx::query(
        "INSERT INTO torrent_tasks \
         (hash, magnet_or_source, state, progress, save_path, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("hash-restore")
    .bind(source)
    .bind("downloading")
    .bind(0.25)
    .bind("/downloads")
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await
    .expect("insert torrent task");
    sqlx::query(
        "INSERT INTO download_items \
         (id, torrent_hash, media_id, file_path, state, created_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind("hash-restore")
    .bind("hash-restore")
    .bind(Some("m-restore"))
    .bind("/downloads/hash-restore")
    .bind("downloading")
    .bind(now)
    .execute(&pool)
    .await
    .expect("insert download item");

    let rows = repo.acquisition_rows().await.expect("acquisition_rows");
    assert_eq!(rows.len(), 1);
    let (item, stored_source) = &rows[0];
    assert_eq!(item.id, "hash-restore");
    assert_eq!(item.media_id.as_deref(), Some("m-restore"));
    assert_eq!(item.state, DownloadState::Downloading);
    assert!((item.progress - 0.25).abs() < f64::EPSILON);
    assert_eq!(stored_source, source);

    drop(repo);
    drop(pool);
    // SQLite may keep the file handle briefly on Windows; cleanup is best effort.
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn download_remove_deletes_the_persisted_row() {
    let (dir, db) = temp_db_path();
    let pool = connect_path(&db).await.expect("connect and migrate");
    let repo = SqliteLibraryRepository::new(pool.clone());
    let item = DownloadItem {
        id: "d-remove".to_string(),
        torrent_hash: "h-remove".to_string(),
        media_id: Some("m-remove".to_string()),
        state: DownloadState::Downloading,
        progress: 0.1,
    };
    repo.download_add(
        &item,
        "/downloads/h-remove",
        "magnet:?xt=urn:btih:REMOVE",
        "/downloads",
    )
    .await
    .expect("download_add");
    assert!(repo
        .download_status("d-remove")
        .await
        .expect("status")
        .is_some());

    repo.download_remove("d-remove")
        .await
        .expect("download_remove");
    assert!(repo
        .download_status("d-remove")
        .await
        .expect("status after remove")
        .is_none());

    drop(repo);
    drop(pool);
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn multi_episode_file_mappings_persist_independently() {
    let (dir, db) = temp_db_path();
    let pool = connect_path(&db).await.expect("connect and migrate");
    let now = "2026-11-20T00:00:00+00:00";

    sqlx::query(
        "INSERT INTO torrent_tasks \
         (hash, magnet_or_source, state, progress, save_path, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("hash-1")
    .bind("magnet:?xt=urn:btih:test")
    .bind("ready")
    .bind(1.0)
    .bind("/downloads/show")
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await
    .expect("insert torrent task");

    for (path, media, episode) in [
        ("/downloads/show/e01.mkv", "media-e1", 1_i64),
        ("/downloads/show/e02.mkv", "media-e2", 2_i64),
    ] {
        sqlx::query(
            "INSERT INTO download_items \
             (id, torrent_hash, media_id, file_path, state, created_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(format!("item-{episode}"))
        .bind("hash-1")
        .bind(media)
        .bind(path)
        .bind("ready")
        .bind(now)
        .execute(&pool)
        .await
        .expect("insert download item");

        sqlx::query(
            "INSERT INTO file_mappings (file_path, torrent_hash, media_id, episode_no) \
             VALUES (?, ?, ?, ?)",
        )
        .bind(path)
        .bind("hash-1")
        .bind(media)
        .bind(episode)
        .execute(&pool)
        .await
        .expect("insert file mapping");
    }

    let mapped: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM file_mappings WHERE torrent_hash = ?")
            .bind("hash-1")
            .fetch_one(&pool)
            .await
            .expect("count file mappings");
    assert_eq!(mapped, 2, "both episodes of the torrent must be recorded");

    sqlx::query("DELETE FROM file_mappings WHERE file_path = ?")
        .bind("/downloads/show/e01.mkv")
        .execute(&pool)
        .await
        .expect("delete one mapping");

    let survivor: Option<String> =
        sqlx::query_scalar("SELECT media_id FROM file_mappings WHERE episode_no = ?")
            .bind(2_i64)
            .fetch_optional(&pool)
            .await
            .expect("read surviving mapping");
    assert_eq!(
        survivor.as_deref(),
        Some("media-e2"),
        "removing one episode must not remove the other's media"
    );

    pool.close().await;
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn acquisition_migration_is_idempotent() {
    let (dir, db) = temp_db_path();

    let first = connect_path(&db).await.expect("first migrate");
    let expected: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(&first)
        .await
        .expect("read first migration bookkeeping");
    first.close().await;

    let second = connect_path(&db).await.expect("second migrate");
    let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(&second)
        .await
        .expect("read migration bookkeeping");
    assert_eq!(
        applied, expected,
        "migrations were re-applied instead of skipped"
    );
    for table in [
        "torrent_tasks",
        "download_items",
        "file_mappings",
        "prefetch_entries",
        "cleanup_entries",
    ] {
        let found = sqlx::query("SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?")
            .bind(table)
            .fetch_optional(&second)
            .await
            .expect("query sqlite_master");
        assert!(found.is_some(), "{table} table was not created");
    }

    second.close().await;
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn download_update_follows_engine_progress_without_erasing_playback_lifecycle() {
    let (dir, db) = temp_db_path();
    let pool = connect_path(&db).await.expect("connect and migrate");
    let repo = SqliteLibraryRepository::new(pool);
    let mut item = DownloadItem {
        id: "d-update".to_string(),
        torrent_hash: "h-update".to_string(),
        media_id: Some("m-update".to_string()),
        state: DownloadState::Downloading,
        progress: 0.25,
    };
    repo.download_add(
        &item,
        "/downloads/show/e01.mkv",
        "magnet:?xt=urn:btih:update",
        "/downloads/show",
    )
    .await
    .expect("download_add");

    // A poll after the engine completes must update both tables.
    item.state = DownloadState::Ready;
    item.progress = 1.0;
    repo.download_update(&item).await.expect("download_update");
    let status = repo
        .download_status("d-update")
        .await
        .expect("status")
        .expect("row exists");
    assert_eq!(status.state, DownloadState::Ready);
    assert_eq!(status.progress, 1.0);
    let task_progress: f64 =
        sqlx::query_scalar("SELECT progress FROM torrent_tasks WHERE hash = ?")
            .bind("h-update")
            .fetch_one(repo.pool())
            .await
            .expect("task progress");
    assert_eq!(task_progress, 1.0);

    // Once playback/cleanup owns the lifecycle state, a later engine poll
    // must not reset it to Ready and make the file look newly available.
    sqlx::query("UPDATE download_items SET state = 'playing' WHERE id = ?")
        .bind("d-update")
        .execute(repo.pool())
        .await
        .expect("set lifecycle state");
    repo.download_update(&item).await.expect("download_update");
    let status = repo
        .download_status("d-update")
        .await
        .expect("status")
        .expect("row exists");
    assert_eq!(status.state, DownloadState::Playing);
    assert_eq!(status.progress, 1.0);

    // A poll for a task with no persisted row is ignored; it must not create
    // a phantom acquisition in SQLite.
    let orphan = DownloadItem {
        id: "d-missing".to_string(),
        torrent_hash: "h-missing".to_string(),
        media_id: None,
        state: DownloadState::Ready,
        progress: 1.0,
    };
    repo.download_update(&orphan)
        .await
        .expect("unknown download is ignored");
    assert!(repo
        .download_status("d-missing")
        .await
        .expect("status after ignored update")
        .is_none());

    repo.pool().close().await;
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn download_set_paths_replaces_placeholders_without_touching_episode_mappings() {
    let (dir, db) = temp_db_path();
    let pool = connect_path(&db).await.expect("connect and migrate");
    let repo = SqliteLibraryRepository::new(pool);
    let item = DownloadItem {
        id: "d-paths".to_string(),
        torrent_hash: "h-paths".to_string(),
        media_id: Some("m-paths".to_string()),
        state: DownloadState::Ready,
        progress: 1.0,
    };
    repo.download_add(
        &item,
        "/downloads/h-paths",
        "magnet:?xt=urn:btih:paths",
        "/downloads",
    )
    .await
    .expect("download_add with placeholder path");
    sqlx::query(
        "INSERT INTO file_mappings (file_path, torrent_hash, media_id, episode_no) \
         VALUES ('/downloads/show/e01.mkv', 'h-paths', 'm-paths', 1)",
    )
    .execute(repo.pool())
    .await
    .expect("manual episode mapping");

    let real = vec![
        "/downloads/show/e01.mkv".to_string(),
        "/downloads/show/e02.mkv".to_string(),
    ];
    repo.download_set_paths("d-paths", &real)
        .await
        .expect("replace paths");

    assert_eq!(
        repo.media_source_path("m-paths").await.expect("source"),
        Some("/downloads/show/e01.mkv".to_string()),
        "playback must resolve the real file, not the placeholder"
    );
    let mapping_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM file_mappings WHERE torrent_hash = ?")
            .bind("h-paths")
            .fetch_one(repo.pool())
            .await
            .expect("mapping count");
    assert_eq!(
        mapping_count, 2,
        "placeholder is gone; both real files mapped"
    );
    let episode: Option<i64> =
        sqlx::query_scalar("SELECT episode_no FROM file_mappings WHERE file_path = ?")
            .bind("/downloads/show/e01.mkv")
            .fetch_optional(repo.pool())
            .await
            .expect("manual episode mapping survives");
    assert_eq!(episode, Some(1));

    repo.pool().close().await;
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn downloads_screen_reflects_the_setting_and_persisted_tasks() {
    let (dir, db) = temp_db_path();
    let pool = connect_path(&db).await.expect("connect and migrate");
    let repo = SqliteLibraryRepository::new(pool);

    let mut settings = repo.settings_get().await.expect("settings");
    settings.optional_services.downloads_enabled = true;
    let data = serde_json::to_string(&settings).expect("serialize settings");
    sqlx::query(
        "INSERT INTO settings (id, data) VALUES (1, ?) \
         ON CONFLICT(id) DO UPDATE SET data = excluded.data",
    )
    .bind(data)
    .execute(repo.pool())
    .await
    .expect("persist download setting");

    repo.download_add(
        &DownloadItem {
            id: "d-screen".to_string(),
            torrent_hash: "h-screen".to_string(),
            media_id: Some("m-screen".to_string()),
            state: DownloadState::Ready,
            progress: 1.0,
        },
        "/downloads/show/e01.mkv",
        "magnet:?xt=urn:btih:screen",
        "/downloads/show",
    )
    .await
    .expect("download_add");

    let screen = repo.downloads().await.expect("downloads screen");
    assert!(screen.enabled, "the operator setting is the switch");
    assert_eq!(screen.tasks.len(), 1);
    assert_eq!(screen.tasks[0].id, "d-screen");
    assert_eq!(
        screen.tasks[0].title, "h-screen",
        "no media title exists yet"
    );
    assert_eq!(screen.tasks[0].state, LegacyDownloadState::Available);
    assert_eq!(screen.tasks[0].progress, 1.0);

    repo.pool().close().await;
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn acquisition_port_methods_round_trip() {
    let (dir, db) = temp_db_path();
    let pool = connect_path(&db).await.expect("connect and migrate");
    let repo = SqliteLibraryRepository::new(pool);

    let item = DownloadItem {
        id: "d1".to_string(),
        torrent_hash: "h1".to_string(),
        media_id: Some("m1".to_string()),
        state: DownloadState::Ready,
        progress: 1.0,
    };
    repo.download_add(
        &item,
        "/downloads/show/e01.mkv",
        "magnet:?xt=test",
        "/downloads/show",
    )
    .await
    .expect("download_add");

    // download_add persists the app-managed file mapping too (no production
    // INSERT into file_mappings may be omitted, or cleanup sees app_managed=false).
    let mapped: Option<String> =
        sqlx::query_scalar("SELECT torrent_hash FROM file_mappings WHERE file_path = ?")
            .bind("/downloads/show/e01.mkv")
            .fetch_optional(repo.pool())
            .await
            .expect("read mapping persisted by download_add");
    assert_eq!(mapped.as_deref(), Some("h1"));

    let status = repo
        .download_status("d1")
        .await
        .expect("download_status")
        .expect("item exists");
    assert_eq!(status.id, "d1");
    assert_eq!(status.torrent_hash, "h1");
    assert_eq!(status.media_id.as_deref(), Some("m1"));
    assert_eq!(status.state, DownloadState::Ready);
    assert_eq!(status.progress, 1.0);

    // A host-internal path is never returned by the renderer-facing status.
    let json = serde_json::to_string(&status).expect("serialize DownloadItem");
    assert!(
        !json.contains("filePath"),
        "path leaked into renderer DTO: {json}"
    );
    assert!(
        !json.contains("/downloads/"),
        "path leaked into renderer DTO: {json}"
    );

    repo.prefetch_upsert(&PrefetchEntry {
        media_id: "m2".to_string(),
        requested_for: "m1".to_string(),
        priority: 1,
        state: "pending".to_string(),
    })
    .await
    .expect("prefetch_upsert");
    repo.prefetch_upsert(&PrefetchEntry {
        media_id: "m2".to_string(),
        requested_for: "m1".to_string(),
        priority: 2,
        state: "active".to_string(),
    })
    .await
    .expect("prefetch_upsert update");
    let priority: i64 =
        sqlx::query_scalar("SELECT priority FROM prefetch_entries WHERE media_id = ?")
            .bind("m2")
            .fetch_one(repo.pool())
            .await
            .expect("read prefetch priority");
    assert_eq!(priority, 2, "upsert must update the existing row");

    sqlx::query(
        "INSERT INTO cleanup_entries (media_id, eligible_at, state, outcome) \
         VALUES ('m1', '2020-01-01T00:00:00+00:00', 'pending', NULL), \
                ('m3', '2999-01-01T00:00:00+00:00', 'pending', NULL), \
                ('m4', '2020-01-01T00:00:00+00:00', 'removed', NULL)",
    )
    .execute(repo.pool())
    .await
    .expect("seed cleanup entries");
    let eligible = repo.cleanup_eligible().await.expect("cleanup_eligible");
    assert_eq!(
        eligible.len(),
        1,
        "only past-due pending entries are eligible"
    );
    assert_eq!(eligible[0].media_id, "m1");

    // A surviving mapping keeps the torrent task (coordinated cleanup); drop
    // it first so the orphaned-task removal path can be asserted.
    sqlx::query("DELETE FROM file_mappings WHERE torrent_hash = ?")
        .bind("h1")
        .execute(repo.pool())
        .await
        .expect("drop mapping before asserting orphan task removal");
    repo.download_remove("d1").await.expect("download_remove");
    assert!(
        repo.download_status("d1")
            .await
            .expect("status after remove")
            .is_none(),
        "removed item must be gone"
    );
    let task: Option<String> = sqlx::query_scalar("SELECT hash FROM torrent_tasks WHERE hash = ?")
        .bind("h1")
        .fetch_optional(repo.pool())
        .await
        .expect("read torrent task");
    assert!(task.is_none(), "orphaned torrent task must be removed");

    repo.pool().close().await;
    std::fs::remove_dir_all(&dir).ok();
}
