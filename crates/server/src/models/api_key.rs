use rand::RngCore;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

/// Number of characters of the plaintext key kept for display ("ainews_ab12…").
const PREFIX_LEN: usize = 12;

#[derive(sqlx::FromRow, Clone, Debug)]
pub struct ApiKey {
    pub id: i64,
    pub user_id: i64,
    pub key_prefix: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

/// Unsalted sha256, unlike `user::hash_password`: API keys are 128-bit random
/// values, so rainbow tables are useless and the deterministic hash lets us
/// look a key up with `WHERE key_hash = ?`.
pub fn hash_key(key: &str) -> String {
    hex::encode(Sha256::digest(key.as_bytes()))
}

impl ApiKey {
    /// Create (or replace) the user's API key and return the plaintext.
    /// The plaintext is never stored; callers must show it to the user now.
    pub async fn generate(pool: &SqlitePool, user_id: i64) -> Result<String, sqlx::Error> {
        let mut bytes = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut bytes);
        let key = format!("ainews_{}", hex::encode(bytes));
        let prefix: String = key.chars().take(PREFIX_LEN).collect();

        let mut tx = pool.begin().await?;
        sqlx::query("DELETE FROM api_keys WHERE user_id = ?")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO api_keys (user_id, key_hash, key_prefix) VALUES (?, ?, ?)")
            .bind(user_id)
            .bind(hash_key(&key))
            .bind(&prefix)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;

        Ok(key)
    }

    pub async fn find_for_user(
        pool: &SqlitePool,
        user_id: i64,
    ) -> Result<Option<ApiKey>, sqlx::Error> {
        sqlx::query_as::<_, ApiKey>(
            "SELECT id, user_id, key_prefix, created_at, last_used_at FROM api_keys WHERE user_id = ?",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await
    }

    pub async fn delete_for_user(pool: &SqlitePool, user_id: i64) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM api_keys WHERE user_id = ?")
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Resolve a plaintext key to its owner, bumping last_used_at on a hit.
    pub async fn user_id_for_key(
        pool: &SqlitePool,
        key: &str,
    ) -> Result<Option<i64>, sqlx::Error> {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT user_id FROM api_keys WHERE key_hash = ?")
                .bind(hash_key(key))
                .fetch_optional(pool)
                .await?;
        if let Some((user_id,)) = row {
            sqlx::query("UPDATE api_keys SET last_used_at = datetime('now') WHERE user_id = ?")
                .bind(user_id)
                .execute(pool)
                .await?;
            return Ok(Some(user_id));
        }
        Ok(None)
    }
}
