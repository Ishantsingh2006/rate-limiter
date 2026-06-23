use crate::AppState;
use axum::{
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::IntoResponse,
};
use limiter_engine::strategy::{fixed, sliding};
pub async fn rate_limit_middleware(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> impl IntoResponse {
    let client_id = match extract_client_id(&headers) {
        Some(id) => id,
        Option::None => {
            return (
                StatusCode::UNAUTHORIZED,
                "Missing or invalid Authorization header. Expected format: 'Bearer <token>'\n",
            )
                .into_response()
        }
    };
    // Dispatch request checking to the configured rate limit strategy (fixed vs sliding window)
    let engine_result = match state.config.limiter_strategy.to_lowercase().as_str() {
        "fixed" => {
            fixed::check(
                &state.limiter_client,
                &client_id,
                state.config.max_requests,
                state.config.window_size_ms,
            )
            .await
        }
        _ => {
            sliding::check(
                &state.limiter_client,
                &client_id,
                state.config.max_requests,
                state.config.window_size_ms,
            )
            .await
        }
    };

    // Fail-Closed Strategy: We block requests with a 500 error if the rate limiter
    // database is unreachable, protecting the core app database from potential floods.

    let rate_limit_data = match engine_result {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Rate Limiter Engine Error: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Rate limiter offline. Traffic blocked to protect backend capacity.\n",
            )
                .into_response();
        }
    };
    let mut rate_limit_headers = HeaderMap::new();
    rate_limit_headers.insert("RateLimit-Limit", HeaderValue::from(rate_limit_data.limit));
    rate_limit_headers.insert(
        "RateLimit-Remaining",
        HeaderValue::from(rate_limit_data.remaining),
    );
    rate_limit_headers.insert(
        "RateLimit-Reset",
        HeaderValue::from(rate_limit_data.rest_in_sec),
    );
    rate_limit_headers.insert(
        "Cache-Control",
        HeaderValue::from_static("no-store, no-cache, must-revalidate"),
    );

    if !rate_limit_data.allowed {
        let mut response = (StatusCode::TOO_MANY_REQUESTS, "Quota Exceeded.\n").into_response();
        response.headers_mut().extend(rate_limit_headers);
        return response;
    }

    let mut response = next.run(request).await;
    response.headers_mut().extend(rate_limit_headers);
    response
}
fn extract_client_id(headers: &HeaderMap) -> Option<String> {
    let auth_header = headers.get("Authorization")?.to_str().ok()?;
    auth_header
        .strip_prefix("Bearer ")
        .map(|token| token.to_string())
}
