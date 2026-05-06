use std::env;
use std::time::Duration;

use ai_news_core::{
    IngestArticleRequest, IngestArticlesRequest, IngestArticlesResponse, ListSummary,
    ListsResponse, PendingSourcesResponse,
};
use ai_news_generation::{check_model_available, generate_drafts_for_list, OllamaConfig};

enum Mode {
    Unscoped,
    List(i64),
    AllLists,
}

fn parse_mode() -> anyhow::Result<Mode> {
    let args: Vec<String> = env::args().collect();
    if args.iter().any(|a| a == "--all-lists") {
        return Ok(Mode::AllLists);
    }
    if let Some(pos) = args.iter().position(|a| a == "--list") {
        let id: i64 = args
            .get(pos + 1)
            .ok_or_else(|| anyhow::anyhow!("--list requires a list id"))?
            .parse()
            .map_err(|_| anyhow::anyhow!("--list requires a numeric list id"))?;
        return Ok(Mode::List(id));
    }
    Ok(Mode::Unscoped)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    let mode = parse_mode()?;

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

    match mode {
        Mode::Unscoped => {
            let summary = run_one(&http, &server_url, &api_token, &ollama_cfg, None, "unscoped").await?;
            print_summary(&[summary]);
        }
        Mode::List(list_id) => {
            let summary = run_one(
                &http,
                &server_url,
                &api_token,
                &ollama_cfg,
                Some(list_id),
                &format!("list {list_id}"),
            )
            .await?;
            print_summary(&[summary]);
        }
        Mode::AllLists => {
            let lists = fetch_lists(&http, &server_url, &api_token).await?;
            if lists.is_empty() {
                println!("No lists configured. Create lists in the admin UI first.");
                return Ok(());
            }
            tracing::info!("Sweeping {} list(s)", lists.len());
            let mut summaries = Vec::with_capacity(lists.len());
            for list in &lists {
                let label = format!("list '{}'", list.name);
                match run_one(
                    &http,
                    &server_url,
                    &api_token,
                    &ollama_cfg,
                    Some(list.id),
                    &label,
                )
                .await
                {
                    Ok(s) => summaries.push(s),
                    Err(e) => tracing::error!("List '{}' failed: {e}", list.name),
                }
            }
            print_summary(&summaries);
        }
    }
    Ok(())
}

struct RunSummary {
    label: String,
    sources: usize,
    drafts: usize,
    uploaded: Vec<i64>,
}

async fn run_one(
    http: &reqwest::Client,
    server_url: &str,
    api_token: &str,
    ollama_cfg: &OllamaConfig,
    list_id: Option<i64>,
    label: &str,
) -> anyhow::Result<RunSummary> {
    tracing::info!("[{label}] fetching pending sources...");
    let mut req = http
        .get(format!("{server_url}/api/sources/pending"))
        .bearer_auth(api_token);
    if let Some(id) = list_id {
        req = req.query(&[("list_id", id)]);
    }

    let pending: PendingSourcesResponse = req.send().await?.error_for_status()?.json().await?;
    let n_sources = pending.sources.len();
    tracing::info!("[{label}] {n_sources} pending source(s)");

    if n_sources == 0 {
        return Ok(RunSummary {
            label: label.to_string(),
            sources: 0,
            drafts: 0,
            uploaded: vec![],
        });
    }

    let drafts: Vec<IngestArticleRequest> =
        generate_drafts_for_list(pending.sources, ollama_cfg, list_id).await?;
    let n_drafts = drafts.len();
    tracing::info!("[{label}] generated {n_drafts} draft(s)");

    if drafts.is_empty() {
        return Ok(RunSummary {
            label: label.to_string(),
            sources: n_sources,
            drafts: 0,
            uploaded: vec![],
        });
    }

    let response: IngestArticlesResponse = http
        .post(format!("{server_url}/api/articles/ingest"))
        .bearer_auth(api_token)
        .json(&IngestArticlesRequest { articles: drafts })
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(RunSummary {
        label: label.to_string(),
        sources: n_sources,
        drafts: n_drafts,
        uploaded: response.created,
    })
}

async fn fetch_lists(
    http: &reqwest::Client,
    server_url: &str,
    api_token: &str,
) -> anyhow::Result<Vec<ListSummary>> {
    let body: ListsResponse = http
        .get(format!("{server_url}/api/lists"))
        .bearer_auth(api_token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(body.lists)
}

fn print_summary(runs: &[RunSummary]) {
    for r in runs {
        println!(
            "[{}] {} sources \u{2192} {} drafts \u{2192} {} uploaded (ids: {:?})",
            r.label,
            r.sources,
            r.drafts,
            r.uploaded.len(),
            r.uploaded
        );
    }
    if runs.len() > 1 {
        let total: usize = runs.iter().map(|r| r.uploaded.len()).sum();
        println!("Total uploaded across all runs: {total}");
    }
}
