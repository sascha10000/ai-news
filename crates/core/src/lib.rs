use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingSource {
    pub id: i64,
    pub feed_id: i64,
    pub title: String,
    pub url: String,
    pub content: String,
    pub summary: Option<String>,
    pub published_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingSourcesResponse {
    pub sources: Vec<PendingSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestSentence {
    pub position: i32,
    pub content: String,
    pub source_article_ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestArticleRequest {
    pub title: String,
    pub slug: String,
    pub summary: Option<String>,
    pub category: Option<String>,
    pub sentences: Vec<IngestSentence>,
    #[serde(default)]
    pub list_id: Option<i64>,
    #[serde(default)]
    pub user_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestArticlesRequest {
    pub articles: Vec<IngestArticleRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestArticlesResponse {
    pub created: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListSummary {
    pub id: i64,
    pub name: String,
    pub slug: String,
    #[serde(default)]
    pub user_id: Option<i64>,
    #[serde(default)]
    pub username: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListsResponse {
    pub lists: Vec<ListSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSummary {
    pub id: i64,
    pub username: String,
    /// Two-letter language code (e.g. "en", "de") for LLM output. None means
    /// "no preference" — the model picks whatever fits the sources.
    #[serde(default)]
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsersResponse {
    pub users: Vec<UserSummary>,
}

/// Two-letter language codes the app supports for LLM output, paired with
/// the human-readable name that gets injected into the prompt. Shared by the
/// server (form validation, UI dropdown) and the client (mapping user-summary
/// codes to prompt strings) so a new language only needs to be added once.
pub const SUPPORTED_LANGUAGES: &[(&str, &str)] = &[
    ("en", "English"),
    ("de", "German"),
    ("fr", "French"),
    ("es", "Spanish"),
    ("it", "Italian"),
    ("pt", "Portuguese"),
    ("nl", "Dutch"),
];

pub fn language_label(code: &str) -> Option<&'static str> {
    SUPPORTED_LANGUAGES
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, name)| *name)
}
