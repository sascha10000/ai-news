use serde::Deserialize;
use sqlx::SqlitePool;

#[derive(sqlx::FromRow, Clone)]
pub struct Feed {
    pub id: i64,
    pub name: String,
    pub url: String,
    pub active: bool,
    pub fetch_interval_minutes: i32,
    pub last_fetched_at: Option<String>,
    pub created_at: String,
}

#[derive(Deserialize)]
pub struct CreateFeed {
    pub name: String,
    pub url: String,
}

impl Feed {
    pub async fn all(pool: &SqlitePool) -> Result<Vec<Feed>, sqlx::Error> {
        sqlx::query_as::<_, Feed>("SELECT * FROM feeds ORDER BY created_at DESC")
            .fetch_all(pool)
            .await
    }

    pub async fn active(pool: &SqlitePool) -> Result<Vec<Feed>, sqlx::Error> {
        sqlx::query_as::<_, Feed>("SELECT * FROM feeds WHERE active = 1 ORDER BY name")
            .fetch_all(pool)
            .await
    }

    pub async fn by_id(pool: &SqlitePool, id: i64) -> Result<Option<Feed>, sqlx::Error> {
        sqlx::query_as::<_, Feed>("SELECT * FROM feeds WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    pub async fn create(pool: &SqlitePool, name: &str, url: &str) -> Result<i64, sqlx::Error> {
        let result = sqlx::query("INSERT INTO feeds (name, url) VALUES (?, ?)")
            .bind(name)
            .bind(url)
            .execute(pool)
            .await?;
        Ok(result.last_insert_rowid())
    }

    pub async fn delete(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM feeds WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn update_last_fetched(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE feeds SET last_fetched_at = datetime('now') WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}
