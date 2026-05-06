use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;

use ai_news_core::{
    IngestArticlesRequest, IngestArticlesResponse, ListSummary, ListsResponse, PendingSource,
    PendingSourcesResponse,
};

use crate::error::AppError;
use crate::models::list::List;
use crate::models::source_article::SourceArticle;
use crate::services::ingest;

use super::auth::RequireApiToken;
use super::super::AppState;

const PENDING_HOURS: i64 = 48;

#[derive(Deserialize)]
pub struct PendingQuery {
    pub list_id: Option<i64>,
}

pub async fn pending_sources(
    _auth: RequireApiToken,
    State(state): State<AppState>,
    Query(params): Query<PendingQuery>,
) -> Result<Json<PendingSourcesResponse>, AppError> {
    let articles = SourceArticle::recent_uncited(
        &state.db,
        PENDING_HOURS,
        state.max_source_age_days as i64,
        params.list_id,
    )
    .await?;
    let sources: Vec<PendingSource> = articles.into_iter().map(to_dto).collect();
    Ok(Json(PendingSourcesResponse { sources }))
}

pub async fn lists(
    _auth: RequireApiToken,
    State(state): State<AppState>,
) -> Result<Json<ListsResponse>, AppError> {
    let rows = List::all(&state.db).await?;
    let lists = rows
        .into_iter()
        .map(|l| ListSummary {
            id: l.id,
            name: l.name,
            slug: l.slug,
        })
        .collect();
    Ok(Json(ListsResponse { lists }))
}

pub async fn ingest_articles(
    _auth: RequireApiToken,
    State(state): State<AppState>,
    Json(req): Json<IngestArticlesRequest>,
) -> Result<Json<IngestArticlesResponse>, AppError> {
    let created = ingest::ingest_articles(&state.db, req).await?;
    Ok(Json(IngestArticlesResponse { created }))
}

fn to_dto(a: SourceArticle) -> PendingSource {
    PendingSource {
        id: a.id,
        feed_id: a.feed_id,
        title: a.title,
        url: a.url,
        content: a.content,
        summary: a.summary,
        published_at: a.published_at,
    }
}
