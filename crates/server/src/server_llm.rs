use crate::error::AppError;
use crate::models::list::List;
use crate::models::source_article::SourceArticle;
use crate::models::user::{language_label, User};
use crate::services::ingest;
use crate::AppState;
use ai_news_core::{IngestArticlesRequest, PendingSource};
use ai_news_generation::{generate_drafts_for_list, OllamaConfig};

pub async fn run_unscoped_generation(state: &AppState) -> Result<Vec<i64>, AppError> {
    run_for_scope(state, None).await
}

pub async fn run_list_generation(state: &AppState, list_id: i64) -> Result<Vec<i64>, AppError> {
    run_for_scope(state, Some(list_id)).await
}

pub async fn run_all_lists_generation(state: &AppState) -> Result<Vec<i64>, AppError> {
    let lists = List::all(&state.db).await?;
    let mut all_ids = Vec::new();
    for list in &lists {
        match run_for_scope(state, Some(list.id)).await {
            Ok(mut ids) => all_ids.append(&mut ids),
            Err(e) => tracing::error!("Generation for list '{}' failed: {e}", list.name),
        }
    }
    Ok(all_ids)
}

async fn run_for_scope(state: &AppState, list_id: Option<i64>) -> Result<Vec<i64>, AppError> {
    let articles = SourceArticle::recent_uncited(
        &state.db,
        48,
        state.max_source_age_days as i64,
        list_id,
    )
    .await?;
    let sources: Vec<PendingSource> = articles.into_iter().map(to_pending_source).collect();

    // Language preference travels with the list's owning user. Admin/global
    // lists (user_id = NULL) fall through to None → LLM default.
    let target_language = resolve_list_language(state, list_id).await?;

    let drafts = generate_drafts_for_list(
        sources,
        &state.ollama_cfg,
        list_id,
        target_language.as_deref(),
    )
    .await
    .map_err(|e| AppError::Llm(e.to_string()))?;

    if drafts.is_empty() {
        return Ok(vec![]);
    }

    ingest::ingest_articles(&state.db, IngestArticlesRequest { articles: drafts }).await
}

async fn resolve_list_language(
    state: &AppState,
    list_id: Option<i64>,
) -> Result<Option<String>, AppError> {
    let Some(list_id) = list_id else { return Ok(None) };
    let Some(owner_id) = List::owner_of(&state.db, list_id).await? else {
        return Ok(None);
    };
    let code = User::language_of(&state.db, owner_id).await?;
    Ok(code.and_then(|c| language_label(&c).map(|name| name.to_string())))
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
