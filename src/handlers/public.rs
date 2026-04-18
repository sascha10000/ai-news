use askama::Template;
use askama_web::WebTemplate;
use axum::extract::{Path, Query, State};
use serde::Deserialize;

use crate::error::AppError;
use crate::models::citation::{self, SentenceWithSources, SourceRef};
use crate::models::generated_article::GeneratedArticle;

use super::super::AppState;

#[derive(Template, WebTemplate)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub articles: Vec<GeneratedArticle>,
    pub categories: Vec<String>,
    pub active_category: Option<String>,
    pub page: i64,
    pub has_more: bool,
}

#[derive(Template, WebTemplate)]
#[template(path = "article.html")]
pub struct ArticleTemplate {
    pub article: GeneratedArticle,
    pub sentences: Vec<SentenceWithSources>,
    pub all_sources: Vec<SourceRef>,
}

#[derive(Deserialize)]
pub struct Pagination {
    pub page: Option<i64>,
    pub category: Option<String>,
}

pub async fn index(
    State(state): State<AppState>,
    Query(params): Query<Pagination>,
) -> Result<IndexTemplate, AppError> {
    let page = params.page.unwrap_or(1).max(1);
    let per_page = 12;
    let offset = (page - 1) * per_page;
    let category = params.category.as_deref().filter(|c| !c.is_empty());

    let articles = GeneratedArticle::published(&state.db, per_page + 1, offset, category).await?;
    let has_more = articles.len() as i64 > per_page;
    let articles: Vec<_> = articles.into_iter().take(per_page as usize).collect();
    let categories = GeneratedArticle::published_categories(&state.db).await?;

    Ok(IndexTemplate {
        articles,
        categories,
        active_category: category.map(|s| s.to_string()),
        page,
        has_more,
    })
}

pub async fn article(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<ArticleTemplate, AppError> {
    let article = GeneratedArticle::by_slug(&state.db, &slug)
        .await?
        .ok_or(AppError::NotFound)?;

    let sentences = citation::sentences_with_sources(&state.db, article.id).await?;
    let all_sources = citation::all_sources_for_article(&state.db, article.id).await?;

    Ok(ArticleTemplate {
        article,
        sentences,
        all_sources,
    })
}
