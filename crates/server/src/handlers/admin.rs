use askama::Template;
use askama_web::WebTemplate;
use axum::extract::{Path, State};
use axum::response::Redirect;
use axum::Form;

use serde::Deserialize;

use crate::error::AppError;
use crate::models::feed::{CreateFeed, Feed};
use crate::models::generated_article::{GeneratedArticle, CATEGORIES};
use crate::models::source_article::SourceArticle;

use super::auth::RequireAuth;
use super::super::AppState;

const SERVER_LLM_ENABLED: bool = cfg!(feature = "server-llm");

#[derive(Template, WebTemplate)]
#[template(path = "admin/dashboard.html")]
pub struct DashboardTemplate {
    pub feeds: Vec<FeedWithCount>,
    pub drafts: Vec<GeneratedArticle>,
    pub categories: &'static [&'static str],
    pub server_llm_enabled: bool,
}

pub struct FeedWithCount {
    pub feed: Feed,
    pub article_count: i64,
}

pub async fn dashboard(
    _auth: RequireAuth,
    State(state): State<AppState>,
) -> Result<DashboardTemplate, AppError> {
    let feeds = Feed::all(&state.db).await?;
    let mut feeds_with_count = Vec::new();
    for feed in feeds {
        let count = SourceArticle::count_for_feed(&state.db, feed.id).await?;
        feeds_with_count.push(FeedWithCount {
            feed,
            article_count: count,
        });
    }

    let drafts = GeneratedArticle::drafts(&state.db).await?;

    Ok(DashboardTemplate {
        feeds: feeds_with_count,
        drafts,
        categories: CATEGORIES,
        server_llm_enabled: SERVER_LLM_ENABLED,
    })
}

pub async fn create_feed(
    _auth: RequireAuth,
    State(state): State<AppState>,
    Form(input): Form<CreateFeed>,
) -> Result<Redirect, AppError> {
    Feed::create(&state.db, &input.name, &input.url).await?;
    Ok(Redirect::to("/admin"))
}

#[derive(Deserialize)]
pub struct ImportFeeds {
    pub csv: String,
}

pub async fn import_feeds(
    _auth: RequireAuth,
    State(state): State<AppState>,
    Form(input): Form<ImportFeeds>,
) -> Result<Redirect, AppError> {
    for (idx, raw) in input.csv.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let (name, url) = line.split_once(';').ok_or_else(|| {
            AppError::FeedParse(format!(
                "Line {}: expected 'name;url', got: {line}",
                idx + 1
            ))
        })?;
        let name = name.trim();
        let url = url.trim();
        if name.is_empty() || url.is_empty() {
            return Err(AppError::FeedParse(format!(
                "Line {}: name and url must be non-empty",
                idx + 1
            )));
        }
        Feed::create(&state.db, name, url).await?;
    }
    Ok(Redirect::to("/admin"))
}

pub async fn delete_feed(
    _auth: RequireAuth,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Redirect, AppError> {
    Feed::delete(&state.db, id).await?;
    Ok(Redirect::to("/admin"))
}
