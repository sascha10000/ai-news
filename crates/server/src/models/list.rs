use serde::Deserialize;
use sqlx::SqlitePool;

#[derive(sqlx::FromRow, Clone)]
pub struct List {
    pub id: i64,
    pub user_id: Option<i64>,
    pub name: String,
    pub slug: String,
    pub created_at: String,
}

#[derive(sqlx::FromRow, Clone)]
pub struct ListWithOwner {
    #[sqlx(flatten)]
    pub list: List,
    pub owner_username: Option<String>,
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

    pub async fn all_global(pool: &SqlitePool) -> Result<Vec<List>, sqlx::Error> {
        sqlx::query_as::<_, List>("SELECT * FROM lists WHERE user_id IS NULL ORDER BY name")
            .fetch_all(pool)
            .await
    }

    pub async fn all_for_user(pool: &SqlitePool, user_id: i64) -> Result<Vec<List>, sqlx::Error> {
        sqlx::query_as::<_, List>("SELECT * FROM lists WHERE user_id = ? ORDER BY name")
            .bind(user_id)
            .fetch_all(pool)
            .await
    }

    pub async fn all_with_owner(pool: &SqlitePool) -> Result<Vec<ListWithOwner>, sqlx::Error> {
        sqlx::query_as::<_, ListWithOwner>(
            "SELECT l.*, u.username AS owner_username \
             FROM lists l LEFT JOIN users u ON u.id = l.user_id \
             ORDER BY u.username NULLS FIRST, l.name",
        )
        .fetch_all(pool)
        .await
    }

    pub async fn by_id(pool: &SqlitePool, id: i64) -> Result<Option<List>, sqlx::Error> {
        sqlx::query_as::<_, List>("SELECT * FROM lists WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    pub async fn owner_of(pool: &SqlitePool, id: i64) -> Result<Option<i64>, sqlx::Error> {
        let row: Option<(Option<i64>,)> = sqlx::query_as("SELECT user_id FROM lists WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
        Ok(row.and_then(|r| r.0))
    }

    pub async fn create(
        pool: &SqlitePool,
        name: &str,
        slug: &str,
        user_id: Option<i64>,
    ) -> Result<i64, sqlx::Error> {
        let result = sqlx::query("INSERT INTO lists (name, slug, user_id) VALUES (?, ?, ?)")
            .bind(name)
            .bind(slug)
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(result.last_insert_rowid())
    }

    pub async fn delete(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM lists WHERE id = ? AND user_id IS NULL")
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
        let result = sqlx::query("DELETE FROM lists WHERE id = ? AND user_id = ?")
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
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
        sqlx::query(
            "INSERT OR IGNORE INTO feed_lists (feed_id, list_id) \
             SELECT ?, ? WHERE \
             EXISTS (SELECT 1 FROM feeds WHERE id = ? AND user_id IS NULL) AND \
             EXISTS (SELECT 1 FROM lists WHERE id = ? AND user_id IS NULL)",
        )
        .bind(feed_id)
        .bind(list_id)
        .bind(feed_id)
        .bind(list_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Insert (feed, list) into feed_lists only if both belong to the given user.
    /// Returns true on success, false if either is missing or owned by someone else.
    pub async fn add_feed_for_user(
        pool: &SqlitePool,
        list_id: i64,
        feed_id: i64,
        user_id: i64,
    ) -> Result<bool, sqlx::Error> {
        let list_owner: Option<(Option<i64>,)> =
            sqlx::query_as("SELECT user_id FROM lists WHERE id = ?")
                .bind(list_id)
                .fetch_optional(pool)
                .await?;
        let feed_owner: Option<(Option<i64>,)> =
            sqlx::query_as("SELECT user_id FROM feeds WHERE id = ?")
                .bind(feed_id)
                .fetch_optional(pool)
                .await?;
        match (list_owner, feed_owner) {
            (Some((Some(lo),)), Some((Some(fo),))) if lo == user_id && fo == user_id => {
                sqlx::query("INSERT OR IGNORE INTO feed_lists (feed_id, list_id) VALUES (?, ?)")
                    .bind(feed_id)
                    .bind(list_id)
                    .execute(pool)
                    .await?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    pub async fn remove_feed_for_user(
        pool: &SqlitePool,
        list_id: i64,
        feed_id: i64,
        user_id: i64,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM feed_lists \
             WHERE feed_id = ? AND list_id = ? \
             AND EXISTS (SELECT 1 FROM lists WHERE id = ? AND user_id = ?) \
             AND EXISTS (SELECT 1 FROM feeds WHERE id = ? AND user_id = ?)",
        )
        .bind(feed_id)
        .bind(list_id)
        .bind(list_id)
        .bind(user_id)
        .bind(feed_id)
        .bind(user_id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn remove_feed(
        pool: &SqlitePool,
        list_id: i64,
        feed_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "DELETE FROM feed_lists \
             WHERE list_id = ? AND feed_id = ? \
             AND EXISTS (SELECT 1 FROM lists WHERE id = ? AND user_id IS NULL)",
        )
        .bind(list_id)
        .bind(feed_id)
        .bind(list_id)
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
