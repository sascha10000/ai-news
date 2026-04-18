use crate::error::AppError;
use crate::models::citation::{GeneratedSentence, SentenceCitation};
use crate::models::generated_article::GeneratedArticle;
use crate::models::source_article::SourceArticle;
use crate::services::{llm, topic_clusterer};
use ollama_rs::Ollama;
use slug::slugify;
use sqlx::SqlitePool;
use std::collections::HashSet;

pub async fn generate_articles(
    pool: &SqlitePool,
    ollama: &Ollama,
    model: &str,
) -> Result<Vec<i64>, AppError> {
    let articles = SourceArticle::recent_uncited(pool, 48).await?;
    if articles.is_empty() {
        tracing::info!("No uncited source articles found");
        return Ok(vec![]);
    }

    let clusters = topic_clusterer::cluster_articles(&articles);
    if clusters.is_empty() {
        tracing::info!("No clusters formed (need at least 2 related articles)");
        return Ok(vec![]);
    }

    let mut generated_ids = Vec::new();

    for cluster in &clusters {
        match generate_one(pool, ollama, model, cluster).await {
            Ok(id) => generated_ids.push(id),
            Err(e) => tracing::error!("Failed to generate article for cluster: {e}"),
        }
    }

    Ok(generated_ids)
}

async fn generate_one(
    pool: &SqlitePool,
    ollama: &Ollama,
    model: &str,
    sources: &[&SourceArticle],
) -> Result<i64, AppError> {
    let prompt = llm::build_prompt(sources);
    let valid_ids: HashSet<i64> = sources.iter().map(|a| a.id).collect();

    // Try up to 3 times
    let mut last_err = None;
    for attempt in 0..3 {
        let raw = llm::call_ollama(ollama, model, &prompt).await?;
        match llm::parse_response(&raw, &valid_ids) {
            Ok(output) => {
                let slug = slugify(&output.title);
                let summary = output
                    .sentences
                    .first()
                    .map(|s| s.text.as_str());

                let article_id = GeneratedArticle::insert(
                    pool,
                    &output.title,
                    &slug,
                    summary,
                    output.category.as_deref(),
                )
                .await?;

                for (i, sentence) in output.sentences.iter().enumerate() {
                    let sentence_id = GeneratedSentence::insert(
                        pool,
                        article_id,
                        i as i32,
                        &sentence.text,
                    )
                    .await?;

                    for &source_id in &sentence.sources {
                        SentenceCitation::insert(pool, sentence_id, source_id).await?;
                    }
                }

                tracing::info!("Generated article '{}' (id={})", output.title, article_id);
                return Ok(article_id);
            }
            Err(e) => {
                tracing::warn!("Generation attempt {} failed: {e}", attempt + 1);
                last_err = Some(e);
            }
        }
    }

    Err(last_err.unwrap_or_else(|| AppError::Llm("Unknown generation error".to_string())))
}
