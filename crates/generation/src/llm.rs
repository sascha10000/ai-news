use crate::GenerationError;
use ai_news_core::PendingSource;
use ollama_rs::generation::completion::request::GenerationRequest;
use ollama_rs::Ollama;
use serde::Deserialize;
use std::collections::HashSet;

#[derive(Deserialize, Debug)]
pub struct LlmArticleOutput {
    pub title: String,
    pub category: Option<String>,
    pub sentences: Vec<LlmSentence>,
}

#[derive(Deserialize, Debug)]
pub struct LlmSentence {
    pub text: String,
    pub sources: Vec<i64>,
}

pub fn build_prompt(articles: &[&PendingSource], target_language: Option<&str>) -> String {
    let mut prompt = String::from(
        r#"You are a professional news journalist. You will be given a set of source articles, each identified by a numeric ID and a publication date. Your task is to write a NEW synthesized news article that covers the topic.

STRICT RULES:
1. Every sentence you write MUST be grounded in one or more source articles.
2. You MUST output valid JSON and nothing else.
3. The JSON format is: {"title": "...", "category": "...", "sentences": [{"text": "...", "sources": [1, 3]}, ...]}
4. Each object in "sentences" has "text" (one sentence) and "sources" (array of source IDs used).
5. "category" is a short Title Case label (1-2 words) describing the article's topic. Reuse a common label when one fits naturally (e.g. Technology, Politics, Business, Science, Health, Sports, Entertainment, World, Environment); otherwise pick a more specific label that reflects the topic. Avoid invented or quirky labels — prefer the same wording you'd see on a newspaper section.
6. Never fabricate information not present in the sources.
7. Write 5-15 sentences. Be concise and journalistic.
8. When sources span multiple dates, weight the most recent reporting for the lede, headline framing, and present-tense facts. Older sources are background context only — do not present stale information as current. If only old sources are available, write the piece as a retrospective rather than implying it is breaking news.
9. Do NOT include any text outside the JSON object.
"#,
    );

    if let Some(language) = target_language {
        // Note: we do NOT translate JSON keys. If we said "write in German",
        // some models translate "title"/"category"/"sentences" too, which
        // then fails serde parsing downstream. Explicit is safer than clever.
        prompt.push_str(&format!(
            "10. Write the article in {language}. Translate the source content as needed. Keep the JSON keys (\"title\", \"category\", \"sentences\", \"text\", \"sources\") exactly as shown — only the human-readable string values should be in {language}. The \"category\" value stays in English so categories stay consistent across users.\n",
            language = language,
        ));
    }

    prompt.push_str("\nSOURCE ARTICLES (sorted newest first):\n");

    for article in articles {
        let content = if article.content.len() > 1500 {
            &article.content[..1500]
        } else {
            &article.content
        };
        let date_str = article
            .published_at
            .as_deref()
            .map(|s| if s.len() >= 10 { &s[..10] } else { s })
            .unwrap_or("unknown");
        prompt.push_str(&format!(
            "\n[ID: {} | Published: {}] Title: {}\n{}\n",
            article.id, date_str, article.title, content
        ));
    }

    prompt.push_str("\nRespond ONLY with the JSON object:");
    prompt
}

pub async fn check_model_available(ollama: &Ollama, model: &str) -> Result<(), GenerationError> {
    let models = ollama
        .list_local_models()
        .await
        .map_err(|e| GenerationError::Llm(format!("Could not reach Ollama at startup: {e}")))?;

    let wanted_with_latest = if model.contains(':') {
        model.to_string()
    } else {
        format!("{model}:latest")
    };

    let found = models
        .iter()
        .any(|m| m.name == model || m.name == wanted_with_latest);

    if found {
        tracing::info!("Ollama model '{model}' is available");
        Ok(())
    } else {
        let available: Vec<&str> = models.iter().map(|m| m.name.as_str()).collect();
        Err(GenerationError::Llm(format!(
            "Configured OLLAMA_MODEL '{model}' is not installed. Available models: [{}]. \
             Run `ollama pull {model}` to install it.",
            available.join(", ")
        )))
    }
}

pub async fn call_ollama(
    ollama: &Ollama,
    model: &str,
    prompt: &str,
) -> Result<String, GenerationError> {
    let request = GenerationRequest::new(model.to_string(), prompt.to_string())
        .format(ollama_rs::generation::parameters::FormatType::Json);

    let response = ollama
        .generate(request)
        .await
        .map_err(|e| GenerationError::Llm(format!("Ollama call failed: {e}")))?;

    Ok(response.response)
}

pub fn parse_response(
    raw: &str,
    valid_source_ids: &HashSet<i64>,
) -> Result<LlmArticleOutput, GenerationError> {
    let cleaned = raw.trim();
    let cleaned = if cleaned.starts_with("```") {
        let inner = cleaned
            .strip_prefix("```json")
            .or_else(|| cleaned.strip_prefix("```"))
            .unwrap_or(cleaned);
        inner.strip_suffix("```").unwrap_or(inner).trim()
    } else {
        cleaned
    };

    let start = cleaned.find('{').ok_or_else(|| {
        GenerationError::Llm(format!(
            "No JSON object found in LLM response: {}",
            &raw[..raw.len().min(200)]
        ))
    })?;
    let end = cleaned
        .rfind('}')
        .ok_or_else(|| GenerationError::Llm("No closing brace found in LLM response".to_string()))?;
    let json_str = &cleaned[start..=end];

    let mut output: LlmArticleOutput = serde_json::from_str(json_str)
        .map_err(|e| GenerationError::Llm(format!("Failed to parse LLM JSON: {e}")))?;

    output.category = output.category.and_then(|c| {
        let normalized = c.split_whitespace().collect::<Vec<_>>().join(" ");
        if normalized.is_empty() {
            None
        } else {
            Some(normalized)
        }
    });

    for sentence in &mut output.sentences {
        sentence.sources.retain(|id| valid_source_ids.contains(id));
    }

    output.sentences.retain(|s| !s.sources.is_empty());

    if output.sentences.is_empty() {
        return Err(GenerationError::Llm(
            "All sentences had invalid source IDs".to_string(),
        ));
    }

    Ok(output)
}
