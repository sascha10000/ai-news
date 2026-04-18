use crate::AppState;
use crate::services::{article_generator, feed_fetcher};
use tokio_cron_scheduler::{Job, JobScheduler};

pub async fn start_scheduler(state: AppState) -> Result<(), Box<dyn std::error::Error>> {
    let sched = JobScheduler::new().await?;

    // Fetch feeds on schedule
    let fetch_cron = format!("0 */{} * * * *", state.config_fetch_interval);
    let fetch_state = state.clone();
    sched
        .add(Job::new_async(fetch_cron.as_str(), move |_uuid, _lock| {
            let s = fetch_state.clone();
            Box::pin(async move {
                tracing::info!("Scheduled feed fetch starting...");
                match feed_fetcher::fetch_all_feeds(&s.db).await {
                    Ok(count) => tracing::info!("Scheduled fetch complete: {count} new articles"),
                    Err(e) => tracing::error!("Scheduled fetch failed: {e}"),
                }
            })
        })?)
        .await?;

    // Generate articles on schedule
    let gen_cron = format!("0 0 */{} * * *", state.config_gen_interval);
    let gen_state = state.clone();
    sched
        .add(Job::new_async(gen_cron.as_str(), move |_uuid, _lock| {
            let s = gen_state.clone();
            Box::pin(async move {
                tracing::info!("Scheduled article generation starting...");
                match article_generator::generate_articles(&s.db, &s.ollama, &s.ollama_model).await
                {
                    Ok(ids) => {
                        tracing::info!("Scheduled generation complete: {} articles", ids.len())
                    }
                    Err(e) => tracing::error!("Scheduled generation failed: {e}"),
                }
            })
        })?)
        .await?;

    sched.start().await?;
    tracing::info!(
        "Scheduler started: fetch every {} min, generate every {} hr",
        state.config_fetch_interval,
        state.config_gen_interval
    );
    Ok(())
}
