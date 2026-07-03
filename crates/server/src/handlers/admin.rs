use askama::Template;
use askama_web::WebTemplate;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use axum_extra::extract::Form as ExtraForm;

use serde::Deserialize;

use crate::error::AppError;
use crate::models::app_settings::AppSettings;
use crate::models::feed::{CreateFeed, Feed};
use crate::models::generated_article::{ArticleWithOwner, GeneratedArticle};
use crate::models::list::{CreateList, List, ListWithOwner};
use crate::models::source_article::SourceArticle;
use crate::services::feed_import::{self, ImportError};

#[derive(Template, WebTemplate)]
#[template(path = "import_errors.html")]
pub struct ImportErrorsTemplate {
    pub errors: Vec<ImportError>,
    pub csv: String,
    pub lists: Vec<List>,
    pub form_action: String,
    pub back_url: String,
}

use super::auth::RequireAdmin;
use super::super::AppState;

const SERVER_LLM_ENABLED: bool = cfg!(feature = "server-llm");

#[derive(Template, WebTemplate)]
#[template(path = "admin/dashboard.html")]
pub struct DashboardTemplate {
    pub feeds: Vec<FeedWithLists>,
    pub lists: Vec<ListWithOwner>,
    pub assignable_lists: Vec<List>,
    pub drafts: Vec<ArticleWithOwner>,
    pub published: Vec<ArticleWithOwner>,
    pub categories: Vec<String>,
    pub server_llm_enabled: bool,
    pub auto_publish: bool,
    // Shared with templates/partials/desk/{drafts,published}_table.html.
    pub article_api_prefix: &'static str,
    pub bulk_publish_url: &'static str,
    pub bulk_unpublish_url: &'static str,
    pub show_owner: bool,
    pub admin_scope: bool,
}

pub struct FeedWithLists {
    pub feed: Feed,
    pub owner_username: Option<String>,
    pub article_count: i64,
    pub lists: Vec<List>,
}

pub async fn dashboard(
    _auth: RequireAdmin,
    State(state): State<AppState>,
) -> Result<DashboardTemplate, AppError> {
    let feeds = Feed::all_with_owner(&state.db).await?;
    let lists = List::all_with_owner(&state.db).await?;
    let assignable_lists = List::all_global(&state.db).await?;

    let mut feeds_with_lists = Vec::new();
    for fw in feeds {
        let count = SourceArticle::count_for_feed(&state.db, fw.feed.id).await?;
        let feed_lists = List::lists_for_feed(&state.db, fw.feed.id).await?;
        feeds_with_lists.push(FeedWithLists {
            feed: fw.feed,
            owner_username: fw.owner_username,
            article_count: count,
            lists: feed_lists,
        });
    }

    let drafts = GeneratedArticle::drafts_with_owner(&state.db).await?;
    let published = GeneratedArticle::all_published_with_owner(&state.db).await?;
    let categories = GeneratedArticle::all_categories(&state.db).await?;
    let auto_publish = AppSettings::auto_publish(&state.db).await?;

    Ok(DashboardTemplate {
        feeds: feeds_with_lists,
        lists,
        assignable_lists,
        drafts,
        published,
        categories,
        server_llm_enabled: SERVER_LLM_ENABLED,
        auto_publish,
        article_api_prefix: "/api/article",
        bulk_publish_url: "/api/articles/bulk-publish",
        bulk_unpublish_url: "/api/articles/bulk-unpublish",
        show_owner: true,
        admin_scope: true,
    })
}

pub async fn create_list(
    _auth: RequireAdmin,
    State(state): State<AppState>,
    Form(input): Form<CreateList>,
) -> Result<Redirect, AppError> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError::FeedParse("List name cannot be empty".to_string()));
    }
    let slug = slug::slugify(name);
    List::create(&state.db, name, &slug, None).await?;
    Ok(Redirect::to("/admin"))
}

pub async fn delete_list(
    _auth: RequireAdmin,
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
    _auth: RequireAdmin,
    State(state): State<AppState>,
    Path(feed_id): Path<i64>,
    Form(input): Form<AssignFeedToList>,
) -> Result<Redirect, AppError> {
    List::add_feed(&state.db, input.list_id, feed_id).await?;
    Ok(Redirect::to("/admin"))
}

#[derive(Deserialize)]
pub struct BulkAssignFeedsToList {
    pub list_id: i64,
    #[serde(default, rename = "feed_ids")]
    pub feed_ids: Vec<i64>,
}

pub async fn bulk_add_feeds_to_list(
    _auth: RequireAdmin,
    State(state): State<AppState>,
    ExtraForm(input): ExtraForm<BulkAssignFeedsToList>,
) -> Result<Redirect, AppError> {
    for feed_id in &input.feed_ids {
        List::add_feed(&state.db, input.list_id, *feed_id).await?;
    }
    Ok(Redirect::to("/admin"))
}

pub async fn remove_feed_from_list(
    _auth: RequireAdmin,
    State(state): State<AppState>,
    Path((feed_id, list_id)): Path<(i64, i64)>,
) -> Result<Redirect, AppError> {
    List::remove_feed(&state.db, list_id, feed_id).await?;
    Ok(Redirect::to("/admin"))
}

pub async fn create_feed(
    _auth: RequireAdmin,
    State(state): State<AppState>,
    Form(input): Form<CreateFeed>,
) -> Result<Redirect, AppError> {
    Feed::create(&state.db, &input.name, &input.url, None).await?;
    Ok(Redirect::to("/admin"))
}

#[derive(Deserialize)]
pub struct ImportFeeds {
    pub csv: String,
    #[serde(default)]
    pub list_ids: Vec<i64>,
}

pub async fn import_feeds(
    _auth: RequireAdmin,
    State(state): State<AppState>,
    ExtraForm(input): ExtraForm<ImportFeeds>,
) -> Result<Response, AppError> {
    match feed_import::parse_csv(&input.csv) {
        Err(errors) => {
            let lists = List::all_global(&state.db).await?;
            Ok(ImportErrorsTemplate {
                errors,
                csv: input.csv,
                lists,
                form_action: "/admin/feeds/import".to_string(),
                back_url: "/admin".to_string(),
            }
            .into_response())
        }
        Ok(parsed) => {
            for feed in &parsed {
                let feed_id = Feed::create(&state.db, &feed.name, &feed.url, None).await?;
                for list_id in &input.list_ids {
                    List::add_feed(&state.db, *list_id, feed_id).await?;
                }
            }
            Ok(Redirect::to("/admin").into_response())
        }
    }
}

pub async fn delete_feed(
    _auth: RequireAdmin,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Redirect, AppError> {
    Feed::delete(&state.db, id).await?;
    Ok(Redirect::to("/admin"))
}
