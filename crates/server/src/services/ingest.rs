use std::collections::HashMap;

use crate::error::AppError;
use crate::models::app_settings::AppSettings;
use crate::models::citation::{GeneratedSentence, SentenceCitation};
use crate::models::generated_article::GeneratedArticle;
use crate::models::list::List;
use ai_news_core::IngestArticlesRequest;
use sqlx::SqlitePool;

pub async fn ingest_articles(
    pool: &SqlitePool,
    req: IngestArticlesRequest,
) -> Result<Vec<i64>, AppError> {
    let mut created = Vec::new();
    let mut owner_cache: HashMap<i64, Option<i64>> = HashMap::new();
    let auto_publish = AppSettings::auto_publish(pool).await?;

    for article in req.articles {
        if article.sentences.is_empty() {
            tracing::warn!("Skipping ingest of '{}': no sentences", article.title);
            continue;
        }

        let summary = article
            .summary
            .clone()
            .or_else(|| article.sentences.first().map(|s| s.content.clone()));

        let user_id = match article.list_id {
            Some(lid) => match owner_cache.get(&lid) {
                Some(cached) => *cached,
                None => {
                    let owner = List::owner_of(pool, lid).await?;
                    owner_cache.insert(lid, owner);
                    owner
                }
            },
            None => article.user_id,
        };

        let id = GeneratedArticle::insert(
            pool,
            &article.title,
            &article.slug,
            summary.as_deref(),
            article.category.as_deref(),
            article.list_id,
            user_id,
        )
        .await?;

        for sentence in &article.sentences {
            let sentence_id =
                GeneratedSentence::insert(pool, id, sentence.position, &sentence.content).await?;
            for &source_id in &sentence.source_article_ids {
                SentenceCitation::insert(pool, sentence_id, source_id).await?;
            }
        }

        // Auto-publish only applies to global (admin-owned) articles so that
        // user-owned drafts still go through the user's own review queue.
        if auto_publish && user_id.is_none() {
            GeneratedArticle::set_status(pool, id, "published").await?;
        }

        tracing::info!("Ingested article '{}' (id={})", article.title, id);
        created.push(id);
    }

    Ok(created)
}
