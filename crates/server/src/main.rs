mod config;
mod db;
mod error;
mod filters;
mod handlers;
mod models;
mod scheduler;
mod services;
#[cfg(feature = "server-llm")]
mod server_llm;

use axum::Router;
use axum::routing::{get, post};
use std::net::SocketAddr;
use tower_http::services::ServeDir;

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::SqlitePool,
    pub config_fetch_interval: u32,
    pub admin_username: String,
    pub admin_password: String,
    pub api_token: Option<String>,
    pub max_source_age_days: u32,
    #[cfg(feature = "server-llm")]
    pub ollama_cfg: ai_news_generation::OllamaConfig,
    #[cfg(feature = "server-llm")]
    pub config_gen_interval: u32,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    let cfg = config::Config::from_env();

    let pool = db::init_pool(&cfg.database_url)
        .await
        .expect("Failed to initialize database");

    if cfg.api_token.is_none() {
        tracing::warn!(
            "API_TOKEN is not set: /api/sources/pending and /api/articles/ingest will return 503"
        );
    }

    #[cfg(feature = "server-llm")]
    let ollama_cfg = server_llm::ollama_config_from(&cfg);

    #[cfg(feature = "server-llm")]
    if let Err(e) = ai_news_generation::check_model_available(&ollama_cfg).await {
        tracing::warn!("LLM startup check failed: {e}");
    }

    let state = AppState {
        db: pool,
        config_fetch_interval: cfg.fetch_interval_minutes,
        admin_username: cfg.admin_username.clone(),
        admin_password: cfg.admin_password.clone(),
        api_token: cfg.api_token.clone(),
        max_source_age_days: cfg.max_source_age_days,
        #[cfg(feature = "server-llm")]
        ollama_cfg,
        #[cfg(feature = "server-llm")]
        config_gen_interval: cfg.generate_interval_hours,
    };

    let sched_state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = scheduler::start_scheduler(sched_state).await {
            tracing::error!("Failed to start scheduler: {e}");
        }
    });

    let app = Router::new()
        // Public routes
        .route("/", get(handlers::public::index))
        .route("/article/{slug}", get(handlers::public::article))
        // Auth routes
        .route("/admin/login", get(handlers::auth::login_page))
        .route("/admin/login", post(handlers::auth::login))
        .route("/admin/logout", post(handlers::auth::logout))
        // Admin routes (protected)
        .route("/admin", get(handlers::admin::dashboard))
        .route("/admin/feeds", post(handlers::admin::create_feed))
        .route("/admin/feeds/import", post(handlers::admin::import_feeds))
        .route("/admin/feeds/{id}/delete", post(handlers::admin::delete_feed))
        .route("/admin/feeds/lists/bulk", post(handlers::admin::bulk_add_feeds_to_list))
        .route("/admin/feeds/{feed_id}/lists", post(handlers::admin::add_feed_to_list))
        .route(
            "/admin/feeds/{feed_id}/lists/{list_id}/delete",
            post(handlers::admin::remove_feed_from_list),
        )
        .route("/admin/lists", post(handlers::admin::create_list))
        .route("/admin/lists/{id}/delete", post(handlers::admin::delete_list))
        // Session-protected API
        .route("/api/fetch-all", post(handlers::api::fetch_all_feeds))
        .route("/api/fetch/{feed_id}", post(handlers::api::fetch_feed))
        .route("/api/articles", get(handlers::api::article_list))
        .route("/api/articles/bulk-publish", post(handlers::api::bulk_publish))
        .route("/api/articles/bulk-unpublish", post(handlers::api::bulk_unpublish))
        .route("/api/article/{id}/category", post(handlers::api::set_category))
        .route("/api/article/{id}/publish", post(handlers::api::publish_article))
        .route("/api/article/{id}/unpublish", post(handlers::api::unpublish_article))
        .route("/api/article/{id}/reject", post(handlers::api::reject_article))
        // Token-protected remote-control API
        .route("/api/sources/pending", get(handlers::remote::pending_sources))
        .route("/api/lists", get(handlers::remote::lists))
        .route("/api/articles/ingest", post(handlers::remote::ingest_articles));

    #[cfg(feature = "server-llm")]
    let app = app
        .route("/api/generate", post(handlers::api::generate_articles))
        .route(
            "/api/generate/list/{list_id}",
            post(handlers::api::generate_articles_for_list),
        )
        .route(
            "/api/generate/all-lists",
            post(handlers::api::generate_articles_all_lists),
        );

    let app = app
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state);

    let addr: SocketAddr = format!("{}:{}", cfg.server_host, cfg.server_port)
        .parse()
        .expect("Invalid server address");

    tracing::info!("Starting server on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
