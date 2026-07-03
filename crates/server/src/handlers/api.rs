use askama::Template;
use askama_web::WebTemplate;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue};
use axum::response::Html;
use axum::Form;
use axum_extra::extract::Form as ExtraForm;

use crate::error::AppError;
use crate::models::app_settings::AppSettings;
use crate::models::feed::Feed;
use crate::models::generated_article::GeneratedArticle;
use crate::services::feed_fetcher;

use super::auth::RequireAdmin;
use super::super::AppState;
use super::public::Pagination;

#[derive(Template, WebTemplate)]
#[template(path = "partials/article_list.html")]
pub struct ArticleListPartial {
    pub articles: Vec<GeneratedArticle>,
    pub active_category: Option<String>,
    pub page: i64,
    pub has_more: bool,
    pub load_more_base: String,
}

pub async fn fetch_all_feeds(
    _auth: RequireAdmin,
    State(state): State<AppState>,
) -> Result<Html<String>, AppError> {
    let count = feed_fetcher::fetch_all_feeds(&state.db).await?;
    Ok(Html(format!(
        r#"<div class="success">Fetched {count} new articles from all feeds</div>"#
    )))
}

pub async fn fetch_feed(
    _auth: RequireAdmin,
    State(state): State<AppState>,
    Path(feed_id): Path<i64>,
) -> Result<Html<String>, AppError> {
    let feed = Feed::by_id(&state.db, feed_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if feed.user_id.is_some() {
        return Err(AppError::NotFound);
    }
    let count = feed_fetcher::fetch_feed(&state.db, &feed).await?;
    Ok(Html(format!(
        r#"<span class="success">{count} new</span>"#
    )))
}

#[cfg(feature = "server-llm")]
pub async fn generate_articles(
    _auth: RequireAdmin,
    State(state): State<AppState>,
) -> Result<Html<String>, AppError> {
    let ids = crate::server_llm::run_unscoped_generation(&state).await?;
    Ok(Html(generate_response_html(ids.len(), "unscoped")))
}

#[cfg(feature = "server-llm")]
pub async fn generate_articles_for_list(
    _auth: RequireAdmin,
    State(state): State<AppState>,
    Path(list_id): Path<i64>,
) -> Result<Html<String>, AppError> {
    let ids = crate::server_llm::run_list_generation(&state, list_id).await?;
    Ok(Html(generate_response_html(ids.len(), "list")))
}

#[cfg(feature = "server-llm")]
pub async fn generate_articles_all_lists(
    _auth: RequireAdmin,
    State(state): State<AppState>,
) -> Result<Html<String>, AppError> {
    let ids = crate::server_llm::run_all_lists_generation(&state).await?;
    Ok(Html(generate_response_html(ids.len(), "across all lists")))
}

#[cfg(feature = "server-llm")]
fn generate_response_html(count: usize, scope: &str) -> String {
    if count == 0 {
        format!(
            r#"<div class="info">No article clusters found ({scope}). Need more source articles from different feeds covering the same topic.</div>"#
        )
    } else {
        format!(
            r#"<div class="success">Generated {count} new article(s) ({scope}). Check drafts below.</div>"#
        )
    }
}

pub async fn article_list(
    State(state): State<AppState>,
    Query(params): Query<Pagination>,
) -> Result<ArticleListPartial, AppError> {
    let page = params.page.unwrap_or(1).max(1);
    let per_page = 12;
    let offset = (page - 1) * per_page;
    let category = params.category.as_deref().filter(|c| !c.is_empty());

    let articles =
        GeneratedArticle::published_global(&state.db, per_page + 1, offset, category).await?;
    let has_more = articles.len() as i64 > per_page;
    let articles: Vec<_> = articles.into_iter().take(per_page as usize).collect();

    Ok(ArticleListPartial {
        articles,
        active_category: category.map(|s| s.to_string()),
        page,
        has_more,
        load_more_base: "/api/articles".to_string(),
    })
}

pub async fn list_articles_page(
    State(state): State<AppState>,
    axum::extract::Path(slug): axum::extract::Path<String>,
    Query(params): Query<Pagination>,
) -> Result<ArticleListPartial, AppError> {
    let list = crate::models::list::List::by_slug_global(&state.db, &slug)
        .await?
        .ok_or(AppError::NotFound)?;

    let page = params.page.unwrap_or(1).max(1);
    let per_page = 12;
    let offset = (page - 1) * per_page;
    let category = params.category.as_deref().filter(|c| !c.is_empty());

    let articles = GeneratedArticle::published_for_list(
        &state.db,
        list.id,
        per_page + 1,
        offset,
        category,
    )
    .await?;
    let has_more = articles.len() as i64 > per_page;
    let articles: Vec<_> = articles.into_iter().take(per_page as usize).collect();

    Ok(ArticleListPartial {
        articles,
        active_category: category.map(|s| s.to_string()),
        page,
        has_more,
        load_more_base: format!("/api/list/{}/articles", list.slug),
    })
}

#[derive(serde::Deserialize)]
pub struct SetCategoryForm {
    pub category: String,
}

pub async fn set_category(
    _auth: RequireAdmin,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(input): Form<SetCategoryForm>,
) -> Result<Html<String>, AppError> {
    let normalized = input.category.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return Err(AppError::BadRequest("Category cannot be empty".to_string()));
    }
    let updated = GeneratedArticle::set_category_for_admin(&state.db, id, &normalized).await?;
    if !updated {
        return Err(AppError::NotFound);
    }
    Ok(Html(format!(
        r#"<span class="badge category">{}</span>"#,
        normalized
    )))
}

pub async fn publish_article(
    _auth: RequireAdmin,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    let updated = GeneratedArticle::set_status_for_admin(&state.db, id, "published").await?;
    if !updated {
        return Err(AppError::NotFound);
    }
    Ok(Html(r#"<span class="badge published">Published</span>"#.to_string()))
}

pub async fn reject_article(
    _auth: RequireAdmin,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    let updated = GeneratedArticle::set_status_for_admin(&state.db, id, "rejected").await?;
    if !updated {
        return Err(AppError::NotFound);
    }
    Ok(Html(r#"<span class="badge rejected">Rejected</span>"#.to_string()))
}

pub async fn delete_article(
    _auth: RequireAdmin,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    let deleted = GeneratedArticle::delete_for_admin(&state.db, id).await?;
    if !deleted {
        return Err(AppError::NotFound);
    }
    Ok(Html(String::new()))
}

pub async fn unpublish_article(
    _auth: RequireAdmin,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    let updated = GeneratedArticle::set_status_for_admin(&state.db, id, "draft").await?;
    if !updated {
        return Err(AppError::NotFound);
    }
    Ok(Html(r#"<span class="badge">Unpublished</span>"#.to_string()))
}

#[derive(serde::Deserialize)]
pub struct BulkArticleIds {
    #[serde(default)]
    pub ids: Vec<i64>,
}

pub async fn bulk_publish(
    _auth: RequireAdmin,
    State(state): State<AppState>,
    ExtraForm(input): ExtraForm<BulkArticleIds>,
) -> Result<(HeaderMap, Html<String>), AppError> {
    let n = GeneratedArticle::set_status_bulk_for_admin(&state.db, &input.ids, "published").await?;
    Ok(refresh_response(format!("Published {n} article(s).")))
}

pub async fn bulk_unpublish(
    _auth: RequireAdmin,
    State(state): State<AppState>,
    ExtraForm(input): ExtraForm<BulkArticleIds>,
) -> Result<(HeaderMap, Html<String>), AppError> {
    let n = GeneratedArticle::set_status_bulk_for_admin(&state.db, &input.ids, "draft").await?;
    Ok(refresh_response(format!("Unpublished {n} article(s).")))
}

fn refresh_response(msg: String) -> (HeaderMap, Html<String>) {
    let mut headers = HeaderMap::new();
    headers.insert("HX-Refresh", HeaderValue::from_static("true"));
    (headers, Html(format!(r#"<div class="success">{msg}</div>"#)))
}

#[derive(serde::Deserialize)]
pub struct SetAutoPublishForm {
    // HTML checkboxes only submit when checked, so `enabled` being absent
    // means "off" and any present value means "on".
    #[serde(default)]
    pub enabled: Option<String>,
}

pub async fn set_auto_publish(
    _auth: RequireAdmin,
    State(state): State<AppState>,
    Form(input): Form<SetAutoPublishForm>,
) -> Result<Html<String>, AppError> {
    let enabled = input.enabled.is_some();
    AppSettings::set_auto_publish(&state.db, enabled).await?;
    let msg = if enabled {
        "Auto-publish on — new global articles skip drafts."
    } else {
        "Auto-publish off — new articles land in drafts."
    };
    Ok(Html(format!(r#"<span class="hint">{msg}</span>"#)))
}
