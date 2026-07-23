//! MCP (Model Context Protocol) endpoint, Streamable HTTP transport.
//!
//! Deliberately hand-rolled and stateless: the spec allows a server to answer
//! every POST with a single `application/json` response and reject GET/DELETE
//! with 405 (no SSE stream, no sessions). Auth is a per-user API key (see
//! `RequireApiKey`); the key's owner also sees their own private user-space
//! content, but never drafts — every query is published-only.

use axum::extract::rejection::JsonRejection;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::models::generated_article::GeneratedArticle;
use crate::models::list::List;
use crate::models::user::User;

use super::auth::RequireApiKey;
use super::super::AppState;

const MAX_RESULTS: i64 = 200;
const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &["2024-11-05", "2025-03-26", "2025-06-18"];

pub async fn method_not_allowed() -> StatusCode {
    StatusCode::METHOD_NOT_ALLOWED
}

pub async fn handle(
    RequireApiKey(viewer): RequireApiKey,
    State(state): State<AppState>,
    body: Result<Json<Value>, JsonRejection>,
) -> Response {
    let Ok(Json(req)) = body else {
        return rpc_error(Value::Null, -32700, "Parse error");
    };
    if req.is_array() {
        // Batching was dropped from the spec in 2025-06-18; we never supported it.
        return rpc_error(Value::Null, -32600, "Batch requests are not supported");
    }

    let id = req.get("id").cloned();
    let Some(method) = req.get("method").and_then(Value::as_str) else {
        return rpc_error(id.unwrap_or(Value::Null), -32600, "Missing method");
    };

    if method.starts_with("notifications/") {
        return StatusCode::ACCEPTED.into_response();
    }

    let id = id.unwrap_or(Value::Null);
    let params = req.get("params").cloned().unwrap_or(Value::Null);

    match method {
        "initialize" => {
            let requested = params.get("protocolVersion").and_then(Value::as_str).unwrap_or("");
            let version = if SUPPORTED_PROTOCOL_VERSIONS.contains(&requested) {
                requested
            } else {
                "2025-06-18"
            };
            rpc_result(
                id,
                json!({
                    "protocolVersion": version,
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "ai-news", "version": env!("CARGO_PKG_VERSION")},
                }),
            )
        }
        "ping" => rpc_result(id, json!({})),
        "tools/list" => rpc_result(id, json!({"tools": tool_definitions()})),
        "tools/call" => tools_call(&state, viewer, id, &params).await,
        _ => rpc_error(id, -32601, "Method not found"),
    }
}

fn rpc_result(id: Value, result: Value) -> Response {
    Json(json!({"jsonrpc": "2.0", "id": id, "result": result})).into_response()
}

fn rpc_error(id: Value, code: i64, message: &str) -> Response {
    Json(json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}))
        .into_response()
}

fn tool_definitions() -> Value {
    json!([
        {
            "name": "get_articles",
            "description": "Get published news articles in a date range. end_date defaults to today.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "start_date": {"type": "string", "description": "Start date (inclusive), YYYY-MM-DD"},
                    "end_date": {"type": "string", "description": "End date (inclusive), YYYY-MM-DD. Defaults to today."}
                },
                "required": ["start_date"]
            }
        },
        {
            "name": "get_news_by_tag",
            "description": "Get published news articles for a tag (article category), matched case-insensitively.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tag": {"type": "string", "description": "Tag / category name"}
                },
                "required": ["tag"]
            }
        },
        {
            "name": "get_user_news",
            "description": "Get the published news of a user-space. Only allowed if that user-space is public or the API key belongs to that user.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "username": {"type": "string", "description": "Username of the user-space"}
                },
                "required": ["username"]
            }
        },
        {
            "name": "get_list_news",
            "description": "Get the published news articles of a list by its slug. Global lists take precedence; if no global list matches, the API key owner's own lists are searched.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "list": {"type": "string", "description": "List slug"}
                },
                "required": ["list"]
            }
        }
    ])
}

enum ToolFailure {
    InvalidParams(String),
    Db(sqlx::Error),
}

impl From<sqlx::Error> for ToolFailure {
    fn from(e: sqlx::Error) -> Self {
        ToolFailure::Db(e)
    }
}

async fn tools_call(state: &AppState, viewer: i64, id: Value, params: &Value) -> Response {
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return rpc_error(id, -32602, "Missing tool name");
    };
    let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));

    let outcome = match name {
        "get_articles" => get_articles(state, viewer, args).await,
        "get_news_by_tag" => get_news_by_tag(state, viewer, args).await,
        "get_user_news" => get_user_news(state, viewer, args).await,
        "get_list_news" => get_list_news(state, viewer, args).await,
        _ => return rpc_error(id, -32602, "Unknown tool"),
    };

    match outcome {
        Ok(result) => rpc_result(id, result),
        Err(ToolFailure::InvalidParams(msg)) => rpc_error(id, -32602, &msg),
        Err(ToolFailure::Db(e)) => {
            tracing::error!("MCP tool '{name}' query failed: {e}");
            rpc_error(id, -32603, "Internal error")
        }
    }
}

fn parse_args<T: serde::de::DeserializeOwned>(args: Value) -> Result<T, ToolFailure> {
    serde_json::from_value(args).map_err(|e| ToolFailure::InvalidParams(format!("Invalid arguments: {e}")))
}

fn parse_date(s: &str) -> Result<chrono::NaiveDate, ToolFailure> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| ToolFailure::InvalidParams(format!("Invalid date '{s}', expected YYYY-MM-DD")))
}

/// Domain outcomes like "unknown user" go into the tool result with
/// `isError: true` (not a JSON-RPC error) so the calling LLM sees them.
fn tool_error(message: &str) -> Value {
    json!({"content": [{"type": "text", "text": message}], "isError": true})
}

fn articles_result(articles: &[GeneratedArticle], base_url: Option<&str>) -> Value {
    let items: Vec<Value> = articles
        .iter()
        .map(|a| {
            let path = format!("/article/{}", a.slug);
            json!({
                "title": a.title,
                "slug": a.slug,
                "summary": a.summary,
                "category": a.category,
                "published_at": a.published_at,
                "list": a.list_name,
                "url": match base_url {
                    Some(base) => format!("{base}{path}"),
                    None => path,
                },
            })
        })
        .collect();
    let text = serde_json::to_string_pretty(&items).unwrap_or_else(|_| "[]".to_string());
    json!({"content": [{"type": "text", "text": text}], "isError": false})
}

#[derive(Deserialize)]
struct GetArticlesArgs {
    start_date: String,
    end_date: Option<String>,
}

async fn get_articles(state: &AppState, viewer: i64, args: Value) -> Result<Value, ToolFailure> {
    let args: GetArticlesArgs = parse_args(args)?;
    let start = parse_date(&args.start_date)?;
    let end = match &args.end_date {
        Some(s) => parse_date(s)?,
        None => chrono::Local::now().date_naive(),
    };
    if start > end {
        return Err(ToolFailure::InvalidParams(
            "start_date must not be after end_date".to_string(),
        ));
    }
    let articles = GeneratedArticle::published_in_range(
        &state.db,
        &start.to_string(),
        &end.to_string(),
        viewer,
        MAX_RESULTS,
    )
    .await?;
    Ok(articles_result(&articles, state.public_base_url.as_deref()))
}

#[derive(Deserialize)]
struct GetNewsByTagArgs {
    tag: String,
}

async fn get_news_by_tag(state: &AppState, viewer: i64, args: Value) -> Result<Value, ToolFailure> {
    let args: GetNewsByTagArgs = parse_args(args)?;
    let tag = args.tag.trim();
    if tag.is_empty() {
        return Err(ToolFailure::InvalidParams("tag must not be empty".to_string()));
    }
    let articles =
        GeneratedArticle::published_by_category(&state.db, tag, viewer, MAX_RESULTS).await?;
    Ok(articles_result(&articles, state.public_base_url.as_deref()))
}

#[derive(Deserialize)]
struct GetUserNewsArgs {
    username: String,
}

async fn get_user_news(state: &AppState, viewer: i64, args: Value) -> Result<Value, ToolFailure> {
    let args: GetUserNewsArgs = parse_args(args)?;
    let username = args.username.trim().trim_start_matches('@').to_lowercase();

    let Some(user) = User::by_username(&state.db, &username).await? else {
        return Ok(tool_error(&format!("No user-space named '{username}'")));
    };
    if !user.public && user.id != viewer {
        return Ok(tool_error(&format!("The user-space '{username}' is private")));
    }
    let articles =
        GeneratedArticle::published_for_user(&state.db, user.id, MAX_RESULTS, 0, None).await?;
    Ok(articles_result(&articles, state.public_base_url.as_deref()))
}

#[derive(Deserialize)]
struct GetListNewsArgs {
    list: String,
}

async fn get_list_news(state: &AppState, viewer: i64, args: Value) -> Result<Value, ToolFailure> {
    let args: GetListNewsArgs = parse_args(args)?;
    let slug = args.list.trim();
    if slug.is_empty() {
        return Err(ToolFailure::InvalidParams("list must not be empty".to_string()));
    }

    // Global lists win; fall back to the viewer's own lists (documented in
    // the tool description so clients know the precedence).
    let list = match List::by_slug_global(&state.db, slug).await? {
        Some(list) => list,
        None => match List::by_slug_for_user(&state.db, slug, viewer).await? {
            Some(list) => list,
            None => return Ok(tool_error(&format!("No list with slug '{slug}'"))),
        },
    };
    let articles =
        GeneratedArticle::published_for_list_id(&state.db, list.id, MAX_RESULTS).await?;
    Ok(articles_result(&articles, state.public_base_url.as_deref()))
}
