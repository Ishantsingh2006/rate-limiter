use dotenvy::dotenv;
use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub redis_url: String,
    pub server_port: u16,
    pub max_requests: u64,
    pub window_size_ms: u64,
    pub limiter_strategy: String,
    pub limiter_backend: String,
}

impl AppConfig {
    pub fn load() -> Self {
        dotenv().ok();
        Self {
            redis_url: env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string()),
            server_port: env::var("PORT")
                .or_else(|_| env::var("SERVER_PORT"))
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .expect("PORT or SERVER_PORT must be a valid u16"),
            max_requests: env::var("LIMITER_MAX_REQUESTS")
                .unwrap_or_else(|_| "5".to_string())
                .parse()
                .expect("LIMITER_MAX_REQUESTS must be a valid u64"),
            window_size_ms: env::var("LIMITER_WINDOW_SIZE_MS")
                .unwrap_or_else(|_| "60000".to_string())
                .parse()
                .expect("LIMITER_WINDOW_SIZE_MS must be a valid u64"),
            limiter_strategy: env::var("LIMITER_STRATEGY")
                .unwrap_or_else(|_| "sliding".to_string()),
            limiter_backend: env::var("LIMITER_BACKEND")
                .unwrap_or_else(|_| "redis".to_string()),
        }
    }
}

