use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{ConnectOptions, SqlitePool};
use std::str::FromStr;

pub async fn init_pool(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    // Run migrations on a dedicated connection with FKs disabled.
    // Some migrations rebuild parent tables; SQLite blocks DROP of a
    // referenced table when foreign_keys is on.
    let migration_options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .foreign_keys(false);

    let mut migration_conn = migration_options.connect().await?;
    sqlx::migrate!("../../migrations").run(&mut migration_conn).await?;

    let violations: Vec<(i64, i64, String, i64)> =
        sqlx::query_as("PRAGMA foreign_key_check")
            .fetch_all(&mut migration_conn)
            .await
            .unwrap_or_default();
    if !violations.is_empty() {
        return Err(sqlx::Error::Protocol(format!(
            "foreign_key_check found {} violation(s) after migration",
            violations.len()
        )));
    }
    drop(migration_conn);

    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    Ok(pool)
}
