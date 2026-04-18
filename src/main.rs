mod config;
mod db;
mod error;
mod filters;
mod handlers;
mod models;
mod scheduler;
mod services;

use axum::Router;
use axum::routing::{get, post};
use ollama_rs::Ollama;
use std::net::SocketAddr;
use tower_http::services::ServeDir;

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::SqlitePool,
    pub ollama: Ollama,
    pub ollama_model: String,
    pub config_fetch_interval: u32,
    pub config_gen_interval: u32,
    pub admin_username: String,
    pub admin_password: String,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    let cfg = config::Config::from_env();

    let pool = db::init_pool(&cfg.database_url)
        .await
        .expect("Failed to initialize database");

    let ollama = Ollama::new(cfg.ollama_host.clone(), 11434);

    let state = AppState {
        db: pool,
        ollama,
        ollama_model: cfg.ollama_model.clone(),
        config_fetch_interval: cfg.fetch_interval_minutes,
        config_gen_interval: cfg.generate_interval_hours,
        admin_username: cfg.admin_username,
        admin_password: cfg.admin_password,
    };

    // Start background scheduler
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
        .route("/admin/feeds/{id}/delete", post(handlers::admin::delete_feed))
        // API routes (protected)
        .route("/api/fetch-all", post(handlers::api::fetch_all_feeds))
        .route("/api/fetch/{feed_id}", post(handlers::api::fetch_feed))
        .route("/api/generate", post(handlers::api::generate_articles))
        .route("/api/articles", get(handlers::api::article_list))
        .route("/api/articles/publish-all", post(handlers::api::publish_all_drafts))
        .route("/api/article/{id}/category", post(handlers::api::set_category))
        .route("/api/article/{id}/publish", post(handlers::api::publish_article))
        .route("/api/article/{id}/reject", post(handlers::api::reject_article))
        // Static files
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state);

    let addr: SocketAddr = format!("{}:{}", cfg.server_host, cfg.server_port)
        .parse()
        .expect("Invalid server address");

    tracing::info!("Starting server on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
