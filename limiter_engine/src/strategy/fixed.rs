use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    client::{LimiterClient, LimiterBackend},
    errors::LimiterError,
    strategy::RateLimitResponse,
};

// Atomically checks and increments a fixed window counter.
// Performs GET, checks threshold, INCR, and sets expiration on first increment.
pub const FIXED_WINDOW_SCRIPT: &str = r#"
local key = KEYS[1]
local max_requests = tonumber(ARGV[1])
local window_size = tonumber(ARGV[2])

local current_count = tonumber(redis.call("GET", key) or "0")

if current_count >= max_requests then
    return {0, tostring(current_count)}
end

current_count = redis.call("INCR", key)
if current_count == 1 then
    redis.call("PEXPIRE", key, window_size)
end

return {1, tostring(current_count)}
"#;

/// Executes the Fixed Window Counter rate limit check.
pub async fn check(
    client: &LimiterClient,
    client_id: &str,
    max_requests: u64,
    window_size: u64,
) -> Result<RateLimitResponse, LimiterError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| LimiterError::TimeError)?
        .as_millis() as u64;

    let curr = now / window_size;
    let elapsed = now % window_size;

    let key = format!("rate_limit_fixed:{}:{}", client_id, curr);

    match &client.backend {
        LimiterBackend::Redis(pool) => {
            let mut conn = pool
                .get()
                .await
                .map_err(|e| LimiterError::ConnectionError(e.to_string()))?;

            let script = redis::Script::new(FIXED_WINDOW_SCRIPT);

            let result: (i64, String) = script
                .key(key)
                .arg(max_requests)
                .arg(window_size)
                .invoke_async(&mut conn)
                .await
                .map_err(|e| LimiterError::ScriptError(e.to_string()))?;

            let (allowed_flag, current_count_str) = result;
            let current_count: u64 = current_count_str.parse().unwrap_or(max_requests);

            let remaining = if current_count >= max_requests {
                0
            } else {
                max_requests - current_count
            };

            // Uses ceiling division to ensure we don't return 0 when a fraction of a second is left
            let rest_in_sec = (window_size - elapsed + 999) / 1000;

            Ok(RateLimitResponse {
                allowed: allowed_flag == 1,
                limit: max_requests,
                remaining,
                rest_in_sec,
            })
        }
        LimiterBackend::Memory(store) => {
            let current_count = store.get(&key)
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);

            let allowed = current_count < max_requests;
            let final_count = if allowed {
                let next = store.incr(&key);
                if next == 1 {
                    store.pexpire(&key, window_size);
                }
                next
            } else {
                current_count
            };

            let remaining = if final_count >= max_requests {
                0
            } else {
                max_requests - final_count
            };

            let rest_in_sec = (window_size - elapsed + 999) / 1000;

            Ok(RateLimitResponse {
                allowed,
                limit: max_requests,
                remaining,
                rest_in_sec,
            })
        }
    }
}

/// Query the current rate limit status without incrementing.
pub async fn peek(
    client: &LimiterClient,
    client_id: &str,
    max_requests: u64,
    window_size: u64,
) -> Result<RateLimitResponse, LimiterError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| LimiterError::TimeError)?
        .as_millis() as u64;

    let curr = now / window_size;
    let elapsed = now % window_size;

    let key = format!("rate_limit_fixed:{}:{}", client_id, curr);

    match &client.backend {
        LimiterBackend::Redis(pool) => {
            let mut conn = pool
                .get()
                .await
                .map_err(|e| LimiterError::ConnectionError(e.to_string()))?;

            let current_count: u64 = redis::cmd("GET")
                .arg(&key)
                .query_async::<_, Option<u64>>(&mut conn)
                .await
                .map_err(|e| LimiterError::ScriptError(e.to_string()))?
                .unwrap_or(0);

            let remaining = if current_count >= max_requests {
                0
            } else {
                max_requests - current_count
            };

            let rest_in_sec = (window_size - elapsed + 999) / 1000;

            Ok(RateLimitResponse {
                allowed: current_count < max_requests,
                limit: max_requests,
                remaining,
                rest_in_sec,
            })
        }
        LimiterBackend::Memory(store) => {
            let current_count = store.get(&key)
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);

            let remaining = if current_count >= max_requests {
                0
            } else {
                max_requests - current_count
            };

            let rest_in_sec = (window_size - elapsed + 999) / 1000;

            Ok(RateLimitResponse {
                allowed: current_count < max_requests,
                limit: max_requests,
                remaining,
                rest_in_sec,
            })
        }
    }
}
