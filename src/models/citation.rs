use crate::filters;
use sqlx::SqlitePool;

#[derive(sqlx::FromRow, Clone)]
pub struct GeneratedSentence {
    pub id: i64,
    pub generated_article_id: i64,
    pub position: i32,
    pub content: String,
}

#[derive(sqlx::FromRow, Clone)]
pub struct SentenceCitation {
    pub id: i64,
    pub sentence_id: i64,
    pub source_article_id: i64,
}

#[derive(Clone)]
pub struct SentenceWithSources {
    pub position: i32,
    pub content: String,
    pub sources: Vec<SourceRef>,
}

#[derive(sqlx::FromRow, Clone)]
pub struct SourceRef {
    pub id: i64,
    pub title: String,
    pub url: String,
    pub published_at: Option<String>,
}

impl SourceRef {
    pub fn formatted_date(&self) -> Option<String> {
        self.published_at.as_deref().map(filters::format_date)
    }
}

impl GeneratedSentence {
    pub async fn insert(
        pool: &SqlitePool,
        article_id: i64,
        position: i32,
        content: &str,
    ) -> Result<i64, sqlx::Error> {
        let result = sqlx::query(
            "INSERT INTO generated_sentences (generated_article_id, position, content) VALUES (?, ?, ?)"
        )
        .bind(article_id)
        .bind(position)
        .bind(content)
        .execute(pool)
        .await?;
        Ok(result.last_insert_rowid())
    }
}

impl SentenceCitation {
    pub async fn insert(
        pool: &SqlitePool,
        sentence_id: i64,
        source_article_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO sentence_citations (sentence_id, source_article_id) VALUES (?, ?)"
        )
        .bind(sentence_id)
        .bind(source_article_id)
        .execute(pool)
        .await?;
        Ok(())
    }
}

pub async fn sentences_with_sources(
    pool: &SqlitePool,
    article_id: i64,
) -> Result<Vec<SentenceWithSources>, sqlx::Error> {
    let sentences = sqlx::query_as::<_, GeneratedSentence>(
        "SELECT * FROM generated_sentences WHERE generated_article_id = ? ORDER BY position"
    )
    .bind(article_id)
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for sentence in sentences {
        let sources = sqlx::query_as::<_, SourceRef>(
            "SELECT sa.id, sa.title, sa.url, sa.published_at FROM sentence_citations sc
             JOIN source_articles sa ON sa.id = sc.source_article_id
             WHERE sc.sentence_id = ?"
        )
        .bind(sentence.id)
        .fetch_all(pool)
        .await?;

        result.push(SentenceWithSources {
            position: sentence.position,
            content: sentence.content,
            sources,
        });
    }

    Ok(result)
}

pub async fn all_sources_for_article(
    pool: &SqlitePool,
    article_id: i64,
) -> Result<Vec<SourceRef>, sqlx::Error> {
    sqlx::query_as::<_, SourceRef>(
        "SELECT DISTINCT sa.id, sa.title, sa.url, sa.published_at FROM sentence_citations sc
         JOIN generated_sentences gs ON gs.id = sc.sentence_id
         JOIN source_articles sa ON sa.id = sc.source_article_id
         WHERE gs.generated_article_id = ?
         ORDER BY sa.id"
    )
    .bind(article_id)
    .fetch_all(pool)
    .await
}
