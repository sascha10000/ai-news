use crate::filters;
use sqlx::SqlitePool;

pub const CATEGORIES: &[&str] = &[
    "Technology",
    "Politics",
    "Business",
    "Science",
    "Health",
    "Sports",
    "Entertainment",
    "World",
    "Environment",
    "Other",
];

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

    pub async fn drafts(pool: &SqlitePool) -> Result<Vec<GeneratedArticle>, sqlx::Error> {
        sqlx::query_as::<_, GeneratedArticle>(
            "SELECT * FROM generated_articles WHERE status = 'draft' ORDER BY generated_at DESC"
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
    ) -> Result<i64, sqlx::Error> {
        let result = sqlx::query(
            "INSERT INTO generated_articles (title, slug, summary, category, list_id) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(title)
        .bind(slug)
        .bind(summary)
        .bind(category)
        .bind(list_id)
        .execute(pool)
        .await?;
        Ok(result.last_insert_rowid())
    }

    pub async fn set_status(pool: &SqlitePool, id: i64, status: &str) -> Result<(), sqlx::Error> {
        if status == "published" {
            sqlx::query("UPDATE generated_articles SET status = ?, published_at = datetime('now') WHERE id = ?")
                .bind(status)
                .bind(id)
                .execute(pool)
                .await?;
        } else {
            sqlx::query("UPDATE generated_articles SET status = ? WHERE id = ?")
                .bind(status)
                .bind(id)
                .execute(pool)
                .await?;
        }
        Ok(())
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
