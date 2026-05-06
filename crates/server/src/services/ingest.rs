use crate::error::AppError;
use crate::models::citation::{GeneratedSentence, SentenceCitation};
use crate::models::generated_article::GeneratedArticle;
use ai_news_core::IngestArticlesRequest;
use sqlx::SqlitePool;

pub async fn ingest_articles(
    pool: &SqlitePool,
    req: IngestArticlesRequest,
) -> Result<Vec<i64>, AppError> {
    let mut created = Vec::new();

    for article in req.articles {
        if article.sentences.is_empty() {
            tracing::warn!("Skipping ingest of '{}': no sentences", article.title);
            continue;
        }

        let summary = article
            .summary
            .clone()
            .or_else(|| article.sentences.first().map(|s| s.content.clone()));

        let id = GeneratedArticle::insert(
            pool,
            &article.title,
            &article.slug,
            summary.as_deref(),
            article.category.as_deref(),
            article.list_id,
        )
        .await?;

        for sentence in &article.sentences {
            let sentence_id =
                GeneratedSentence::insert(pool, id, sentence.position, &sentence.content).await?;
            for &source_id in &sentence.source_article_ids {
                SentenceCitation::insert(pool, sentence_id, source_id).await?;
            }
        }

        tracing::info!("Ingested article '{}' (id={})", article.title, id);
        created.push(id);
    }

    Ok(created)
}
