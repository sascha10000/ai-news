use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Db(#[from] sqlx::Error),

    #[error("Feed parse error: {0}")]
    FeedParse(String),

    #[error("LLM error: {0}")]
    Llm(String),

    #[error("HTTP request error: {0}")]
    Request(#[from] reqwest::Error),

    #[error("Not found")]
    NotFound,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "Not found".to_string()),
            AppError::Db(e) => {
                tracing::error!("Database error: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string())
            }
            AppError::FeedParse(msg) => {
                tracing::error!("Feed parse error: {msg}");
                (StatusCode::BAD_REQUEST, format!("Feed error: {msg}"))
            }
            AppError::Llm(msg) => {
                tracing::error!("LLM error: {msg}");
                (StatusCode::INTERNAL_SERVER_ERROR, format!("Generation error: {msg}"))
            }
            AppError::Request(e) => {
                tracing::error!("HTTP request error: {e}");
                (StatusCode::BAD_GATEWAY, "Failed to fetch external resource".to_string())
            }
        };

        (status, Html(format!(r#"<div class="error">{message}</div>"#))).into_response()
    }
}
