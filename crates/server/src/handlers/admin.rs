use askama::Template;
use askama_web::WebTemplate;
use axum::extract::{Path, State};
use axum::response::Redirect;
use axum::Form;

use serde::Deserialize;

use crate::error::AppError;
use crate::models::feed::{CreateFeed, Feed};
use crate::models::generated_article::{GeneratedArticle, CATEGORIES};
use crate::models::list::{CreateList, List};
use crate::models::source_article::SourceArticle;

use super::auth::RequireAuth;
use super::super::AppState;

const SERVER_LLM_ENABLED: bool = cfg!(feature = "server-llm");

#[derive(Template, WebTemplate)]
#[template(path = "admin/dashboard.html")]
pub struct DashboardTemplate {
    pub feeds: Vec<FeedWithLists>,
    pub lists: Vec<List>,
    pub drafts: Vec<GeneratedArticle>,
    pub published: Vec<GeneratedArticle>,
    pub categories: &'static [&'static str],
    pub server_llm_enabled: bool,
}

pub struct FeedWithLists {
    pub feed: Feed,
    pub article_count: i64,
    pub lists: Vec<List>,
}

pub async fn dashboard(
    _auth: RequireAuth,
    State(state): State<AppState>,
) -> Result<DashboardTemplate, AppError> {
    let feeds = Feed::all(&state.db).await?;
    let lists = List::all(&state.db).await?;
    let mut feeds_with_lists = Vec::new();
    for feed in feeds {
        let count = SourceArticle::count_for_feed(&state.db, feed.id).await?;
        let feed_lists = List::lists_for_feed(&state.db, feed.id).await?;
        feeds_with_lists.push(FeedWithLists {
            feed,
            article_count: count,
            lists: feed_lists,
        });
    }

    let drafts = GeneratedArticle::drafts(&state.db).await?;
    let published = GeneratedArticle::all_published(&state.db).await?;

    Ok(DashboardTemplate {
        feeds: feeds_with_lists,
        lists,
        drafts,
        published,
        categories: CATEGORIES,
        server_llm_enabled: SERVER_LLM_ENABLED,
    })
}

pub async fn create_list(
    _auth: RequireAuth,
    State(state): State<AppState>,
    Form(input): Form<CreateList>,
) -> Result<Redirect, AppError> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError::FeedParse("List name cannot be empty".to_string()));
    }
    let slug = slug::slugify(name);
    List::create(&state.db, name, &slug).await?;
    Ok(Redirect::to("/admin"))
}

pub async fn delete_list(
    _auth: RequireAuth,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Redirect, AppError> {
    List::delete(&state.db, id).await?;
    Ok(Redirect::to("/admin"))
}

#[derive(Deserialize)]
pub struct AssignFeedToList {
    pub list_id: i64,
}

pub async fn add_feed_to_list(
    _auth: RequireAuth,
    State(state): State<AppState>,
    Path(feed_id): Path<i64>,
    Form(input): Form<AssignFeedToList>,
) -> Result<Redirect, AppError> {
    List::add_feed(&state.db, input.list_id, feed_id).await?;
    Ok(Redirect::to("/admin"))
}

pub async fn remove_feed_from_list(
    _auth: RequireAuth,
    State(state): State<AppState>,
    Path((feed_id, list_id)): Path<(i64, i64)>,
) -> Result<Redirect, AppError> {
    List::remove_feed(&state.db, list_id, feed_id).await?;
    Ok(Redirect::to("/admin"))
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
