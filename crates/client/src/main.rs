use std::env;
use std::time::Duration;

use ai_news_core::{IngestArticlesRequest, IngestArticlesResponse, PendingSourcesResponse};
use ai_news_generation::{check_model_available, generate_drafts, OllamaConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    let server_url = env::var("SERVER_URL")
        .map_err(|_| anyhow::anyhow!("SERVER_URL must be set (e.g. http://localhost:3000)"))?;
    let server_url = server_url.trim_end_matches('/').to_string();
    let api_token =
        env::var("API_TOKEN").map_err(|_| anyhow::anyhow!("API_TOKEN must be set"))?;
    let ollama_cfg = OllamaConfig {
        host: env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".to_string()),
        model: env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama3.2:latest".to_string()),
    };

    check_model_available(&ollama_cfg).await?;

    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()?;

    tracing::info!("Fetching pending sources from {server_url}...");
    let pending: PendingSourcesResponse = http
        .get(format!("{server_url}/api/sources/pending"))
        .bearer_auth(&api_token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let n_sources = pending.sources.len();
    tracing::info!("Got {n_sources} pending source(s)");

    if n_sources == 0 {
        println!("Nothing to do (0 sources, 0 drafts uploaded).");
        return Ok(());
    }

    let drafts = generate_drafts(pending.sources, &ollama_cfg).await?;
    let n_drafts = drafts.len();
    tracing::info!("Generated {n_drafts} draft(s)");

    if drafts.is_empty() {
        println!("{n_sources} sources \u{2192} 0 clusters \u{2192} 0 drafts uploaded.");
        return Ok(());
    }

    let response: IngestArticlesResponse = http
        .post(format!("{server_url}/api/articles/ingest"))
        .bearer_auth(&api_token)
        .json(&IngestArticlesRequest { articles: drafts })
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    println!(
        "{n_sources} sources \u{2192} {n_drafts} drafts \u{2192} {} uploaded (ids: {:?})",
        response.created.len(),
        response.created
    );
    Ok(())
}
