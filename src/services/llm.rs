use crate::error::AppError;
use crate::models::generated_article::CATEGORIES;
use crate::models::source_article::SourceArticle;
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

pub fn build_prompt(articles: &[&SourceArticle]) -> String {
    let mut prompt = String::from(
        r#"You are a professional news journalist. You will be given a set of source articles, each identified by a numeric ID. Your task is to write a NEW synthesized news article that covers the topic.

STRICT RULES:
1. Every sentence you write MUST be grounded in one or more source articles.
2. You MUST output valid JSON and nothing else.
3. The JSON format is: {"title": "...", "category": "...", "sentences": [{"text": "...", "sources": [1, 3]}, ...]}
4. Each object in "sentences" has "text" (one sentence) and "sources" (array of source IDs used).
5. "category" MUST be exactly one of: Technology, Politics, Business, Science, Health, Sports, Entertainment, World, Environment, Other.
6. Never fabricate information not present in the sources.
7. Write 5-15 sentences. Be concise and journalistic.
8. Do NOT include any text outside the JSON object.

SOURCE ARTICLES:
"#,
    );

    for article in articles {
        let content = if article.content.len() > 1500 {
            &article.content[..1500]
        } else {
            &article.content
        };
        prompt.push_str(&format!(
            "\n[ID: {}] Title: {}\n{}\n",
            article.id, article.title, content
        ));
    }

    prompt.push_str("\nRespond ONLY with the JSON object:");
    prompt
}

pub async fn check_model_available(ollama: &Ollama, model: &str) -> Result<(), AppError> {
    let models = ollama
        .list_local_models()
        .await
        .map_err(|e| AppError::Llm(format!("Could not reach Ollama at startup: {e}")))?;

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
        Err(AppError::Llm(format!(
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
) -> Result<String, AppError> {
    let request = GenerationRequest::new(model.to_string(), prompt.to_string())
        .format(ollama_rs::generation::parameters::FormatType::Json);

    let response = ollama
        .generate(request)
        .await
        .map_err(|e| AppError::Llm(format!("Ollama call failed: {e}")))?;

    Ok(response.response)
}

pub fn parse_response(
    raw: &str,
    valid_source_ids: &HashSet<i64>,
) -> Result<LlmArticleOutput, AppError> {
    // Strip markdown fences if present
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

    // Find the JSON object
    let start = cleaned.find('{').ok_or_else(|| {
        AppError::Llm(format!("No JSON object found in LLM response: {}", &raw[..raw.len().min(200)]))
    })?;
    let end = cleaned.rfind('}').ok_or_else(|| {
        AppError::Llm("No closing brace found in LLM response".to_string())
    })?;
    let json_str = &cleaned[start..=end];

    let mut output: LlmArticleOutput = serde_json::from_str(json_str)
        .map_err(|e| AppError::Llm(format!("Failed to parse LLM JSON: {e}")))?;

    // Validate category
    if let Some(ref cat) = output.category {
        if !CATEGORIES.iter().any(|c| c.eq_ignore_ascii_case(cat)) {
            output.category = Some("Other".to_string());
        }
    }

    // Validate source IDs
    for sentence in &mut output.sentences {
        sentence.sources.retain(|id| valid_source_ids.contains(id));
    }

    // Remove sentences with no valid sources
    output.sentences.retain(|s| !s.sources.is_empty());

    if output.sentences.is_empty() {
        return Err(AppError::Llm("All sentences had invalid source IDs".to_string()));
    }

    Ok(output)
}
