use axum::{http::Method, middleware as axum_middleware, routing::get, Router};
use tower_http::cors::{Any, CorsLayer};
use limiter_engine::LimiterClient;
mod config;
mod handlers;
mod middleware;
use config::AppConfig;
#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub limiter_client: LimiterClient,
}

#[tokio::main]
async fn main() {
    let config = AppConfig::load();
    println!("Starting API server on port {}...", config.server_port);
    // Verify Redis connection on startup. Since client pool initialization is lazy and 
    // won't fail if Redis is offline, we must actively verify connectivity using client.verify().
    // If verification fails, we fall back to the in-memory backend for local testing.
    let limiter_client = if config.limiter_backend.to_lowercase() == "memory" {
        println!("Using IN-MEMORY rate limiting.");
        LimiterClient::new_in_memory()
    } else {

        match LimiterClient::new(&config.redis_url) {
            Ok(client) => {
                match client.verify().await {
                    Ok(_) => {
                        println!("Rate Limiter initialized and verified with Redis at {}.", config.redis_url);
                        client
                    }
                    Err(e) => {
                        println!("WARNING: Redis is offline or unreachable: {}. Falling back to IN-MEMORY rate limiter for local development.", e);
                        LimiterClient::new_in_memory()
                    }
                }
            }
            Err(e) => {
                println!("WARNING: Failed to initialize Redis client pool: {}. Falling back to IN-MEMORY rate limiter for local development.", e);
                LimiterClient::new_in_memory()
            }
        }
    };
    let state = AppState {
        config: config.clone(),
        limiter_client,
    };
    let protected_routes = Router::new()
        .route("/data", get(handlers::get_data))
        .route_layer(axum_middleware::from_fn_with_state(
            state.clone(),
            middleware::rate_limit_middleware,
        ));
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any)
        .expose_headers([
            axum::http::HeaderName::from_static("ratelimit-limit"),
            axum::http::HeaderName::from_static("ratelimit-remaining"),
            axum::http::HeaderName::from_static("ratelimit-reset"),
        ]);

    let app = Router::new()
        .route("/", get(handlers::serve_root))
        .route("/health", get(handlers::health_check))
        .route("/api/limiter-status", get(handlers::get_limiter_status))
        .nest("/api", protected_routes)
        .layer(cors)
        .with_state(state);
    let bind_addr = format!("0.0.0.0:{}", config.server_port);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await.unwrap();    
    println!("Listening on http://{}", bind_addr);
    axum::serve(listener, app).await.unwrap();
}
