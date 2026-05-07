use rand::Rng;
use sqlx::SqlitePool;

pub struct Session;

#[derive(Clone, Copy, Debug)]
pub enum Identity {
    Admin,
    User(i64),
}

impl Session {
    pub async fn create(pool: &SqlitePool, identity: Identity) -> Result<String, sqlx::Error> {
        let token: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(64)
            .map(char::from)
            .collect();

        let user_id: Option<i64> = match identity {
            Identity::Admin => None,
            Identity::User(id) => Some(id),
        };

        sqlx::query(
            "INSERT INTO sessions (token, user_id, expires_at) VALUES (?, ?, datetime('now', '+24 hours'))",
        )
        .bind(&token)
        .bind(user_id)
        .execute(pool)
        .await?;

        Ok(token)
    }

    pub async fn validate(
        pool: &SqlitePool,
        token: &str,
    ) -> Result<Option<Identity>, sqlx::Error> {
        let row: Option<(Option<i64>,)> = sqlx::query_as(
            "SELECT user_id FROM sessions WHERE token = ? AND expires_at > datetime('now')",
        )
        .bind(token)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|(user_id,)| match user_id {
            Some(id) => Identity::User(id),
            None => Identity::Admin,
        }))
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
