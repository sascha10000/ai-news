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
                    match crate::server_llm::run_local_generation(&s).await {
                        Ok(ids) => tracing::info!(
                            "Scheduled generation complete: {} articles",
                            ids.len()
                        ),
                        Err(e) => tracing::error!("Scheduled generation failed: {e}"),
                    }
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
