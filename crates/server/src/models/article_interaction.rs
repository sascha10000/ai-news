use sqlx::SqlitePool;

use super::generated_article::GeneratedArticle;

pub struct ArticleInteraction;

impl ArticleInteraction {
    pub async fn like(pool: &SqlitePool, user_id: i64, article_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT OR IGNORE INTO article_likes (user_id, article_id) VALUES (?, ?)",
        )
        .bind(user_id)
        .bind(article_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn unlike(pool: &SqlitePool, user_id: i64, article_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM article_likes WHERE user_id = ? AND article_id = ?")
            .bind(user_id)
            .bind(article_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn is_liked(
        pool: &SqlitePool,
        user_id: i64,
        article_id: i64,
    ) -> Result<bool, sqlx::Error> {
        let row: Option<(i64,)> = sqlx::query_as(
            "SELECT 1 FROM article_likes WHERE user_id = ? AND article_id = ?",
        )
        .bind(user_id)
        .bind(article_id)
        .fetch_optional(pool)
        .await?;
        Ok(row.is_some())
    }

    pub async fn like_count(pool: &SqlitePool, article_id: i64) -> Result<i64, sqlx::Error> {
        let row: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM article_likes WHERE article_id = ?")
                .bind(article_id)
                .fetch_one(pool)
                .await?;
        Ok(row.0)
    }

    pub async fn mark_read_later(
        pool: &SqlitePool,
        user_id: i64,
        article_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT OR IGNORE INTO article_read_later (user_id, article_id) VALUES (?, ?)",
        )
        .bind(user_id)
        .bind(article_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn unmark_read_later(
        pool: &SqlitePool,
        user_id: i64,
        article_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM article_read_later WHERE user_id = ? AND article_id = ?")
            .bind(user_id)
            .bind(article_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn is_read_later(
        pool: &SqlitePool,
        user_id: i64,
        article_id: i64,
    ) -> Result<bool, sqlx::Error> {
        let row: Option<(i64,)> = sqlx::query_as(
            "SELECT 1 FROM article_read_later WHERE user_id = ? AND article_id = ?",
        )
        .bind(user_id)
        .bind(article_id)
        .fetch_optional(pool)
        .await?;
        Ok(row.is_some())
    }

    pub async fn liked_for_user(
        pool: &SqlitePool,
        user_id: i64,
    ) -> Result<Vec<GeneratedArticle>, sqlx::Error> {
        sqlx::query_as::<_, GeneratedArticle>(
            "SELECT ga.* FROM generated_articles ga \
             INNER JOIN article_likes al ON al.article_id = ga.id \
             WHERE al.user_id = ? \
             ORDER BY al.created_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
    }

    pub async fn read_later_for_user(
        pool: &SqlitePool,
        user_id: i64,
    ) -> Result<Vec<GeneratedArticle>, sqlx::Error> {
        sqlx::query_as::<_, GeneratedArticle>(
            "SELECT ga.* FROM generated_articles ga \
             INNER JOIN article_read_later arl ON arl.article_id = ga.id \
             WHERE arl.user_id = ? \
             ORDER BY arl.created_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
    }
}
