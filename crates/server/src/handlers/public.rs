use askama::Template;
use askama_web::WebTemplate;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;

use crate::error::AppError;
use crate::models::article_interaction::ArticleInteraction;
use crate::models::citation::{self, SentenceWithSources, SourceRef};
use crate::models::generated_article::GeneratedArticle;
use crate::models::list::List;
use crate::models::session::{Identity, Session};
use crate::models::user::{is_valid_username, User};

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
    pub like_count: i64,
    pub viewer_user_id: Option<i64>,
    pub viewer_liked: bool,
    pub viewer_read_later: bool,
}

#[derive(Template, WebTemplate)]
#[template(path = "private_page.html")]
pub struct PrivatePageTemplate {
    pub username: String,
}

#[derive(Template, WebTemplate)]
#[template(path = "why_account.html")]
pub struct WhyAccountTemplate;

#[derive(Template, WebTemplate)]
#[template(path = "impressum.html")]
pub struct ImpressumTemplate;

#[derive(Template, WebTemplate)]
#[template(path = "datenschutz.html")]
pub struct DatenschutzTemplate;

#[derive(Template, WebTemplate)]
#[template(path = "user_news.html")]
pub struct UserNewsTemplate {
    pub username: String,
    pub articles: Vec<GeneratedArticle>,
    pub categories: Vec<String>,
    pub active_category: Option<String>,
    pub page: i64,
    pub has_more: bool,
}

#[derive(Template, WebTemplate)]
#[template(path = "list.html")]
pub struct ListTemplate {
    pub list_name: String,
    pub list_slug: String,
    pub articles: Vec<GeneratedArticle>,
    pub categories: Vec<String>,
    pub active_category: Option<String>,
    pub page: i64,
    pub has_more: bool,
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

    let articles =
        GeneratedArticle::published_global(&state.db, per_page + 1, offset, category).await?;
    let has_more = articles.len() as i64 > per_page;
    let articles: Vec<_> = articles.into_iter().take(per_page as usize).collect();
    let categories = GeneratedArticle::published_categories_global(&state.db).await?;

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
    jar: CookieJar,
    Path(slug): Path<String>,
) -> Result<ArticleTemplate, AppError> {
    let article = GeneratedArticle::by_slug(&state.db, &slug)
        .await?
        .ok_or(AppError::NotFound)?;

    if article.status != "published" {
        return Err(AppError::NotFound);
    }

    // Owner-private articles are still link-shareable per spec, regardless of
    // the user's `public` flag. The flag only controls the listing page.

    let sentences = citation::sentences_with_sources(&state.db, article.id).await?;
    let all_sources = citation::all_sources_for_article(&state.db, article.id).await?;
    let like_count = ArticleInteraction::like_count(&state.db, article.id).await?;

    let viewer_user_id = match jar.get("session") {
        Some(cookie) => match Session::validate(&state.db, cookie.value()).await? {
            Some(Identity::User(uid)) => Some(uid),
            _ => None,
        },
        None => None,
    };

    let (viewer_liked, viewer_read_later) = match viewer_user_id {
        Some(uid) => (
            ArticleInteraction::is_liked(&state.db, uid, article.id).await?,
            ArticleInteraction::is_read_later(&state.db, uid, article.id).await?,
        ),
        None => (false, false),
    };

    Ok(ArticleTemplate {
        article,
        sentences,
        all_sources,
        like_count,
        viewer_user_id,
        viewer_liked,
        viewer_read_later,
    })
}

pub async fn user_news(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(username_with_at): Path<String>,
    Query(params): Query<Pagination>,
) -> Result<Response, AppError> {
    let username = username_with_at
        .strip_prefix('@')
        .unwrap_or(&username_with_at)
        .to_lowercase();
    if !is_valid_username(&username) {
        return Err(AppError::NotFound);
    }
    let user = User::by_username(&state.db, &username)
        .await?
        .ok_or(AppError::NotFound)?;

    if !user.public {
        let identity = match jar.get("session") {
            Some(cookie) => Session::validate(&state.db, cookie.value()).await?,
            None => None,
        };
        let allowed = match identity {
            Some(Identity::Admin) => true,
            Some(Identity::User(id)) => id == user.id,
            None => false,
        };
        if !allowed {
            // Reveals that the username exists, but the listing stays hidden.
            // Friendlier than a bare 404 for owners who simply aren't logged in.
            let page = PrivatePageTemplate {
                username: user.username,
            };
            return Ok((StatusCode::FORBIDDEN, page).into_response());
        }
    }

    let page = params.page.unwrap_or(1).max(1);
    let per_page = 12;
    let offset = (page - 1) * per_page;
    let category = params.category.as_deref().filter(|c| !c.is_empty());

    let articles =
        GeneratedArticle::published_for_user(&state.db, user.id, per_page + 1, offset, category)
            .await?;
    let has_more = articles.len() as i64 > per_page;
    let articles: Vec<_> = articles.into_iter().take(per_page as usize).collect();
    let categories =
        GeneratedArticle::published_categories_for_user(&state.db, user.id).await?;

    Ok(UserNewsTemplate {
        username: user.username,
        articles,
        categories,
        active_category: category.map(|s| s.to_string()),
        page,
        has_more,
    }
    .into_response())
}

pub async fn list_view(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Query(params): Query<Pagination>,
) -> Result<ListTemplate, AppError> {
    let list = List::by_slug_global(&state.db, &slug)
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
    let categories =
        GeneratedArticle::published_categories_for_list(&state.db, list.id).await?;

    Ok(ListTemplate {
        list_name: list.name,
        list_slug: list.slug,
        articles,
        categories,
        active_category: category.map(|s| s.to_string()),
        page,
        has_more,
    })
}

pub async fn why_account() -> WhyAccountTemplate {
    WhyAccountTemplate
}

pub async fn impressum() -> ImpressumTemplate {
    ImpressumTemplate
}

pub async fn datenschutz() -> DatenschutzTemplate {
    DatenschutzTemplate
}
