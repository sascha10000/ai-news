use crate::services::feed_fetcher;
use crate::AppState;
use tokio_cron_scheduler::{Job, JobScheduler};

pub async fn start_scheduler(state: AppState) -> Result<(), Box<dyn std::error::Error>> {
    let sched = JobScheduler::new().await?;

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

    #[cfg(feature = "server-llm")]
    {
        let gen_cron = format!("0 0 */{} * * *", state.config_gen_interval);
        let gen_state = state.clone();
        sched
            .add(Job::new_async(gen_cron.as_str(), move |_uuid, _lock| {
                let s = gen_state.clone();
                Box::pin(async move {
                    tracing::info!("Scheduled article generation starting...");
                    let unscoped = crate::server_llm::run_unscoped_generation(&s).await;
                    let per_list = crate::server_llm::run_all_lists_generation(&s).await;
                    let total = unscoped.as_ref().map(|v| v.len()).unwrap_or(0)
                        + per_list.as_ref().map(|v| v.len()).unwrap_or(0);
                    if let Err(e) = unscoped {
                        tracing::error!("Scheduled unscoped generation failed: {e}");
                    }
                    if let Err(e) = per_list {
                        tracing::error!("Scheduled per-list generation failed: {e}");
                    }
                    tracing::info!("Scheduled generation complete: {total} articles");
                })
            })?)
            .await?;
    }

    sched.start().await?;

    #[cfg(feature = "server-llm")]
    let gen_msg = format!(", generate every {} hr", state.config_gen_interval);
    #[cfg(not(feature = "server-llm"))]
    let gen_msg = "";

    tracing::info!(
        "Scheduler started: fetch every {} min{}",
        state.config_fetch_interval,
        gen_msg
    );
    Ok(())
}
