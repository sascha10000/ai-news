use crate::error::AppError;
use crate::models::feed::Feed;
use crate::models::source_article::SourceArticle;
use sqlx::SqlitePool;

pub async fn fetch_feed(pool: &SqlitePool, feed: &Feed) -> Result<usize, AppError> {
    let body = reqwest::get(&feed.url).await?.bytes().await?;
    let parsed = feed_rs::parser::parse(&body[..])
        .map_err(|e| AppError::FeedParse(format!("{}: {e}", feed.url)))?;

    let mut new_count = 0;

    for entry in parsed.entries {
        let title = entry
            .title
            .map(|t| t.content)
            .unwrap_or_else(|| "Untitled".to_string());

        let url = entry
            .links
            .first()
            .map(|l| l.href.clone())
            .unwrap_or_default();

        if url.is_empty() {
            continue;
        }

        let summary_text = entry.summary.as_ref().map(|s| s.content.clone());

        let content = entry
            .content
            .and_then(|c| c.body)
            .or_else(|| summary_text.clone())
            .unwrap_or_default();

        let guid = entry.id.as_str();
        let author = entry.authors.first().map(|a| a.name.as_str());
        let published_at = entry
            .published
            .or(entry.updated)
            .map(|d| d.to_rfc3339());

        let inserted = SourceArticle::insert(
            pool,
            feed.id,
            Some(guid),
            &title,
            &url,
            author,
            &content,
            summary_text.as_deref(),
            published_at.as_deref(),
        )
        .await?;

        if inserted.is_some() {
            new_count += 1;
        }
    }

    Feed::update_last_fetched(pool, feed.id).await?;
    Ok(new_count)
}

pub async fn fetch_all_feeds(pool: &SqlitePool) -> Result<usize, AppError> {
    let feeds = Feed::active(pool).await?;
    let mut total = 0;
    for feed in &feeds {
        match fetch_feed(pool, feed).await {
            Ok(count) => total += count,
            Err(e) => tracing::error!("Failed to fetch feed '{}': {e}", feed.name),
        }
    }
    Ok(total)
}

pub async fn fetch_all_feeds_for_user(
    pool: &SqlitePool,
    user_id: i64,
) -> Result<usize, AppError> {
    let feeds = Feed::all_for_user(pool, user_id).await?;
    let mut total = 0;
    for feed in &feeds {
        if !feed.active {
            continue;
        }
        match fetch_feed(pool, feed).await {
            Ok(count) => total += count,
            Err(e) => tracing::error!("Failed to fetch feed '{}': {e}", feed.name),
        }
    }
    Ok(total)
}
