use sqlx::SqlitePool;

#[derive(sqlx::FromRow, Clone)]
pub struct SourceArticle {
    pub id: i64,
    pub feed_id: i64,
    pub guid: Option<String>,
    pub title: String,
    pub url: String,
    pub author: Option<String>,
    pub content: String,
    pub summary: Option<String>,
    pub published_at: Option<String>,
    pub fetched_at: String,
}

impl SourceArticle {
    pub async fn insert(
        pool: &SqlitePool,
        feed_id: i64,
        guid: Option<&str>,
        title: &str,
        url: &str,
        author: Option<&str>,
        content: &str,
        summary: Option<&str>,
        published_at: Option<&str>,
    ) -> Result<Option<i64>, sqlx::Error> {
        let result = sqlx::query(
            "INSERT OR IGNORE INTO source_articles (feed_id, guid, title, url, author, content, summary, published_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(feed_id)
        .bind(guid)
        .bind(title)
        .bind(url)
        .bind(author)
        .bind(content)
        .bind(summary)
        .bind(published_at)
        .execute(pool)
        .await?;

        if result.rows_affected() > 0 {
            Ok(Some(result.last_insert_rowid()))
        } else {
            Ok(None)
        }
    }

    pub async fn recent_uncited(
        pool: &SqlitePool,
        fetched_within_hours: i64,
        max_published_age_days: i64,
        list_id: Option<i64>,
    ) -> Result<Vec<SourceArticle>, sqlx::Error> {
        let base = "SELECT sa.* FROM source_articles sa
             WHERE sa.fetched_at > datetime('now', ? || ' hours')
             AND sa.published_at IS NOT NULL
             AND datetime(sa.published_at) IS NOT NULL
             AND datetime(sa.published_at) > datetime('now', ? || ' days')";

        match list_id {
            Some(lid) => {
                sqlx::query_as::<_, SourceArticle>(&format!(
                    "{base}
                     AND sa.feed_id IN (SELECT feed_id FROM feed_lists WHERE list_id = ?)
                     AND sa.id NOT IN (
                         SELECT sc.source_article_id FROM sentence_citations sc
                         JOIN generated_sentences gs ON gs.id = sc.sentence_id
                         JOIN generated_articles ga ON ga.id = gs.generated_article_id
                         WHERE ga.list_id = ?
                     )
                     ORDER BY sa.published_at DESC"
                ))
                .bind(-fetched_within_hours)
                .bind(-max_published_age_days)
                .bind(lid)
                .bind(lid)
                .fetch_all(pool)
                .await
            }
            None => {
                sqlx::query_as::<_, SourceArticle>(&format!(
                    "{base}
                     AND sa.feed_id IN (SELECT id FROM feeds WHERE user_id IS NULL)
                     AND sa.id NOT IN (
                         SELECT sc.source_article_id FROM sentence_citations sc
                         JOIN generated_sentences gs ON gs.id = sc.sentence_id
                         JOIN generated_articles ga ON ga.id = gs.generated_article_id
                         WHERE ga.list_id IS NULL AND ga.user_id IS NULL
                     )
                     ORDER BY sa.published_at DESC"
                ))
                .bind(-fetched_within_hours)
                .bind(-max_published_age_days)
                .fetch_all(pool)
                .await
            }
        }
    }

    pub async fn recent_uncited_for_user(
        pool: &SqlitePool,
        fetched_within_hours: i64,
        max_published_age_days: i64,
        user_id: i64,
    ) -> Result<Vec<SourceArticle>, sqlx::Error> {
        sqlx::query_as::<_, SourceArticle>(
            "SELECT sa.* FROM source_articles sa
             WHERE sa.fetched_at > datetime('now', ? || ' hours')
             AND sa.published_at IS NOT NULL
             AND datetime(sa.published_at) IS NOT NULL
             AND datetime(sa.published_at) > datetime('now', ? || ' days')
             AND sa.feed_id IN (SELECT id FROM feeds WHERE user_id = ?)
             AND sa.id NOT IN (
                 SELECT sc.source_article_id FROM sentence_citations sc
                 JOIN generated_sentences gs ON gs.id = sc.sentence_id
                 JOIN generated_articles ga ON ga.id = gs.generated_article_id
                 WHERE ga.user_id = ?
             )
             ORDER BY sa.published_at DESC",
        )
        .bind(-fetched_within_hours)
        .bind(-max_published_age_days)
        .bind(user_id)
        .bind(user_id)
        .fetch_all(pool)
        .await
    }

    pub async fn by_ids(pool: &SqlitePool, ids: &[i64]) -> Result<Vec<SourceArticle>, sqlx::Error> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let placeholders: String = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!("SELECT * FROM source_articles WHERE id IN ({placeholders})");
        let mut q = sqlx::query_as::<_, SourceArticle>(&query);
        for id in ids {
            q = q.bind(id);
        }
        q.fetch_all(pool).await
    }

    pub async fn count_for_feed(pool: &SqlitePool, feed_id: i64) -> Result<i64, sqlx::Error> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM source_articles WHERE feed_id = ?")
            .bind(feed_id)
            .fetch_one(pool)
            .await?;
        Ok(row.0)
    }
}
