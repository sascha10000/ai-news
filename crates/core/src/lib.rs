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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListsResponse {
    pub lists: Vec<ListSummary>,
}
