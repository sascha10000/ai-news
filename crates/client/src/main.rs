use std::env;
use std::time::Duration;

use ai_news_core::{
    language_label, IngestArticleRequest, IngestArticlesRequest, IngestArticlesResponse,
    ListSummary, ListsResponse, PendingSourcesResponse, UserSummary, UsersResponse,
};
use ai_news_generation::{check_model_available, generate_drafts_for_list, OllamaConfig};
use clap::{ArgGroup, Parser};

enum Mode {
    Unscoped,
    List(i64),
    AllLists,
    ShowLists,
    UserNews(Option<String>),
    All,
}

#[derive(Clone, Copy)]
enum Scope {
    Unscoped,
    List(i64),
    User(i64),
}

/// Generate articles from pending source feeds via Ollama.
///
/// With no flags: generate from pending sources in global feeds only.
///
/// Required env vars: SERVER_URL, API_TOKEN.
/// Optional: OLLAMA_HOST (default http://localhost:11434), OLLAMA_MODEL (default llama3.2:latest).
#[derive(Parser, Debug)]
#[command(name = "client", version, about, long_about = None)]
#[command(group(ArgGroup::new("mode").required(false).multiple(false)))]
struct Cli {
    /// Generate articles only for the given list ID.
    #[arg(long, value_name = "ID", group = "mode")]
    list: Option<i64>,

    /// Run a generation pass for every configured list.
    #[arg(long, group = "mode")]
    all_lists: bool,

    /// Print all configured lists with their IDs (no generation).
    #[arg(long, group = "mode")]
    show_lists: bool,

    /// For each user (or just USERNAME): sweep each of their lists, then run a
    /// per-user catch-all over all of their feeds. Omit USERNAME to process every user.
    #[arg(
        long = "generate-user-news",
        value_name = "USERNAME",
        num_args = 0..=1,
        group = "mode",
    )]
    generate_user_news: Option<Option<String>>,

    /// Run every generation pass sequentially: unscoped, then every list, then
    /// every user. Equivalent to running `client`, `client --all-lists` and
    /// `client --generate-user-news` back-to-back.
    #[arg(long, group = "mode")]
    all: bool,
}

impl Cli {
    fn into_mode(self) -> Mode {
        if self.show_lists {
            Mode::ShowLists
        } else if self.all {
            Mode::All
        } else if self.all_lists {
            Mode::AllLists
        } else if let Some(id) = self.list {
            Mode::List(id)
        } else if let Some(filter) = self.generate_user_news {
            Mode::UserNews(filter)
        } else {
            Mode::Unscoped
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    let mode = Cli::parse().into_mode();

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
        Mode::ShowLists => unreachable!(),
        Mode::Unscoped => {
            let summary = run_one(&http, &server_url, &api_token, &ollama_cfg, Scope::Unscoped, None, "unscoped").await?;
            print_summary(&[summary]);
        }
        Mode::List(list_id) => {
            // Look up the list's owner to pick up their language preference.
            let lists = fetch_lists(&http, &server_url, &api_token).await?;
            let users = fetch_users(&http, &server_url, &api_token).await?;
            let lang = lists
                .iter()
                .find(|l| l.id == list_id)
                .and_then(|l| list_language(l, &users));
            let summary = run_one(
                &http,
                &server_url,
                &api_token,
                &ollama_cfg,
                Scope::List(list_id),
                lang,
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
            let users = fetch_users(&http, &server_url, &api_token).await?;
            let summaries =
                sweep_all_lists(&http, &server_url, &api_token, &ollama_cfg, &lists, &users).await;
            print_summary(&summaries);
        }
        Mode::UserNews(filter) => {
            let users = fetch_users(&http, &server_url, &api_token).await?;
            let filter_ref = filter.as_deref();
            let targets = filter_users(&users, filter_ref);

            if targets.is_empty() {
                match &filter {
                    Some(name) => println!("No user found with username '{}'.", name),
                    None => println!("No users registered."),
                }
                return Ok(());
            }

            let lists = fetch_lists(&http, &server_url, &api_token).await?;
            let summaries = sweep_user_news(
                &http,
                &server_url,
                &api_token,
                &ollama_cfg,
                &targets,
                &lists,
            )
            .await;
            print_summary(&summaries);
        }
        Mode::All => {
            tracing::info!("=== Phase 1/3: unscoped (global feeds) ===");
            let mut all = Vec::new();
            match run_one(&http, &server_url, &api_token, &ollama_cfg, Scope::Unscoped, None, "unscoped").await {
                Ok(s) => all.push(s),
                Err(e) => tracing::error!("Unscoped pass failed: {e}"),
            }

            let lists = fetch_lists(&http, &server_url, &api_token).await?;
            let users = fetch_users(&http, &server_url, &api_token).await?;

            tracing::info!("=== Phase 2/3: all lists ({}) ===", lists.len());
            all.extend(
                sweep_all_lists(&http, &server_url, &api_token, &ollama_cfg, &lists, &users).await,
            );

            tracing::info!("=== Phase 3/3: user news ({} users) ===", users.len());
            all.extend(
                sweep_user_news(&http, &server_url, &api_token, &ollama_cfg, &users, &lists).await,
            );

            print_summary(&all);
        }
    }
    Ok(())
}

fn user_language(user: &UserSummary) -> Option<&'static str> {
    user.language.as_deref().and_then(language_label)
}

fn list_language(list: &ListSummary, users: &[UserSummary]) -> Option<&'static str> {
    let owner_id = list.user_id?;
    users.iter().find(|u| u.id == owner_id).and_then(user_language)
}

fn filter_users(users: &[UserSummary], filter: Option<&str>) -> Vec<UserSummary> {
    match filter {
        Some(name) => {
            let needle = name.to_ascii_lowercase();
            users
                .iter()
                .filter(|u| u.username.to_ascii_lowercase() == needle)
                .cloned()
                .collect()
        }
        None => users.to_vec(),
    }
}

async fn sweep_all_lists(
    http: &reqwest::Client,
    server_url: &str,
    api_token: &str,
    ollama_cfg: &OllamaConfig,
    lists: &[ListSummary],
    users: &[UserSummary],
) -> Vec<RunSummary> {
    if lists.is_empty() {
        return vec![];
    }
    tracing::info!("Sweeping {} list(s)", lists.len());
    let mut summaries = Vec::with_capacity(lists.len());
    for list in lists {
        let label = format!("list '{}'", list.name);
        match run_one(
            http,
            server_url,
            api_token,
            ollama_cfg,
            Scope::List(list.id),
            list_language(list, users),
            &label,
        )
        .await
        {
            Ok(s) => summaries.push(s),
            Err(e) => tracing::error!("List '{}' failed: {e}", list.name),
        }
    }
    summaries
}

async fn sweep_user_news(
    http: &reqwest::Client,
    server_url: &str,
    api_token: &str,
    ollama_cfg: &OllamaConfig,
    users: &[UserSummary],
    lists: &[ListSummary],
) -> Vec<RunSummary> {
    if users.is_empty() {
        return vec![];
    }
    tracing::info!("Sweeping {} user(s)", users.len());
    let mut summaries = Vec::new();
    for u in users {
        let lang = user_language(u);
        let user_lists: Vec<&ListSummary> =
            lists.iter().filter(|l| l.user_id == Some(u.id)).collect();

        for list in &user_lists {
            let label = format!("list '{}' ({})", list.name, u.username);
            match run_one(
                http,
                server_url,
                api_token,
                ollama_cfg,
                Scope::List(list.id),
                lang,
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
            http,
            server_url,
            api_token,
            ollama_cfg,
            Scope::User(u.id),
            lang,
            &label,
        )
        .await
        {
            Ok(s) => summaries.push(s),
            Err(e) => tracing::error!("User '{}' catch-all failed: {e}", u.username),
        }
    }
    summaries
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
    target_language: Option<&str>,
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
    let mut drafts: Vec<IngestArticleRequest> = generate_drafts_for_list(
        pending.sources,
        ollama_cfg,
        list_id_for_drafts,
        target_language,
    )
    .await?;
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
