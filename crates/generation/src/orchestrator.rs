use crate::clusterer;
use crate::llm;
use crate::GenerationError;
use ai_news_core::{IngestArticleRequest, IngestSentence, PendingSource};
use ollama_rs::Ollama;
use slug::slugify;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct OllamaConfig {
    pub host: String,
    pub model: String,
}

fn build_ollama(cfg: &OllamaConfig) -> Ollama {
    Ollama::new(cfg.host.clone(), 11434)
}

pub async fn check_model_available(cfg: &OllamaConfig) -> Result<(), GenerationError> {
    let ollama = build_ollama(cfg);
    llm::check_model_available(&ollama, &cfg.model).await
}

pub async fn generate_drafts(
    sources: Vec<PendingSource>,
    cfg: &OllamaConfig,
) -> Result<Vec<IngestArticleRequest>, GenerationError> {
    if sources.is_empty() {
        tracing::info!("No source articles to process");
        return Ok(vec![]);
    }

    let clusters = clusterer::cluster_articles(&sources);
    if clusters.is_empty() {
        tracing::info!("No clusters formed (need at least 2 related articles)");
        return Ok(vec![]);
    }

    let ollama = build_ollama(cfg);
    let mut drafts = Vec::new();

    for cluster in &clusters {
        match generate_one(&ollama, &cfg.model, cluster).await {
            Ok(draft) => drafts.push(draft),
            Err(e) => tracing::error!("Failed to generate article for cluster: {e}"),
        }
    }

    Ok(drafts)
}

async fn generate_one(
    ollama: &Ollama,
    model: &str,
    sources: &[&PendingSource],
) -> Result<IngestArticleRequest, GenerationError> {
    let prompt = llm::build_prompt(sources);
    let valid_ids: HashSet<i64> = sources.iter().map(|a| a.id).collect();

    let mut last_err = None;
    for attempt in 0..3 {
        let raw = llm::call_ollama(ollama, model, &prompt).await?;
        match llm::parse_response(&raw, &valid_ids) {
            Ok(output) => {
                let slug = slugify(&output.title);
                let summary = output.sentences.first().map(|s| s.text.clone());

                let sentences = output
                    .sentences
                    .into_iter()
                    .enumerate()
                    .map(|(i, s)| IngestSentence {
                        position: i as i32,
                        content: s.text,
                        source_article_ids: s.sources,
                    })
                    .collect();

                tracing::info!("Drafted article '{}'", output.title);
                return Ok(IngestArticleRequest {
                    title: output.title,
                    slug,
                    summary,
                    category: output.category,
                    sentences,
                });
            }
            Err(e) => {
                tracing::warn!("Generation attempt {} failed: {e}", attempt + 1);
                last_err = Some(e);
            }
        }
    }

    Err(last_err.unwrap_or_else(|| GenerationError::Llm("Unknown generation error".to_string())))
}
