use crate::filters;
use sqlx::SqlitePool;

#[derive(sqlx::FromRow, Clone)]
pub struct GeneratedArticle {
    pub id: i64,
    pub title: String,
    pub slug: String,
    pub summary: Option<String>,
    pub category: Option<String>,
    pub status: String,
    pub generated_at: String,
    pub published_at: Option<String>,
    pub list_id: Option<i64>,
    pub user_id: Option<i64>,
}

#[derive(sqlx::FromRow, Clone)]
pub struct ArticleWithOwner {
    #[sqlx(flatten)]
    pub article: GeneratedArticle,
    pub owner_username: Option<String>,
}

impl GeneratedArticle {
    pub fn formatted_published_at(&self) -> String {
        match &self.published_at {
            Some(date) => filters::format_date(date),
            None => "Draft".to_string(),
        }
    }

    pub async fn published(
        pool: &SqlitePool,
        limit: i64,
        offset: i64,
        category: Option<&str>,
    ) -> Result<Vec<GeneratedArticle>, sqlx::Error> {
        match category {
            Some(cat) => {
                sqlx::query_as::<_, GeneratedArticle>(
                    "SELECT * FROM generated_articles WHERE status = 'published' AND category = ? ORDER BY published_at DESC LIMIT ? OFFSET ?"
                )
                .bind(cat)
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await
            }
            None => {
                sqlx::query_as::<_, GeneratedArticle>(
                    "SELECT * FROM generated_articles WHERE status = 'published' ORDER BY published_at DESC LIMIT ? OFFSET ?"
                )
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await
            }
        }
    }

    pub async fn published_categories(pool: &SqlitePool) -> Result<Vec<String>, sqlx::Error> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT category FROM generated_articles WHERE status = 'published' AND category IS NOT NULL ORDER BY category"
        )
        .fetch_all(pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    pub async fn published_global(
        pool: &SqlitePool,
        limit: i64,
        offset: i64,
        category: Option<&str>,
    ) -> Result<Vec<GeneratedArticle>, sqlx::Error> {
        match category {
            Some(cat) => {
                sqlx::query_as::<_, GeneratedArticle>(
                    "SELECT * FROM generated_articles WHERE status = 'published' AND user_id IS NULL AND category = ? ORDER BY published_at DESC LIMIT ? OFFSET ?"
                )
                .bind(cat)
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await
            }
            None => {
                sqlx::query_as::<_, GeneratedArticle>(
                    "SELECT * FROM generated_articles WHERE status = 'published' AND user_id IS NULL ORDER BY published_at DESC LIMIT ? OFFSET ?"
                )
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await
            }
        }
    }

    pub async fn published_categories_global(
        pool: &SqlitePool,
    ) -> Result<Vec<String>, sqlx::Error> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT category FROM generated_articles WHERE status = 'published' AND user_id IS NULL AND category IS NOT NULL ORDER BY category"
        )
        .fetch_all(pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    pub async fn published_for_user(
        pool: &SqlitePool,
        user_id: i64,
        limit: i64,
        offset: i64,
        category: Option<&str>,
    ) -> Result<Vec<GeneratedArticle>, sqlx::Error> {
        match category {
            Some(cat) => {
                sqlx::query_as::<_, GeneratedArticle>(
                    "SELECT * FROM generated_articles WHERE status = 'published' AND user_id = ? AND category = ? ORDER BY published_at DESC LIMIT ? OFFSET ?"
                )
                .bind(user_id)
                .bind(cat)
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await
            }
            None => {
                sqlx::query_as::<_, GeneratedArticle>(
                    "SELECT * FROM generated_articles WHERE status = 'published' AND user_id = ? ORDER BY published_at DESC LIMIT ? OFFSET ?"
                )
                .bind(user_id)
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await
            }
        }
    }

    pub async fn published_categories_for_user(
        pool: &SqlitePool,
        user_id: i64,
    ) -> Result<Vec<String>, sqlx::Error> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT category FROM generated_articles WHERE status = 'published' AND user_id = ? AND category IS NOT NULL ORDER BY category"
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    pub async fn all_categories(pool: &SqlitePool) -> Result<Vec<String>, sqlx::Error> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT category FROM generated_articles \
             WHERE category IS NOT NULL AND TRIM(category) <> '' \
             ORDER BY category"
        )
        .fetch_all(pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    pub async fn drafts(pool: &SqlitePool) -> Result<Vec<GeneratedArticle>, sqlx::Error> {
        sqlx::query_as::<_, GeneratedArticle>(
            "SELECT * FROM generated_articles WHERE status = 'draft' ORDER BY generated_at DESC"
        )
        .fetch_all(pool)
        .await
    }

    pub async fn drafts_global(pool: &SqlitePool) -> Result<Vec<GeneratedArticle>, sqlx::Error> {
        sqlx::query_as::<_, GeneratedArticle>(
            "SELECT * FROM generated_articles WHERE status = 'draft' AND user_id IS NULL ORDER BY generated_at DESC"
        )
        .fetch_all(pool)
        .await
    }

    pub async fn drafts_for_user(
        pool: &SqlitePool,
        user_id: i64,
    ) -> Result<Vec<GeneratedArticle>, sqlx::Error> {
        sqlx::query_as::<_, GeneratedArticle>(
            "SELECT * FROM generated_articles WHERE status = 'draft' AND user_id = ? ORDER BY generated_at DESC"
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
    }

    pub async fn drafts_with_owner(
        pool: &SqlitePool,
    ) -> Result<Vec<ArticleWithOwner>, sqlx::Error> {
        sqlx::query_as::<_, ArticleWithOwner>(
            "SELECT ga.*, u.username AS owner_username \
             FROM generated_articles ga LEFT JOIN users u ON u.id = ga.user_id \
             WHERE ga.status = 'draft' \
             ORDER BY ga.generated_at DESC",
        )
        .fetch_all(pool)
        .await
    }

    pub async fn by_slug(pool: &SqlitePool, slug: &str) -> Result<Option<GeneratedArticle>, sqlx::Error> {
        sqlx::query_as::<_, GeneratedArticle>(
            "SELECT * FROM generated_articles WHERE slug = ?"
        )
        .bind(slug)
        .fetch_optional(pool)
        .await
    }

    pub async fn by_id(pool: &SqlitePool, id: i64) -> Result<Option<GeneratedArticle>, sqlx::Error> {
        sqlx::query_as::<_, GeneratedArticle>(
            "SELECT * FROM generated_articles WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(pool)
        .await
    }

    pub async fn insert(
        pool: &SqlitePool,
        title: &str,
        slug: &str,
        summary: Option<&str>,
        category: Option<&str>,
        list_id: Option<i64>,
        user_id: Option<i64>,
    ) -> Result<i64, sqlx::Error> {
        let mut attempt_slug = slug.to_string();
        for n in 1..=20 {
            let result = sqlx::query(
                "INSERT INTO generated_articles (title, slug, summary, category, list_id, user_id) VALUES (?, ?, ?, ?, ?, ?)"
            )
            .bind(title)
            .bind(&attempt_slug)
            .bind(summary)
            .bind(category)
            .bind(list_id)
            .bind(user_id)
            .execute(pool)
            .await;

            match result {
                Ok(r) => return Ok(r.last_insert_rowid()),
                Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
                    attempt_slug = format!("{slug}-{}", n + 1);
                }
                Err(e) => return Err(e),
            }
        }
        Err(sqlx::Error::Protocol(format!(
            "could not find a unique slug for '{slug}' after 20 attempts"
        )))
    }

    pub async fn set_status_for_user(
        pool: &SqlitePool,
        id: i64,
        status: &str,
        user_id: i64,
    ) -> Result<bool, sqlx::Error> {
        let result = if status == "published" {
            sqlx::query("UPDATE generated_articles SET status = ?, published_at = datetime('now') WHERE id = ? AND user_id = ?")
                .bind(status)
                .bind(id)
                .bind(user_id)
                .execute(pool)
                .await?
        } else {
            sqlx::query("UPDATE generated_articles SET status = ?, published_at = NULL WHERE id = ? AND user_id = ?")
                .bind(status)
                .bind(id)
                .bind(user_id)
                .execute(pool)
                .await?
        };
        Ok(result.rows_affected() > 0)
    }

    pub async fn set_status_bulk_for_user(
        pool: &SqlitePool,
        ids: &[i64],
        status: &str,
        user_id: i64,
    ) -> Result<u64, sqlx::Error> {
        if ids.is_empty() {
            return Ok(0);
        }
        let placeholders = std::iter::repeat("?").take(ids.len()).collect::<Vec<_>>().join(",");
        let sql = if status == "published" {
            format!(
                "UPDATE generated_articles SET status = ?, published_at = datetime('now') WHERE user_id = ? AND id IN ({placeholders})"
            )
        } else {
            format!(
                "UPDATE generated_articles SET status = ?, published_at = NULL WHERE user_id = ? AND id IN ({placeholders})"
            )
        };
        let mut q = sqlx::query(&sql).bind(status).bind(user_id);
        for id in ids {
            q = q.bind(id);
        }
        let result = q.execute(pool).await?;
        Ok(result.rows_affected())
    }

    pub async fn set_status_for_admin(
        pool: &SqlitePool,
        id: i64,
        status: &str,
    ) -> Result<bool, sqlx::Error> {
        let result = if status == "published" {
            sqlx::query("UPDATE generated_articles SET status = ?, published_at = datetime('now') WHERE id = ? AND user_id IS NULL")
                .bind(status)
                .bind(id)
                .execute(pool)
                .await?
        } else {
            sqlx::query("UPDATE generated_articles SET status = ?, published_at = NULL WHERE id = ? AND user_id IS NULL")
                .bind(status)
                .bind(id)
                .execute(pool)
                .await?
        };
        Ok(result.rows_affected() > 0)
    }

    pub async fn set_status_bulk_for_admin(
        pool: &SqlitePool,
        ids: &[i64],
        status: &str,
    ) -> Result<u64, sqlx::Error> {
        if ids.is_empty() {
            return Ok(0);
        }
        let placeholders = std::iter::repeat("?").take(ids.len()).collect::<Vec<_>>().join(",");
        let sql = if status == "published" {
            format!(
                "UPDATE generated_articles SET status = ?, published_at = datetime('now') WHERE user_id IS NULL AND id IN ({placeholders})"
            )
        } else {
            format!(
                "UPDATE generated_articles SET status = ?, published_at = NULL WHERE user_id IS NULL AND id IN ({placeholders})"
            )
        };
        let mut q = sqlx::query(&sql).bind(status);
        for id in ids {
            q = q.bind(id);
        }
        let result = q.execute(pool).await?;
        Ok(result.rows_affected())
    }

    pub async fn set_category_for_admin(
        pool: &SqlitePool,
        id: i64,
        category: &str,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE generated_articles SET category = ? WHERE id = ? AND user_id IS NULL",
        )
        .bind(category)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn set_category_for_user(
        pool: &SqlitePool,
        id: i64,
        category: &str,
        user_id: i64,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("UPDATE generated_articles SET category = ? WHERE id = ? AND user_id = ?")
            .bind(category)
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn set_status(pool: &SqlitePool, id: i64, status: &str) -> Result<(), sqlx::Error> {
        if status == "published" {
            sqlx::query("UPDATE generated_articles SET status = ?, published_at = datetime('now') WHERE id = ?")
                .bind(status)
                .bind(id)
                .execute(pool)
                .await?;
        } else {
            sqlx::query("UPDATE generated_articles SET status = ?, published_at = NULL WHERE id = ?")
                .bind(status)
                .bind(id)
                .execute(pool)
                .await?;
        }
        Ok(())
    }

    pub async fn set_status_bulk(
        pool: &SqlitePool,
        ids: &[i64],
        status: &str,
    ) -> Result<u64, sqlx::Error> {
        if ids.is_empty() {
            return Ok(0);
        }
        let placeholders = std::iter::repeat("?").take(ids.len()).collect::<Vec<_>>().join(",");
        let sql = if status == "published" {
            format!(
                "UPDATE generated_articles SET status = ?, published_at = datetime('now') WHERE id IN ({placeholders})"
            )
        } else {
            format!(
                "UPDATE generated_articles SET status = ?, published_at = NULL WHERE id IN ({placeholders})"
            )
        };
        let mut q = sqlx::query(&sql).bind(status);
        for id in ids {
            q = q.bind(id);
        }
        let result = q.execute(pool).await?;
        Ok(result.rows_affected())
    }

    pub async fn all_published(pool: &SqlitePool) -> Result<Vec<GeneratedArticle>, sqlx::Error> {
        sqlx::query_as::<_, GeneratedArticle>(
            "SELECT * FROM generated_articles WHERE status = 'published' ORDER BY published_at DESC"
        )
        .fetch_all(pool)
        .await
    }

    pub async fn all_published_global(
        pool: &SqlitePool,
    ) -> Result<Vec<GeneratedArticle>, sqlx::Error> {
        sqlx::query_as::<_, GeneratedArticle>(
            "SELECT * FROM generated_articles WHERE status = 'published' AND user_id IS NULL ORDER BY published_at DESC"
        )
        .fetch_all(pool)
        .await
    }

    pub async fn all_published_for_user(
        pool: &SqlitePool,
        user_id: i64,
    ) -> Result<Vec<GeneratedArticle>, sqlx::Error> {
        sqlx::query_as::<_, GeneratedArticle>(
            "SELECT * FROM generated_articles WHERE status = 'published' AND user_id = ? ORDER BY published_at DESC"
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
    }

    pub async fn all_published_with_owner(
        pool: &SqlitePool,
    ) -> Result<Vec<ArticleWithOwner>, sqlx::Error> {
        sqlx::query_as::<_, ArticleWithOwner>(
            "SELECT ga.*, u.username AS owner_username \
             FROM generated_articles ga LEFT JOIN users u ON u.id = ga.user_id \
             WHERE ga.status = 'published' \
             ORDER BY ga.published_at DESC",
        )
        .fetch_all(pool)
        .await
    }

    pub async fn set_category(pool: &SqlitePool, id: i64, category: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE generated_articles SET category = ? WHERE id = ?")
            .bind(category)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn count_published(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM generated_articles WHERE status = 'published'")
            .fetch_one(pool)
            .await?;
        Ok(row.0)
    }
}
