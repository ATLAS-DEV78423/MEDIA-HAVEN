//! R-01 per-item sources (ADR-0021) and the fail-closed playability gate.

use app_core::application::LibraryService;
use app_core::domain::*;
use app_core::ports::LibraryRepository;
use media_persistence::sqlite::{connect_path, SqliteLibraryRepository};

const MAGNET: &str = "magnet:?xt=urn:btih:ABC123&dn=episode";

fn temp_db() -> (std::path::PathBuf, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("mh-r01-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    (dir.clone(), dir.join("library.db"))
}

async fn seed_torrent(pool: &sqlx::SqlitePool, hash: &str, source: &str) {
    sqlx::query(
        "INSERT INTO torrent_tasks \
         (hash, magnet_or_source, state, progress, save_path, created_at, updated_at) \
         VALUES (?, ?, 'ready', 1.0, '/downloads', '', '')",
    )
    .bind(hash)
    .bind(source)
    .execute(pool)
    .await
    .expect("insert torrent task");
}

async fn seed_download(pool: &sqlx::SqlitePool, id: &str, hash: &str) {
    sqlx::query(
        "INSERT INTO download_items (id, torrent_hash, media_id, file_path, state, created_at) \
         VALUES (?, ?, NULL, ?, 'ready', '')",
    )
    .bind(id)
    .bind(hash)
    .bind(format!("/downloads/{id}.mkv"))
    .execute(pool)
    .await
    .expect("insert download item");
}

async fn seed_media(pool: &sqlx::SqlitePool, id: &str) {
    sqlx::query(
        "INSERT INTO media_items (id, kind, title, created_at, updated_at) \
         VALUES (?, 'movie', ?, '', '')",
    )
    .bind(id)
    .bind(format!("title-{id}"))
    .execute(pool)
    .await
    .expect("insert media");
}

#[tokio::test]
async fn source_round_trips_and_invalid_sources_are_rejected() {
    let (dir, db) = temp_db();
    let pool = connect_path(&db).await.expect("connect and migrate");
    let repo = SqliteLibraryRepository::new(pool.clone());

    assert_eq!(
        repo.source_for_media("m-unknown").await.expect("read"),
        None,
        "an item with no source reads back as None"
    );

    repo.set_item_source("m1", MAGNET)
        .await
        .expect("set magnet");
    assert_eq!(
        repo.source_for_media("m1").await.expect("read").as_deref(),
        Some(MAGNET)
    );
    repo.set_item_source("m1", "episode.torrent")
        .await
        .expect("replace source");
    assert_eq!(
        repo.source_for_media("m1").await.expect("read").as_deref(),
        Some("episode.torrent"),
        "setting again replaces the source"
    );

    for bad in [
        "",
        "   ",
        &"m".repeat(MAX_ITEM_SOURCE_LEN + 1),
        "not a magnet",
        "C:\\media\\ep.torrent",
        "/downloads/ep.torrent",
        "..\\ep.torrent",
        ".torrent",
        "magnet:?xt=urn:btih:ABC\n&dn=ep",
    ] {
        let err = repo
            .set_item_source("m1", bad)
            .await
            .expect_err("invalid source must be rejected");
        assert_eq!(err.code, AppErrorCode::InvalidRequest);
        assert!(!err.message.contains("/downloads"), "no path in the reason");
    }
    assert_eq!(
        repo.source_for_media("m1").await.expect("read").as_deref(),
        Some("episode.torrent"),
        "a rejected write must not change the stored source"
    );
    assert!(
        repo.set_item_source("", MAGNET)
            .await
            .expect_err("empty media id")
            .code
            == AppErrorCode::InvalidRequest
    );

    pool.close().await;
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn setting_a_source_ties_matching_download_rows_to_the_media_id() {
    let (dir, db) = temp_db();
    let pool = connect_path(&db).await.expect("connect and migrate");
    let repo = SqliteLibraryRepository::new(pool.clone());

    // A download added through this app stores the operator's source verbatim.
    seed_torrent(&pool, "h-exact", MAGNET).await;
    seed_download(&pool, "d-exact", "h-exact").await;
    // qB may report a hash that only appears in the magnet's info hash.
    seed_torrent(&pool, "abc123", "some-other-source").await;
    seed_download(&pool, "d-by-hash", "abc123").await;

    repo.set_item_source("m-tied", MAGNET)
        .await
        .expect("set source");

    for id in ["d-exact", "d-by-hash"] {
        let tied: Option<String> =
            sqlx::query_scalar("SELECT media_id FROM download_items WHERE id = ?")
                .bind(id)
                .fetch_one(&pool)
                .await
                .expect("read download media id");
        assert_eq!(
            tied.as_deref(),
            Some("m-tied"),
            "{id} must inherit the media id from its source"
        );
    }
    let name: Option<String> =
        sqlx::query_scalar("SELECT source_name FROM item_sources WHERE media_id = ?")
            .bind("m-tied")
            .fetch_one(&pool)
            .await
            .expect("read source name");
    assert_eq!(
        name.as_deref(),
        Some("Magnet link"),
        "no tracker URL is surfaced"
    );

    pool.close().await;
    std::fs::remove_dir_all(&dir).ok();
}

/// The shipped schema makes the gate columns NOT NULL. Rebuild the table in
/// this throwaway database to simulate a legacy/corrupt row with NULL states
/// and prove the *code* guard, not just the DB constraint.
async fn allow_null_gate_states(pool: &sqlx::SqlitePool) {
    sqlx::query("CREATE TABLE download_items_backup AS SELECT * FROM download_items")
        .execute(pool)
        .await
        .expect("back up rows");
    sqlx::query("DROP TABLE download_items")
        .execute(pool)
        .await
        .expect("drop current table");
    sqlx::query(
        "CREATE TABLE download_items (
           id TEXT PRIMARY KEY,
           torrent_hash TEXT NOT NULL REFERENCES torrent_tasks(hash),
           media_id TEXT,
           file_path TEXT NOT NULL,
           state TEXT NOT NULL,
           created_at TEXT NOT NULL,
           verification_state TEXT,
           verification_detail TEXT,
           defender_state TEXT,
           defender_detail TEXT
         )",
    )
    .execute(pool)
    .await
    .expect("create nullable-gate table");
    sqlx::query(
        "INSERT INTO download_items
         (id, torrent_hash, media_id, file_path, state, created_at,
          verification_state, verification_detail, defender_state, defender_detail)
         SELECT id, torrent_hash, media_id, file_path, state, created_at,
                verification_state, verification_detail, defender_state, defender_detail
         FROM download_items_backup",
    )
    .execute(pool)
    .await
    .expect("restore rows");
    sqlx::query("DROP TABLE download_items_backup")
        .execute(pool)
        .await
        .expect("drop backup");
}

#[tokio::test]
async fn null_verification_on_a_download_refuses_play_but_a_library_import_is_allowed() {
    let (dir, db) = temp_db();
    let pool = connect_path(&db).await.expect("connect and migrate");
    allow_null_gate_states(&pool).await;

    seed_media(&pool, "m-download").await;
    seed_media(&pool, "m-library").await;
    seed_torrent(&pool, "h-null", MAGNET).await;
    sqlx::query(
        "INSERT INTO download_items \
         (id, torrent_hash, media_id, file_path, state, created_at, verification_state, defender_state) \
         VALUES ('d-null', 'h-null', 'm-download', '/downloads/d-null.mkv', 'ready', '', NULL, NULL)",
    )
    .execute(&pool)
    .await
    .expect("insert download with NULL gate states");

    let repo = SqliteLibraryRepository::new(pool.clone());
    assert!(repo.has_download_row("m-download").await.expect("has row"));
    assert!(!repo.has_download_row("m-library").await.expect("has row"));

    let service = LibraryService::new(SqliteLibraryRepository::new(pool.clone()));
    let detail_error = service
        .detail("m-download")
        .await
        .expect_err("NULL verification on a download must not show a detail view");
    assert_eq!(detail_error.code, AppErrorCode::MediaUnsupported);
    assert!(
        !detail_error.message.contains("/downloads"),
        "no path in reason"
    );

    let play_error = service
        .playback_start(&PlaybackStartRequest {
            media_id: "m-download".to_string(),
            start_position_seconds: None,
        })
        .await
        .expect_err("NULL verification on a download must not play");
    assert_eq!(play_error.code, AppErrorCode::MediaUnsupported);

    assert!(
        service.detail("m-library").await.is_ok(),
        "a library import has no download row and stays allowed"
    );

    pool.close().await;
    std::fs::remove_dir_all(&dir).ok();
}
