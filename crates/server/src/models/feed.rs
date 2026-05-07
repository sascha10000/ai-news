use serde::Deserialize;
use sqlx::SqlitePool;

#[derive(sqlx::FromRow, Clone)]
pub struct Feed {
    pub id: i64,
    pub user_id: Option<i64>,
    pub name: String,
    pub url: String,
    pub active: bool,
    pub fetch_interval_minutes: i32,
    pub last_fetched_at: Option<String>,
    pub created_at: String,
}

#[derive(sqlx::FromRow, Clone)]
pub struct FeedWithOwner {
    #[sqlx(flatten)]
    pub feed: Feed,
    pub owner_username: Option<String>,
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

    pub async fn all_global(pool: &SqlitePool) -> Result<Vec<Feed>, sqlx::Error> {
        sqlx::query_as::<_, Feed>(
            "SELECT * FROM feeds WHERE user_id IS NULL ORDER BY created_at DESC",
        )
        .fetch_all(pool)
        .await
    }

    pub async fn all_for_user(pool: &SqlitePool, user_id: i64) -> Result<Vec<Feed>, sqlx::Error> {
        sqlx::query_as::<_, Feed>(
            "SELECT * FROM feeds WHERE user_id = ? ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
    }

    pub async fn all_with_owner(pool: &SqlitePool) -> Result<Vec<FeedWithOwner>, sqlx::Error> {
        sqlx::query_as::<_, FeedWithOwner>(
            "SELECT f.*, u.username AS owner_username \
             FROM feeds f LEFT JOIN users u ON u.id = f.user_id \
             ORDER BY f.created_at DESC",
        )
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

    pub async fn create(
        pool: &SqlitePool,
        name: &str,
        url: &str,
        user_id: Option<i64>,
    ) -> Result<i64, sqlx::Error> {
        let result = sqlx::query("INSERT INTO feeds (name, url, user_id) VALUES (?, ?, ?)")
            .bind(name)
            .bind(url)
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(result.last_insert_rowid())
    }

    pub async fn delete(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM feeds WHERE id = ? AND user_id IS NULL")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn delete_for_user(
        pool: &SqlitePool,
        id: i64,
        user_id: i64,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM feeds WHERE id = ? AND user_id = ?")
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn owner_of(pool: &SqlitePool, id: i64) -> Result<Option<Option<i64>>, sqlx::Error> {
        let row: Option<(Option<i64>,)> = sqlx::query_as("SELECT user_id FROM feeds WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
        Ok(row.map(|r| r.0))
    }

    pub async fn update_last_fetched(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE feeds SET last_fetched_at = datetime('now') WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}
