use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::Path;
use std::str::FromStr;

/// Open (creating if needed) a SQLite database from a connection URL and apply
/// embedded migrations.
pub async fn connect(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    let opts = SqliteConnectOptions::from_str(database_url)?;
    open(opts).await
}

/// Open (creating if needed) a SQLite database from a filesystem path.
///
/// Preferred over URL parsing on Windows, where drive letters and backslashes
/// make hand-built `sqlite://` URLs error prone.
pub async fn connect_path(path: &Path) -> Result<SqlitePool, sqlx::Error> {
    let opts = SqliteConnectOptions::new().filename(path);
    open(opts).await
}

async fn open(opts: SqliteConnectOptions) -> Result<SqlitePool, sqlx::Error> {
    let opts = opts
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await?;
    run_migrations(&pool).await?;
    Ok(pool)
}

/// Apply the embedded migrations. Idempotent: already-applied versions are
/// skipped, so this is safe to call on every start.
pub async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}
