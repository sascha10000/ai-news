use sqlx::SqlitePool;

pub struct AppSettings;

impl AppSettings {
    pub async fn auto_publish(pool: &SqlitePool) -> Result<bool, sqlx::Error> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value FROM app_settings WHERE key = 'auto_publish'")
                .fetch_optional(pool)
                .await?;
        Ok(row.map(|r| r.0 == "true").unwrap_or(false))
    }

    pub async fn set_auto_publish(pool: &SqlitePool, enabled: bool) -> Result<(), sqlx::Error> {
        let value = if enabled { "true" } else { "false" };
        sqlx::query(
            "INSERT INTO app_settings (key, value) VALUES ('auto_publish', ?) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(value)
        .execute(pool)
        .await?;
        Ok(())
    }
}
