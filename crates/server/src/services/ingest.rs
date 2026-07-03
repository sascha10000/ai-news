use std::collections::HashMap;

use crate::error::AppError;
use crate::models::app_settings::AppSettings;
use crate::models::citation::{GeneratedSentence, SentenceCitation};
use crate::models::generated_article::GeneratedArticle;
use crate::models::list::List;
use crate::models::user::User;
use ai_news_core::IngestArticlesRequest;
use sqlx::SqlitePool;

pub async fn ingest_articles(
    pool: &SqlitePool,
    req: IngestArticlesRequest,
) -> Result<Vec<i64>, AppError> {
    let mut created = Vec::new();
    let mut owner_cache: HashMap<i64, Option<i64>> = HashMap::new();
    // Per-user auto-publish flags, cached so a batch touching one user's lists
    // doesn't re-query the users table for every article.
    let mut user_auto_publish_cache: HashMap<i64, bool> = HashMap::new();
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

        // Auto-publish is decided per owner: global (admin-owned) articles use
        // the global `app_settings` flag, while user-owned articles use that
        // user's own per-user preference. Either way, publishing is opt-in.
        let should_publish = match user_id {
            None => auto_publish,
            Some(uid) => match user_auto_publish_cache.get(&uid) {
                Some(cached) => *cached,
                None => {
                    let enabled = User::auto_publish_of(pool, uid).await?;
                    user_auto_publish_cache.insert(uid, enabled);
                    enabled
                }
            },
        };
        if should_publish {
            GeneratedArticle::set_status(pool, id, "published").await?;
        }

        tracing::info!("Ingested article '{}' (id={})", article.title, id);
        created.push(id);
    }

    Ok(created)
}
