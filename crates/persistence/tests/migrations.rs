//! Migration smoke tests: an empty directory must produce a usable database.

use media_persistence::sqlite::pool;
use sqlx::Row;

fn temp_db_path() -> (std::path::PathBuf, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("mediahaven-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let db = dir.join("library.db");
    (dir, db)
}

#[tokio::test]
async fn migrations_create_schema_and_enable_foreign_keys() {
    let (dir, db) = temp_db_path();

    let pool = pool::connect_path(&db).await.expect("connect and migrate");

    let media_items =
        sqlx::query("SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'media_items'")
            .fetch_optional(&pool)
            .await
            .expect("query sqlite_master");
    assert!(media_items.is_some(), "media_items table was not created");

    for table in [
        "media_files",
        "library_roots",
        "watchlist_items",
        "queue_items",
        "settings",
    ] {
        let found = sqlx::query("SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?")
            .bind(table)
            .fetch_optional(&pool)
            .await
            .expect("query sqlite_master");
        assert!(found.is_some(), "{table} table was not created");
    }

    let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
        .fetch_one(&pool)
        .await
        .expect("read pragma");
    assert_eq!(foreign_keys, 1, "foreign keys must be enabled");

    // The enabled pragma must actually be enforced, not just reported.
    let orphan = sqlx::query(
        "INSERT INTO media_files (id, media_id, created_at) VALUES ('f1', 'nope', 'now')",
    )
    .execute(&pool)
    .await;
    assert!(orphan.is_err(), "foreign key violation was not rejected");

    pool.close().await;
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn migrations_are_idempotent() {
    let (dir, db) = temp_db_path();

    let first = pool::connect_path(&db).await.expect("first migrate");
    let expected: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(&first)
        .await
        .expect("read first migration bookkeeping");
    first.close().await;

    let second = pool::connect_path(&db).await.expect("second migrate");
    let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(&second)
        .await
        .expect("read migration bookkeeping");
    assert_eq!(
        applied, expected,
        "migrations were re-applied instead of skipped"
    );

    let kind_column: String =
        sqlx::query("SELECT type FROM pragma_table_info('media_items') WHERE name = 'kind'")
            .fetch_one(&second)
            .await
            .map(|row| row.get("type"))
            .expect("read media_items.kind");
    assert_eq!(kind_column, "TEXT");

    second.close().await;
    std::fs::remove_dir_all(&dir).ok();
}
