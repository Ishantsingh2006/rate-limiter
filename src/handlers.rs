use axum::{
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse},
    Json,
};
use crate::AppState;
use limiter_engine::strategy::{fixed, sliding};

pub async fn serve_root() -> impl IntoResponse {
    Html(include_str!("templates/index.html"))
}

pub async fn health_check() -> impl IntoResponse {
    let response = serde_json::json!({
        "status": "healthy",
        "message": "API Server is running normally."
    });
    
    (StatusCode::OK, Json(response))
}

pub async fn get_data() -> impl IntoResponse {
    let response = serde_json::json!({
        "success": true,
        "data": {
            "id": 101,
            "item": "Highly sensitive business data",
            "message": "Congratulations, you have not exceeded your rate limit!"
        }
    });
    let mut headers = HeaderMap::new();
    headers.insert("Cache-Control", HeaderValue::from_static("no-store, no-cache, must-revalidate"));
    (StatusCode::OK, headers, Json(response))
}

pub async fn get_limiter_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let client_id = match extract_client_id(&headers) {
        Some(id) => id,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "Missing or invalid Authorization header" })),
            )
                .into_response();
        }
    };

    let engine_result = match state.config.limiter_strategy.to_lowercase().as_str() {
        "fixed" => {
            fixed::peek(
                &state.limiter_client,
                &client_id,
                state.config.max_requests,
                state.config.window_size_ms,
            )
            .await
        }
        _ => {
            sliding::peek(
                &state.limiter_client,
                &client_id,
                state.config.max_requests,
                state.config.window_size_ms,
            )
            .await
        }
    };

    match engine_result {
        Ok(data) => {
            let response = serde_json::json!({
                "limit": data.limit,
                "remaining": data.remaining,
                "rest_in_sec": data.rest_in_sec,
            });
            let mut headers = HeaderMap::new();
            headers.insert("Cache-Control", HeaderValue::from_static("no-store, no-cache, must-revalidate"));
            (StatusCode::OK, headers, Json(response)).into_response()
        }
        Err(e) => {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Rate Limiter Error: {:?}", e) })),
            )
                .into_response()
        }
    }
}

fn extract_client_id(headers: &HeaderMap) -> Option<String> {
    let auth_header = headers.get("Authorization")?.to_str().ok()?;
    auth_header
        .strip_prefix("Bearer ")
        .map(|token| token.to_string())
}
