use std::env;

pub struct Config {
    pub database_url: String,
    pub fetch_interval_minutes: u32,
    pub server_host: String,
    pub server_port: u16,
    pub admin_username: String,
    pub admin_password: String,
    pub api_token: Option<String>,
    pub max_source_age_days: u32,
    #[cfg(feature = "server-llm")]
    pub ollama_host: String,
    #[cfg(feature = "server-llm")]
    pub ollama_model: String,
    #[cfg(feature = "server-llm")]
    pub generate_interval_hours: u32,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite:data.db?mode=rwc".to_string()),
            fetch_interval_minutes: env::var("FETCH_INTERVAL_MINUTES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
            server_host: env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            server_port: env::var("SERVER_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3000),
            admin_username: env::var("ADMIN_USERNAME").unwrap_or_else(|_| "admin".to_string()),
            admin_password: env::var("ADMIN_PASSWORD").expect("ADMIN_PASSWORD must be set in .env"),
            api_token: env::var("API_TOKEN").ok().filter(|s| !s.is_empty()),
            max_source_age_days: env::var("MAX_SOURCE_AGE_DAYS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
            #[cfg(feature = "server-llm")]
            ollama_host: env::var("OLLAMA_HOST")
                .unwrap_or_else(|_| "http://localhost:11434".to_string()),
            #[cfg(feature = "server-llm")]
            ollama_model: env::var("OLLAMA_MODEL")
                .unwrap_or_else(|_| "llama3.2:latest".to_string()),
            #[cfg(feature = "server-llm")]
            generate_interval_hours: env::var("GENERATE_INTERVAL_HOURS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2),
        }
    }
}
