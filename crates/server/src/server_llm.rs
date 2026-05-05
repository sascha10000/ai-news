use crate::error::AppError;
use crate::models::source_article::SourceArticle;
use crate::services::ingest;
use crate::AppState;
use ai_news_core::{IngestArticlesRequest, PendingSource};
use ai_news_generation::{generate_drafts, OllamaConfig};

pub async fn run_local_generation(state: &AppState) -> Result<Vec<i64>, AppError> {
    let articles = SourceArticle::recent_uncited(&state.db, 48).await?;
    let sources: Vec<PendingSource> = articles.into_iter().map(to_pending_source).collect();

    let drafts = generate_drafts(sources, &state.ollama_cfg)
        .await
        .map_err(|e| AppError::Llm(e.to_string()))?;

    if drafts.is_empty() {
        return Ok(vec![]);
    }

    ingest::ingest_articles(&state.db, IngestArticlesRequest { articles: drafts }).await
}

pub fn to_pending_source(a: SourceArticle) -> PendingSource {
    PendingSource {
        id: a.id,
        feed_id: a.feed_id,
        title: a.title,
        url: a.url,
        content: a.content,
        summary: a.summary,
        published_at: a.published_at,
    }
}

pub fn ollama_config_from(cfg: &crate::config::Config) -> OllamaConfig {
    OllamaConfig {
        host: cfg.ollama_host.clone(),
        model: cfg.ollama_model.clone(),
    }
}
