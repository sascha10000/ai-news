use askama::Template;
use askama_web::WebTemplate;
use axum::extract::{Path, Query, State};
use axum::response::Html;
use axum::Form;

use crate::error::AppError;
use crate::models::feed::Feed;
use crate::models::generated_article::GeneratedArticle;
use crate::services::feed_fetcher;

use super::auth::RequireAuth;
use super::super::AppState;
use super::public::Pagination;

#[derive(Template, WebTemplate)]
#[template(path = "partials/article_list.html")]
pub struct ArticleListPartial {
    pub articles: Vec<GeneratedArticle>,
    pub active_category: Option<String>,
    pub page: i64,
    pub has_more: bool,
}

pub async fn fetch_all_feeds(
    _auth: RequireAuth,
    State(state): State<AppState>,
) -> Result<Html<String>, AppError> {
    let count = feed_fetcher::fetch_all_feeds(&state.db).await?;
    Ok(Html(format!(
        r#"<div class="success">Fetched {count} new articles from all feeds</div>"#
    )))
}

pub async fn fetch_feed(
    _auth: RequireAuth,
    State(state): State<AppState>,
    Path(feed_id): Path<i64>,
) -> Result<Html<String>, AppError> {
    let feed = Feed::by_id(&state.db, feed_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let count = feed_fetcher::fetch_feed(&state.db, &feed).await?;
    Ok(Html(format!(
        r#"<span class="success">{count} new</span>"#
    )))
}

#[cfg(feature = "server-llm")]
pub async fn generate_articles(
    _auth: RequireAuth,
    State(state): State<AppState>,
) -> Result<Html<String>, AppError> {
    let ids = crate::server_llm::run_local_generation(&state).await?;

    if ids.is_empty() {
        Ok(Html(
            r#"<div class="info">No article clusters found. Need more source articles from different feeds covering the same topic.</div>"#
                .to_string(),
        ))
    } else {
        Ok(Html(format!(
            r#"<div class="success">Generated {} new article(s). Check drafts below.</div>"#,
            ids.len()
        )))
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

    let articles = GeneratedArticle::published(&state.db, per_page + 1, offset, category).await?;
    let has_more = articles.len() as i64 > per_page;
    let articles: Vec<_> = articles.into_iter().take(per_page as usize).collect();

    Ok(ArticleListPartial {
        articles,
        active_category: category.map(|s| s.to_string()),
        page,
        has_more,
    })
}

#[derive(serde::Deserialize)]
pub struct SetCategoryForm {
    pub category: String,
}

pub async fn set_category(
    _auth: RequireAuth,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(input): Form<SetCategoryForm>,
) -> Result<Html<String>, AppError> {
    GeneratedArticle::set_category(&state.db, id, &input.category).await?;
    Ok(Html(format!(
        r#"<span class="badge category">{}</span>"#,
        input.category
    )))
}

pub async fn publish_all_drafts(
    _auth: RequireAuth,
    State(state): State<AppState>,
) -> Result<Html<String>, AppError> {
    let drafts = GeneratedArticle::drafts(&state.db).await?;
    let count = drafts.len();
    for draft in &drafts {
        GeneratedArticle::set_status(&state.db, draft.id, "published").await?;
    }
    Ok(Html(format!(
        r#"<div class="success">Published {count} article(s). <a href="/admin">Refresh</a> to update the list.</div>"#
    )))
}

pub async fn publish_article(
    _auth: RequireAuth,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    GeneratedArticle::set_status(&state.db, id, "published").await?;
    Ok(Html(r#"<span class="badge published">Published</span>"#.to_string()))
}

pub async fn reject_article(
    _auth: RequireAuth,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    GeneratedArticle::set_status(&state.db, id, "rejected").await?;
    Ok(Html(r#"<span class="badge rejected">Rejected</span>"#.to_string()))
}
