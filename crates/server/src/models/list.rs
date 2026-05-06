use serde::Deserialize;
use sqlx::SqlitePool;

#[derive(sqlx::FromRow, Clone)]
pub struct List {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub created_at: String,
}

#[derive(Deserialize)]
pub struct CreateList {
    pub name: String,
}

impl List {
    pub async fn all(pool: &SqlitePool) -> Result<Vec<List>, sqlx::Error> {
        sqlx::query_as::<_, List>("SELECT * FROM lists ORDER BY name")
            .fetch_all(pool)
            .await
    }

    pub async fn by_id(pool: &SqlitePool, id: i64) -> Result<Option<List>, sqlx::Error> {
        sqlx::query_as::<_, List>("SELECT * FROM lists WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    pub async fn create(pool: &SqlitePool, name: &str, slug: &str) -> Result<i64, sqlx::Error> {
        let result = sqlx::query("INSERT INTO lists (name, slug) VALUES (?, ?)")
            .bind(name)
            .bind(slug)
            .execute(pool)
            .await?;
        Ok(result.last_insert_rowid())
    }

    pub async fn delete(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM lists WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn feeds(pool: &SqlitePool, list_id: i64) -> Result<Vec<i64>, sqlx::Error> {
        let rows: Vec<(i64,)> =
            sqlx::query_as("SELECT feed_id FROM feed_lists WHERE list_id = ? ORDER BY feed_id")
                .bind(list_id)
                .fetch_all(pool)
                .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    pub async fn lists_for_feed(
        pool: &SqlitePool,
        feed_id: i64,
    ) -> Result<Vec<List>, sqlx::Error> {
        sqlx::query_as::<_, List>(
            "SELECT l.* FROM lists l
             JOIN feed_lists fl ON fl.list_id = l.id
             WHERE fl.feed_id = ?
             ORDER BY l.name",
        )
        .bind(feed_id)
        .fetch_all(pool)
        .await
    }

    pub async fn add_feed(
        pool: &SqlitePool,
        list_id: i64,
        feed_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT OR IGNORE INTO feed_lists (feed_id, list_id) VALUES (?, ?)")
            .bind(feed_id)
            .bind(list_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn remove_feed(
        pool: &SqlitePool,
        list_id: i64,
        feed_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM feed_lists WHERE list_id = ? AND feed_id = ?")
            .bind(list_id)
            .bind(feed_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn set_feeds(
        pool: &SqlitePool,
        list_id: i64,
        feed_ids: &[i64],
    ) -> Result<(), sqlx::Error> {
        let mut tx = pool.begin().await?;
        sqlx::query("DELETE FROM feed_lists WHERE list_id = ?")
            .bind(list_id)
            .execute(&mut *tx)
            .await?;
        for feed_id in feed_ids {
            sqlx::query("INSERT OR IGNORE INTO feed_lists (feed_id, list_id) VALUES (?, ?)")
                .bind(feed_id)
                .bind(list_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await
    }
}
