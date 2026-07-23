use askama::Template;
use askama_web::WebTemplate;
use axum::extract::{Multipart, Path, State};
use axum::http::{HeaderMap, HeaderValue};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::Form;
use axum_extra::extract::cookie::{Cookie, CookieJar};
use axum_extra::extract::Form as ExtraForm;
use serde::Deserialize;
use time::Duration;

use crate::error::AppError;
use crate::handlers::admin::{FeedWithLists, ImportErrorsTemplate};
use crate::models::api_key::ApiKey;
use crate::models::article_interaction::ArticleInteraction;
use crate::models::feed::{CreateFeed, Feed};
use crate::models::generated_article::{ArticleWithOwner, GeneratedArticle};
use crate::models::list::{CreateList, List};
use crate::models::session::{Identity, Session};
use crate::models::source_article::SourceArticle;
use crate::models::user::{language_label, User, UserError, SUPPORTED_LANGUAGES};
use crate::services::feed_opml::ImportError;
use crate::services::{feed_fetcher, feed_import, feed_opml};

use super::auth::RequireUser;
use super::super::AppState;

#[derive(Template, WebTemplate)]
#[template(path = "register.html")]
pub struct RegisterTemplate {
    pub error: Option<String>,
    pub username: Option<String>,
}

#[derive(Template, WebTemplate)]
#[template(path = "user/dashboard.html")]
pub struct UserDashboardTemplate {
    pub user: User,
    pub feeds: Vec<FeedWithLists>,
    pub lists: Vec<List>,
    pub drafts: Vec<ArticleWithOwner>,
    pub published: Vec<ArticleWithOwner>,
    pub liked: Vec<GeneratedArticle>,
    pub read_later: Vec<GeneratedArticle>,
    pub categories: Vec<String>,
    pub server_llm_enabled: bool,
    pub languages: &'static [(&'static str, &'static str)],
    // Shared with templates/partials/desk/{drafts,published}_table.html.
    pub article_api_prefix: &'static str,
    pub bulk_publish_url: &'static str,
    pub bulk_unpublish_url: &'static str,
    pub show_owner: bool,
    pub admin_scope: bool,
    pub api_key: Option<ApiKey>,
    pub new_key: Option<String>,
}

#[derive(Deserialize)]
pub struct RegisterForm {
    pub username: String,
    pub password: String,
    pub password_confirm: String,
}

pub async fn register_page(
    jar: CookieJar,
    State(state): State<AppState>,
) -> Result<Redirect, RegisterTemplate> {
    if let Some(cookie) = jar.get("session") {
        if let Ok(Some(identity)) = Session::validate(&state.db, cookie.value()).await {
            let target = match identity {
                Identity::Admin => "/admin",
                Identity::User(_) => "/user",
            };
            return Ok(Redirect::to(target));
        }
    }
    Err(RegisterTemplate { error: None, username: None })
}

pub async fn register(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(input): Form<RegisterForm>,
) -> Result<(CookieJar, Redirect), RegisterTemplate> {
    let username = input.username.trim().to_lowercase();

    if input.password != input.password_confirm {
        return Err(RegisterTemplate {
            error: Some("Passwords do not match".to_string()),
            username: Some(username),
        });
    }

    let id = match User::create(&state.db, &username, &input.password).await {
        Ok(id) => id,
        Err(UserError::UsernameTaken) => {
            return Err(RegisterTemplate {
                error: Some("That username is taken".to_string()),
                username: Some(username),
            });
        }
        Err(UserError::ReservedUsername(_)) => {
            return Err(RegisterTemplate {
                error: Some("That username is reserved".to_string()),
                username: None,
            });
        }
        Err(UserError::InvalidUsername) => {
            return Err(RegisterTemplate {
                error: Some(
                    "Username must be 3-32 chars, lowercase a-z, 0-9, dash, underscore"
                        .to_string(),
                ),
                username: None,
            });
        }
        Err(UserError::WeakPassword) => {
            return Err(RegisterTemplate {
                error: Some("Password must be at least 8 characters".to_string()),
                username: Some(username),
            });
        }
        Err(UserError::Db(e)) => {
            tracing::error!("User::create db error: {e}");
            return Err(RegisterTemplate {
                error: Some("Server error, please try again".to_string()),
                username: Some(username),
            });
        }
    };

    let token = Session::create(&state.db, Identity::User(id))
        .await
        .map_err(|_| RegisterTemplate {
            error: Some("Account created but session failed; please log in".to_string()),
            username: None,
        })?;

    let cookie = Cookie::build(("session", token))
        .path("/")
        .http_only(true)
        .max_age(Duration::hours(24))
        .same_site(axum_extra::extract::cookie::SameSite::Lax)
        .build();

    Ok((jar.add(cookie), Redirect::to("/user")))
}

pub async fn dashboard(
    RequireUser(uid): RequireUser,
    State(state): State<AppState>,
) -> Result<UserDashboardTemplate, AppError> {
    let user = User::by_id(&state.db, uid).await?.ok_or(AppError::NotFound)?;
    let feeds = Feed::all_for_user(&state.db, uid).await?;
    let lists = List::all_for_user(&state.db, uid).await?;

    let mut feeds_with_lists = Vec::new();
    for feed in feeds {
        let count = SourceArticle::count_for_feed(&state.db, feed.id).await?;
        let feed_lists = List::lists_for_feed(&state.db, feed.id).await?;
        feeds_with_lists.push(FeedWithLists {
            feed,
            owner_username: None,
            article_count: count,
            lists: feed_lists,
        });
    }

    // Wrap in ArticleWithOwner (owner_username: None) so the drafts/published
    // tables can share one partial with the admin dashboard.
    let drafts = GeneratedArticle::drafts_for_user(&state.db, uid)
        .await?
        .into_iter()
        .map(|article| ArticleWithOwner { article, owner_username: None })
        .collect();
    let published = GeneratedArticle::all_published_for_user(&state.db, uid)
        .await?
        .into_iter()
        .map(|article| ArticleWithOwner { article, owner_username: None })
        .collect();
    let liked = ArticleInteraction::liked_for_user(&state.db, uid).await?;
    let read_later = ArticleInteraction::read_later_for_user(&state.db, uid).await?;
    let categories = GeneratedArticle::all_categories(&state.db).await?;
    let api_key = ApiKey::find_for_user(&state.db, uid).await?;

    Ok(UserDashboardTemplate {
        user,
        feeds: feeds_with_lists,
        lists,
        drafts,
        published,
        liked,
        read_later,
        categories,
        server_llm_enabled: SERVER_LLM_ENABLED,
        languages: SUPPORTED_LANGUAGES,
        article_api_prefix: "/api/user/article",
        bulk_publish_url: "/api/user/articles/bulk-publish",
        bulk_unpublish_url: "/api/user/articles/bulk-unpublish",
        show_owner: false,
        admin_scope: false,
        api_key,
        new_key: None,
    })
}

#[derive(Template, WebTemplate)]
#[template(path = "partials/desk/api_key.html")]
pub struct ApiKeyBlockTemplate {
    pub api_key: Option<ApiKey>,
    pub new_key: Option<String>,
}

pub async fn generate_api_key(
    RequireUser(uid): RequireUser,
    State(state): State<AppState>,
) -> Result<ApiKeyBlockTemplate, AppError> {
    let new_key = ApiKey::generate(&state.db, uid).await?;
    let api_key = ApiKey::find_for_user(&state.db, uid).await?;
    Ok(ApiKeyBlockTemplate { api_key, new_key: Some(new_key) })
}

pub async fn revoke_api_key(
    RequireUser(uid): RequireUser,
    State(state): State<AppState>,
) -> Result<ApiKeyBlockTemplate, AppError> {
    ApiKey::delete_for_user(&state.db, uid).await?;
    Ok(ApiKeyBlockTemplate { api_key: None, new_key: None })
}

const SERVER_LLM_ENABLED: bool = cfg!(feature = "server-llm");

#[derive(Deserialize)]
pub struct TogglePublicForm {
    #[serde(default)]
    pub public: Option<String>,
}

pub async fn toggle_public(
    RequireUser(uid): RequireUser,
    State(state): State<AppState>,
    Form(input): Form<TogglePublicForm>,
) -> Result<Html<String>, AppError> {
    let public = input.public.is_some();
    User::set_public(&state.db, uid, public).await?;
    let msg = if public {
        "Saved &mdash; your news page is now public."
    } else {
        "Saved &mdash; your news page is now private."
    };
    Ok(Html(msg.to_string()))
}

#[derive(Deserialize)]
pub struct SetAutoPublishForm {
    // HTML checkboxes only submit when checked, so `enabled` being absent
    // means "off" and any present value means "on".
    #[serde(default)]
    pub enabled: Option<String>,
}

pub async fn set_auto_publish(
    RequireUser(uid): RequireUser,
    State(state): State<AppState>,
    Form(input): Form<SetAutoPublishForm>,
) -> Result<Html<String>, AppError> {
    let enabled = input.enabled.is_some();
    User::set_auto_publish(&state.db, uid, enabled).await?;
    let msg = if enabled {
        "On &mdash; new articles skip drafts and publish straight to your news page."
    } else {
        "Off &mdash; new articles land in your drafts for review."
    };
    Ok(Html(msg.to_string()))
}

#[derive(Deserialize)]
pub struct SetLanguageForm {
    #[serde(default)]
    pub language: String,
}

pub async fn set_language(
    RequireUser(uid): RequireUser,
    State(state): State<AppState>,
    Form(input): Form<SetLanguageForm>,
) -> Result<Html<String>, AppError> {
    let trimmed = input.language.trim();
    let value = if trimmed.is_empty() {
        None
    } else if language_label(trimmed).is_some() {
        Some(trimmed)
    } else {
        return Err(AppError::BadRequest(format!(
            "Unsupported language code '{trimmed}'"
        )));
    };
    User::set_language(&state.db, uid, value).await?;
    let msg = match value.and_then(language_label) {
        Some(label) => format!("Saved &mdash; summaries will target {label}."),
        None => "Saved &mdash; no preference; the LLM keeps the source language.".to_string(),
    };
    Ok(Html(msg))
}

// ---------- feeds ----------

pub async fn create_feed(
    RequireUser(uid): RequireUser,
    State(state): State<AppState>,
    Form(input): Form<CreateFeed>,
) -> Result<Redirect, AppError> {
    Feed::create(&state.db, &input.name, &input.url, Some(uid)).await?;
    Ok(Redirect::to("/user"))
}

pub async fn delete_feed(
    RequireUser(uid): RequireUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Redirect, AppError> {
    Feed::delete_for_user(&state.db, id, uid).await?;
    Ok(Redirect::to("/user"))
}

#[derive(Deserialize)]
pub struct ImportFeedsForm {
    pub csv: String,
    #[serde(default)]
    pub list_ids: Vec<i64>,
}

pub async fn import_feeds(
    RequireUser(uid): RequireUser,
    State(state): State<AppState>,
    ExtraForm(input): ExtraForm<ImportFeedsForm>,
) -> Result<Response, AppError> {
    match feed_import::parse_csv(&input.csv) {
        Err(errors) => {
            let lists = List::all_for_user(&state.db, uid).await?;
            Ok(ImportErrorsTemplate {
                errors,
                csv: input.csv,
                lists,
                form_action: "/user/feeds/import".to_string(),
                back_url: "/user".to_string(),
            }
            .into_response())
        }
        Ok(parsed) => {
            for feed in &parsed {
                let feed_id = Feed::create(&state.db, &feed.name, &feed.url, Some(uid)).await?;
                for list_id in &input.list_ids {
                    List::add_feed_for_user(&state.db, *list_id, feed_id, uid).await?;
                }
            }
            Ok(Redirect::to("/user").into_response())
        }
    }
}

#[derive(Template, WebTemplate)]
#[template(path = "opml_import_errors.html")]
pub struct OpmlImportErrorsTemplate {
    pub errors: Vec<ImportError>,
    pub lists: Vec<List>,
}

/// Cap uploaded OPML size at 2 MiB. Real exports from Feedly/Reeder/NewsBlur
/// are well under this; the cap is a memory-abuse guard.
const OPML_MAX_BYTES: usize = 2 * 1024 * 1024;

pub async fn import_opml(
    RequireUser(uid): RequireUser,
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Response, AppError> {
    let mut xml: Option<String> = None;
    let mut list_ids: Vec<i64> = Vec::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("upload failed: {e}")))?
    {
        match field.name().unwrap_or("") {
            "file" => {
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("could not read file: {e}")))?;
                if bytes.len() > OPML_MAX_BYTES {
                    return Err(AppError::BadRequest(format!(
                        "OPML file too large ({} bytes, max {})",
                        bytes.len(),
                        OPML_MAX_BYTES
                    )));
                }
                let text = std::str::from_utf8(&bytes)
                    .map_err(|_| AppError::BadRequest("OPML file is not valid UTF-8".to_string()))?
                    .to_string();
                xml = Some(text);
            }
            "list_ids" => {
                let s = field.text().await.unwrap_or_default();
                if let Ok(id) = s.parse::<i64>() {
                    list_ids.push(id);
                }
            }
            _ => {}
        }
    }

    let xml = xml.ok_or_else(|| AppError::BadRequest("no OPML file provided".to_string()))?;

    match feed_opml::parse_opml(&xml) {
        Err(errors) => {
            let lists = List::all_for_user(&state.db, uid).await?;
            Ok(OpmlImportErrorsTemplate { errors, lists }.into_response())
        }
        Ok(parsed) => {
            for feed in &parsed.feeds {
                let feed_id = Feed::create(&state.db, &feed.name, &feed.url, Some(uid)).await?;
                if let Some(cat) = &feed.category {
                    let list_id =
                        List::find_or_create_for_user(&state.db, cat, uid).await?;
                    List::add_feed_for_user(&state.db, list_id, feed_id, uid).await?;
                }
                for list_id in &list_ids {
                    List::add_feed_for_user(&state.db, *list_id, feed_id, uid).await?;
                }
            }
            Ok(Redirect::to("/user").into_response())
        }
    }
}

#[derive(Deserialize)]
pub struct AssignFeedToListForm {
    pub list_id: i64,
}

pub async fn add_feed_to_list(
    RequireUser(uid): RequireUser,
    State(state): State<AppState>,
    Path(feed_id): Path<i64>,
    Form(input): Form<AssignFeedToListForm>,
) -> Result<Redirect, AppError> {
    List::add_feed_for_user(&state.db, input.list_id, feed_id, uid).await?;
    Ok(Redirect::to("/user"))
}

pub async fn remove_feed_from_list(
    RequireUser(uid): RequireUser,
    State(state): State<AppState>,
    Path((feed_id, list_id)): Path<(i64, i64)>,
) -> Result<Redirect, AppError> {
    List::remove_feed_for_user(&state.db, list_id, feed_id, uid).await?;
    Ok(Redirect::to("/user"))
}

#[derive(Deserialize)]
pub struct BulkAssignFeedsForm {
    pub list_id: i64,
    #[serde(default, rename = "feed_ids")]
    pub feed_ids: Vec<i64>,
}

pub async fn bulk_add_feeds_to_list(
    RequireUser(uid): RequireUser,
    State(state): State<AppState>,
    ExtraForm(input): ExtraForm<BulkAssignFeedsForm>,
) -> Result<Redirect, AppError> {
    for feed_id in &input.feed_ids {
        List::add_feed_for_user(&state.db, input.list_id, *feed_id, uid).await?;
    }
    Ok(Redirect::to("/user"))
}

// ---------- lists ----------

pub async fn create_list(
    RequireUser(uid): RequireUser,
    State(state): State<AppState>,
    Form(input): Form<CreateList>,
) -> Result<Redirect, AppError> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError::FeedParse("List name cannot be empty".to_string()));
    }
    let slug = slug::slugify(name);
    List::create(&state.db, name, &slug, Some(uid)).await?;
    Ok(Redirect::to("/user"))
}

pub async fn delete_list(
    RequireUser(uid): RequireUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Redirect, AppError> {
    List::delete_for_user(&state.db, id, uid).await?;
    Ok(Redirect::to("/user"))
}

// ---------- feed fetching ----------

pub async fn fetch_feed(
    RequireUser(uid): RequireUser,
    State(state): State<AppState>,
    Path(feed_id): Path<i64>,
) -> Result<Html<String>, AppError> {
    let feed = Feed::by_id(&state.db, feed_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if feed.user_id != Some(uid) {
        return Err(AppError::NotFound);
    }
    let count = feed_fetcher::fetch_feed(&state.db, &feed).await?;
    Ok(Html(format!(r#"<span class="success">{count} new</span>"#)))
}

pub async fn fetch_all_feeds(
    RequireUser(uid): RequireUser,
    State(state): State<AppState>,
) -> Result<Html<String>, AppError> {
    let count = feed_fetcher::fetch_all_feeds_for_user(&state.db, uid).await?;
    Ok(Html(format!(
        r#"<div class="success">Fetched {count} new articles from your feeds</div>"#
    )))
}

// ---------- article actions ----------

#[derive(Deserialize)]
pub struct SetCategoryForm {
    pub category: String,
}

pub async fn set_category(
    RequireUser(uid): RequireUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(input): Form<SetCategoryForm>,
) -> Result<Html<String>, AppError> {
    let normalized = input.category.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return Err(AppError::BadRequest("Category cannot be empty".to_string()));
    }
    let updated = GeneratedArticle::set_category_for_user(&state.db, id, &normalized, uid).await?;
    if !updated {
        return Err(AppError::NotFound);
    }
    Ok(Html(format!(
        r#"<span class="badge category">{}</span>"#,
        normalized
    )))
}

pub async fn publish(
    RequireUser(uid): RequireUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    let updated = GeneratedArticle::set_status_for_user(&state.db, id, "published", uid).await?;
    if !updated {
        return Err(AppError::NotFound);
    }
    Ok(Html(r#"<span class="badge published">Published</span>"#.to_string()))
}

pub async fn unpublish(
    RequireUser(uid): RequireUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    let updated = GeneratedArticle::set_status_for_user(&state.db, id, "draft", uid).await?;
    if !updated {
        return Err(AppError::NotFound);
    }
    Ok(Html(r#"<span class="badge">Unpublished</span>"#.to_string()))
}

pub async fn reject(
    RequireUser(uid): RequireUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    let updated = GeneratedArticle::set_status_for_user(&state.db, id, "rejected", uid).await?;
    if !updated {
        return Err(AppError::NotFound);
    }
    Ok(Html(r#"<span class="badge rejected">Rejected</span>"#.to_string()))
}

pub async fn delete(
    RequireUser(uid): RequireUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    let deleted = GeneratedArticle::delete_for_user(&state.db, id, uid).await?;
    if !deleted {
        return Err(AppError::NotFound);
    }
    Ok(Html(String::new()))
}

#[derive(Deserialize)]
pub struct BulkArticleIds {
    #[serde(default)]
    pub ids: Vec<i64>,
}

pub async fn bulk_publish(
    RequireUser(uid): RequireUser,
    State(state): State<AppState>,
    ExtraForm(input): ExtraForm<BulkArticleIds>,
) -> Result<(HeaderMap, Html<String>), AppError> {
    let n = GeneratedArticle::set_status_bulk_for_user(&state.db, &input.ids, "published", uid)
        .await?;
    Ok(refresh_response(format!("Published {n} article(s).")))
}

pub async fn bulk_unpublish(
    RequireUser(uid): RequireUser,
    State(state): State<AppState>,
    ExtraForm(input): ExtraForm<BulkArticleIds>,
) -> Result<(HeaderMap, Html<String>), AppError> {
    let n = GeneratedArticle::set_status_bulk_for_user(&state.db, &input.ids, "draft", uid).await?;
    Ok(refresh_response(format!("Unpublished {n} article(s).")))
}

fn refresh_response(msg: String) -> (HeaderMap, Html<String>) {
    let mut headers = HeaderMap::new();
    headers.insert("HX-Refresh", HeaderValue::from_static("true"));
    (headers, Html(format!(r#"<div class="success">{msg}</div>"#)))
}

// ---------- likes & read-later ----------

pub async fn like_article(
    RequireUser(uid): RequireUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    ArticleInteraction::like(&state.db, uid, id).await?;
    Ok(Html(render_like_button(id, true)))
}

pub async fn unlike_article(
    RequireUser(uid): RequireUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    ArticleInteraction::unlike(&state.db, uid, id).await?;
    Ok(Html(render_like_button(id, false)))
}

pub async fn mark_read_later(
    RequireUser(uid): RequireUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    ArticleInteraction::mark_read_later(&state.db, uid, id).await?;
    Ok(Html(render_read_later_button(id, true)))
}

pub async fn unmark_read_later(
    RequireUser(uid): RequireUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    ArticleInteraction::unmark_read_later(&state.db, uid, id).await?;
    Ok(Html(render_read_later_button(id, false)))
}

pub fn render_like_button(article_id: i64, liked: bool) -> String {
    if liked {
        format!(
            r##"<button id="like-btn-{id}" hx-post="/api/user/article/{id}/unlike" hx-target="#like-btn-{id}" hx-swap="outerHTML" class="btn btn-sm btn-success" title="You liked this">&#9829; Liked</button>"##,
            id = article_id
        )
    } else {
        format!(
            r##"<button id="like-btn-{id}" hx-post="/api/user/article/{id}/like" hx-target="#like-btn-{id}" hx-swap="outerHTML" class="btn btn-sm" title="Like this article">&#9825; Like</button>"##,
            id = article_id
        )
    }
}

pub fn render_read_later_button(article_id: i64, saved: bool) -> String {
    if saved {
        format!(
            r##"<button id="rl-btn-{id}" hx-post="/api/user/article/{id}/unread-later" hx-target="#rl-btn-{id}" hx-swap="outerHTML" class="btn btn-sm btn-success" title="Saved for later">&#9733; Saved</button>"##,
            id = article_id
        )
    } else {
        format!(
            r##"<button id="rl-btn-{id}" hx-post="/api/user/article/{id}/read-later" hx-target="#rl-btn-{id}" hx-swap="outerHTML" class="btn btn-sm" title="Save to read later">&#9734; Read later</button>"##,
            id = article_id
        )
    }
}

// ---------- generation (server-llm gated) ----------

#[cfg(feature = "server-llm")]
pub async fn generate_for_list(
    RequireUser(uid): RequireUser,
    State(state): State<AppState>,
    Path(list_id): Path<i64>,
) -> Result<Html<String>, AppError> {
    if List::owner_of(&state.db, list_id).await? != Some(uid) {
        return Err(AppError::NotFound);
    }
    let ids = crate::server_llm::run_list_generation(&state, list_id).await?;
    Ok(Html(generate_response_html(ids.len(), "your list")))
}

#[cfg(feature = "server-llm")]
pub async fn generate_all_lists(
    RequireUser(uid): RequireUser,
    State(state): State<AppState>,
) -> Result<Html<String>, AppError> {
    let lists = List::all_for_user(&state.db, uid).await?;
    let mut all_ids = Vec::new();
    for list in &lists {
        match crate::server_llm::run_list_generation(&state, list.id).await {
            Ok(mut ids) => all_ids.append(&mut ids),
            Err(e) => tracing::error!("Generation for list '{}' failed: {e}", list.name),
        }
    }
    Ok(Html(generate_response_html(
        all_ids.len(),
        "across your lists",
    )))
}

#[cfg(feature = "server-llm")]
fn generate_response_html(count: usize, scope: &str) -> String {
    if count == 0 {
        format!(
            r#"<div class="info">No article clusters found ({scope}). Need more source articles from different feeds covering the same topic.</div>"#
        )
    } else {
        format!(
            r#"<div class="success">Generated {count} new article(s) ({scope}). Refresh to see drafts.</div>"#
        )
    }
}
