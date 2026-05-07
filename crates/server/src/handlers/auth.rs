use askama::Template;
use askama_web::WebTemplate;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Redirect;
use axum::Form;
use axum_extra::extract::cookie::{Cookie, CookieJar};
use serde::Deserialize;
use time::Duration;

use crate::error::AppError;
use crate::models::session::{Identity, Session};

use super::super::AppState;

#[derive(Template, WebTemplate)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    pub error: Option<String>,
}

#[derive(Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

pub async fn login_page(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<Redirect, LoginTemplate> {
    if let Some(cookie) = jar.get("session") {
        if let Ok(Some(identity)) = Session::validate(&state.db, cookie.value()).await {
            return Ok(Redirect::to(post_login_path(identity)));
        }
    }
    Err(LoginTemplate { error: None })
}

pub async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(input): Form<LoginForm>,
) -> Result<(CookieJar, Redirect), LoginTemplate> {
    let identity = if input.username == state.admin_username
        && constant_time_eq(input.password.as_bytes(), state.admin_password.as_bytes())
    {
        Some(Identity::Admin)
    } else {
        match crate::models::user::User::authenticate(&state.db, &input.username, &input.password)
            .await
        {
            Ok(Some(user)) => Some(Identity::User(user.id)),
            _ => None,
        }
    };

    let identity = identity.ok_or_else(|| LoginTemplate {
        error: Some("Invalid username or password".to_string()),
    })?;

    let token = Session::create(&state.db, identity)
        .await
        .map_err(|_| LoginTemplate {
            error: Some("Server error, please try again".to_string()),
        })?;

    let cookie = Cookie::build(("session", token))
        .path("/")
        .http_only(true)
        .max_age(Duration::hours(24))
        .same_site(axum_extra::extract::cookie::SameSite::Lax)
        .build();

    Ok((jar.add(cookie), Redirect::to(post_login_path(identity))))
}

fn post_login_path(identity: Identity) -> &'static str {
    match identity {
        Identity::Admin => "/admin",
        Identity::User(_) => "/user",
    }
}

pub async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<(CookieJar, Redirect), AppError> {
    if let Some(cookie) = jar.get("session") {
        let _ = Session::delete(&state.db, cookie.value()).await;
    }

    let cookie = Cookie::build(("session", ""))
        .path("/")
        .max_age(Duration::ZERO)
        .build();

    Ok((jar.remove(cookie), Redirect::to("/login")))
}

/// Resolve the session cookie to an `Identity`, or `Err` if missing/invalid.
async fn resolve_identity(
    parts: &mut axum::http::request::Parts,
    state: &AppState,
) -> Result<Identity, Redirect> {
    let jar = CookieJar::from_headers(&parts.headers);
    let token = jar
        .get("session")
        .map(|c| c.value().to_string())
        .ok_or(Redirect::to("/login"))?;
    Session::validate(&state.db, &token)
        .await
        .map_err(|_| Redirect::to("/login"))?
        .ok_or(Redirect::to("/login"))
}

/// Extractor: requires an admin session. User sessions are rejected.
pub struct RequireAdmin;

impl axum::extract::FromRequestParts<AppState> for RequireAdmin {
    type Rejection = Redirect;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        match resolve_identity(parts, state).await? {
            Identity::Admin => Ok(RequireAdmin),
            Identity::User(_) => Err(Redirect::to("/user")),
        }
    }
}

/// Extractor: requires a user session. Admin sessions are rejected.
pub struct RequireUser(pub i64);

impl axum::extract::FromRequestParts<AppState> for RequireUser {
    type Rejection = Redirect;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        match resolve_identity(parts, state).await? {
            Identity::User(uid) => Ok(RequireUser(uid)),
            Identity::Admin => Err(Redirect::to("/admin")),
        }
    }
}

/// Extractor: any logged-in identity.
#[allow(dead_code)]
pub struct AuthIdentity(pub Identity);

impl axum::extract::FromRequestParts<AppState> for AuthIdentity {
    type Rejection = Redirect;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(AuthIdentity(resolve_identity(parts, state).await?))
    }
}

/// Extractor: validates `Authorization: Bearer <token>` against the configured API_TOKEN.
/// Returns 503 if no token is configured (so an unconfigured server can't be probed silently).
pub struct RequireApiToken;

impl axum::extract::FromRequestParts<AppState> for RequireApiToken {
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let configured = state.api_token.as_deref().ok_or((
            StatusCode::SERVICE_UNAVAILABLE,
            "Remote API not enabled (set API_TOKEN)",
        ))?;

        let header = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or((StatusCode::UNAUTHORIZED, "Missing Authorization header"))?;

        let token = header
            .strip_prefix("Bearer ")
            .ok_or((StatusCode::UNAUTHORIZED, "Expected `Bearer <token>`"))?;

        if constant_time_eq(token.as_bytes(), configured.as_bytes()) {
            Ok(RequireApiToken)
        } else {
            Err((StatusCode::UNAUTHORIZED, "Invalid token"))
        }
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
