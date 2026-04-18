use rand::Rng;
use sqlx::SqlitePool;

pub struct Session;

impl Session {
    pub async fn create(pool: &SqlitePool) -> Result<String, sqlx::Error> {
        let token: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(64)
            .map(char::from)
            .collect();

        sqlx::query(
            "INSERT INTO sessions (token, expires_at) VALUES (?, datetime('now', '+24 hours'))",
        )
        .bind(&token)
        .execute(pool)
        .await?;

        Ok(token)
    }

    pub async fn validate(pool: &SqlitePool, token: &str) -> Result<Option<bool>, sqlx::Error> {
        let row: Option<(i64,)> = sqlx::query_as(
            "SELECT 1 FROM sessions WHERE token = ? AND expires_at > datetime('now')",
        )
        .bind(token)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|_| true))
    }

    pub async fn delete(pool: &SqlitePool, token: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM sessions WHERE token = ?")
            .bind(token)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn cleanup_expired(pool: &SqlitePool) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM sessions WHERE expires_at <= datetime('now')")
            .execute(pool)
            .await?;
        Ok(())
    }
}
