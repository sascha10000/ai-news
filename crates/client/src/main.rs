use std::env;
use std::time::Duration;

use ai_news_core::{
    IngestArticleRequest, IngestArticlesRequest, IngestArticlesResponse, ListSummary,
    ListsResponse, PendingSourcesResponse, UserSummary, UsersResponse,
};
use ai_news_generation::{check_model_available, generate_drafts_for_list, OllamaConfig};

enum Mode {
    Unscoped,
    List(i64),
    AllLists,
    ShowLists,
    UserNews(Option<String>),
    Help,
}

#[derive(Clone, Copy)]
enum Scope {
    Unscoped,
    List(i64),
    User(i64),
}

fn parse_mode() -> anyhow::Result<Mode> {
    let args: Vec<String> = env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        return Ok(Mode::Help);
    }
    if args.iter().any(|a| a == "--show-lists") {
        return Ok(Mode::ShowLists);
    }
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
    if let Some(pos) = args.iter().position(|a| a == "--generate-user-news") {
        let username = args
            .get(pos + 1)
            .filter(|s| !s.starts_with("--"))
            .map(|s| s.clone());
        return Ok(Mode::UserNews(username));
    }
    Ok(Mode::Unscoped)
}

fn print_help() {
    println!(
        "ai-news-client — generate articles from pending source feeds via Ollama.

USAGE:
    client [MODE]

MODES:
    (no flags)                       Generate articles from pending sources in global feeds only.
    --list <ID>                      Generate articles only for the given list ID.
    --all-lists                      Run a generation pass for every configured list.
    --show-lists                     Print all configured lists with their IDs (no generation).
    --generate-user-news [USERNAME]  For each user (or just USERNAME): sweep each of their lists,
                                     then run a per-user catch-all over all of their feeds.
    --help, -h                       Show this help.

ENVIRONMENT:
    SERVER_URL          Required. Base URL of the ai-news server (e.g. http://localhost:3000).
    API_TOKEN           Required. Token for /api/* token-protected routes.
    OLLAMA_HOST         Optional. Default: http://localhost:11434
    OLLAMA_MODEL        Optional. Default: llama3.2:latest

EXAMPLES:
    client --show-lists
    client --list 3
    client --all-lists
    client --generate-user-news
    client --generate-user-news alice"
    );
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    let mode = parse_mode()?;

    if let Mode::Help = mode {
        print_help();
        return Ok(());
    }

    let server_url = env::var("SERVER_URL")
        .map_err(|_| anyhow::anyhow!("SERVER_URL must be set (e.g. http://localhost:3000)"))?;
    let server_url = server_url.trim_end_matches('/').to_string();
    let api_token =
        env::var("API_TOKEN").map_err(|_| anyhow::anyhow!("API_TOKEN must be set"))?;

    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()?;

    if let Mode::ShowLists = mode {
        let lists = fetch_lists(&http, &server_url, &api_token).await?;
        print_lists(&lists);
        return Ok(());
    }

    let ollama_cfg = OllamaConfig {
        host: env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".to_string()),
        model: env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama3.2:latest".to_string()),
    };

    check_model_available(&ollama_cfg).await?;

    match mode {
        Mode::Help | Mode::ShowLists => unreachable!(),
        Mode::Unscoped => {
            let summary = run_one(&http, &server_url, &api_token, &ollama_cfg, Scope::Unscoped, "unscoped").await?;
            print_summary(&[summary]);
        }
        Mode::List(list_id) => {
            let summary = run_one(
                &http,
                &server_url,
                &api_token,
                &ollama_cfg,
                Scope::List(list_id),
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
                    Scope::List(list.id),
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
        Mode::UserNews(filter) => {
            let users = fetch_users(&http, &server_url, &api_token).await?;
            let targets: Vec<UserSummary> = match &filter {
                Some(name) => {
                    let needle = name.to_ascii_lowercase();
                    users
                        .into_iter()
                        .filter(|u| u.username.to_ascii_lowercase() == needle)
                        .collect()
                }
                None => users,
            };

            if targets.is_empty() {
                match &filter {
                    Some(name) => println!("No user found with username '{}'.", name),
                    None => println!("No users registered."),
                }
                return Ok(());
            }

            let lists = fetch_lists(&http, &server_url, &api_token).await?;
            tracing::info!("Sweeping {} user(s)", targets.len());
            let mut summaries = Vec::new();
            for u in &targets {
                let user_lists: Vec<&ListSummary> =
                    lists.iter().filter(|l| l.user_id == Some(u.id)).collect();

                for list in &user_lists {
                    let label = format!("list '{}' ({})", list.name, u.username);
                    match run_one(
                        &http,
                        &server_url,
                        &api_token,
                        &ollama_cfg,
                        Scope::List(list.id),
                        &label,
                    )
                    .await
                    {
                        Ok(s) => summaries.push(s),
                        Err(e) => tracing::error!("List '{}' for '{}' failed: {e}", list.name, u.username),
                    }
                }

                let label = format!("user '{}'", u.username);
                match run_one(
                    &http,
                    &server_url,
                    &api_token,
                    &ollama_cfg,
                    Scope::User(u.id),
                    &label,
                )
                .await
                {
                    Ok(s) => summaries.push(s),
                    Err(e) => tracing::error!("User '{}' catch-all failed: {e}", u.username),
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
    scope: Scope,
    label: &str,
) -> anyhow::Result<RunSummary> {
    tracing::info!("[{label}] fetching pending sources...");
    let mut req = http
        .get(format!("{server_url}/api/sources/pending"))
        .bearer_auth(api_token);
    match scope {
        Scope::List(id) => req = req.query(&[("list_id", id)]),
        Scope::User(id) => req = req.query(&[("user_id", id)]),
        Scope::Unscoped => {}
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

    let list_id_for_drafts = match scope {
        Scope::List(id) => Some(id),
        _ => None,
    };
    let mut drafts: Vec<IngestArticleRequest> =
        generate_drafts_for_list(pending.sources, ollama_cfg, list_id_for_drafts).await?;
    if let Scope::User(uid) = scope {
        for d in drafts.iter_mut() {
            d.user_id = Some(uid);
        }
    }
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

async fn fetch_users(
    http: &reqwest::Client,
    server_url: &str,
    api_token: &str,
) -> anyhow::Result<Vec<UserSummary>> {
    let body: UsersResponse = http
        .get(format!("{server_url}/api/users"))
        .bearer_auth(api_token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(body.users)
}

fn print_lists(lists: &[ListSummary]) {
    if lists.is_empty() {
        println!("No lists configured. Create lists in the admin UI first.");
        return;
    }
    let id_w = lists.iter().map(|l| l.id.to_string().len()).max().unwrap_or(2).max(2);
    let name_w = lists.iter().map(|l| l.name.len()).max().unwrap_or(4).max(4);
    println!("{:>w_id$}  {:<w_name$}  {}", "ID", "NAME", "SLUG", w_id = id_w, w_name = name_w);
    for l in lists {
        println!("{:>w_id$}  {:<w_name$}  {}", l.id, l.name, l.slug, w_id = id_w, w_name = name_w);
    }
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
