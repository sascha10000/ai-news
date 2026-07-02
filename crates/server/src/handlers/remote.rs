use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;

use ai_news_core::{
    IngestArticlesRequest, IngestArticlesResponse, ListSummary, ListsResponse, PendingSource,
    PendingSourcesResponse, UserSummary, UsersResponse,
};

use crate::error::AppError;
use crate::models::list::List;
use crate::models::source_article::SourceArticle;
use crate::models::user::User;
use crate::services::ingest;

use super::auth::RequireApiToken;
use super::super::AppState;

const PENDING_HOURS: i64 = 48;

#[derive(Deserialize)]
pub struct PendingQuery {
    pub list_id: Option<i64>,
    pub user_id: Option<i64>,
}

pub async fn pending_sources(
    _auth: RequireApiToken,
    State(state): State<AppState>,
    Query(params): Query<PendingQuery>,
) -> Result<Json<PendingSourcesResponse>, AppError> {
    let articles = if let Some(uid) = params.user_id {
        SourceArticle::recent_uncited_for_user(
            &state.db,
            PENDING_HOURS,
            state.max_source_age_days as i64,
            uid,
        )
        .await?
    } else {
        SourceArticle::recent_uncited(
            &state.db,
            PENDING_HOURS,
            state.max_source_age_days as i64,
            params.list_id,
        )
        .await?
    };
    let sources: Vec<PendingSource> = articles.into_iter().map(to_dto).collect();
    Ok(Json(PendingSourcesResponse { sources }))
}

pub async fn lists(
    _auth: RequireApiToken,
    State(state): State<AppState>,
) -> Result<Json<ListsResponse>, AppError> {
    let rows = List::all_with_owner(&state.db).await?;
    let lists = rows
        .into_iter()
        .map(|lw| ListSummary {
            id: lw.list.id,
            name: lw.list.name,
            slug: lw.list.slug,
            user_id: lw.list.user_id,
            username: lw.owner_username,
        })
        .collect();
    Ok(Json(ListsResponse { lists }))
}

pub async fn users(
    _auth: RequireApiToken,
    State(state): State<AppState>,
) -> Result<Json<UsersResponse>, AppError> {
    let rows = User::all_brief(&state.db).await?;
    let users = rows
        .into_iter()
        .map(|(id, username, language)| UserSummary { id, username, language })
        .collect();
    Ok(Json(UsersResponse { users }))
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
