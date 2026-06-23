use crate::{
    client::{LimiterBackend, LimiterClient},
    errors::LimiterError,
    scripts::SLIDING_WINDOW_SCRIPT,
    strategy::RateLimitResponse,
};
use std::time::{SystemTime, UNIX_EPOCH};
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
    let curr_key: String = format!("rate_limit:{}:{}", client_id, curr);
    let prev_suffix = if curr > 0 {
        (curr - 1).to_string()
    } else {
        "none".to_string()
    };
    let prev_key: String = format!("rate_limit:{}:{}", client_id, prev_suffix);
    match &client.backend {
        LimiterBackend::Redis(pool) => {
            let mut conn = pool
                .get()
                .await
                .map_err(|e| LimiterError::ConnectionError(e.to_string()))?;
            let script = redis::Script::new(SLIDING_WINDOW_SCRIPT);
            let result: (i64, String, u64, u64) = script
                .key(&curr_key)
                .key(&prev_key)
                .arg(max_requests)
                .arg(window_size)
                .arg(elapsed)
                .invoke_async(&mut conn)
                .await
                .map_err(|e| LimiterError::ScriptError(e.to_string()))?;

            let (allowed_flag, estimated_count_str, current_count, previous_count) = result;
            let estimated_count: f64 = estimated_count_str.parse().unwrap_or(max_requests as f64);
            let remaining = if estimated_count >= max_requests as f64 {
                0
            } else {
                (max_requests as f64 - estimated_count).floor() as u64
            };
            let allowed = allowed_flag == 1;

            let rest_in_sec = if !allowed {
                if current_count >= max_requests {
                    (window_size - elapsed + 999) / 1000
                } else if previous_count > 0 {
                    let target_elapsed = window_size as f64 * (1.0 - (max_requests as f64 - current_count as f64) / previous_count as f64);
                    if target_elapsed > elapsed as f64 {
                        ((target_elapsed - elapsed as f64) / 1000.0).ceil() as u64
                    } else {
                        1
                    }
                } else {
                    (window_size - elapsed + 999) / 1000
                }
            } else {
                (window_size - elapsed + 999) / 1000
            };

            Ok(RateLimitResponse {
                allowed,
                limit: max_requests,
                remaining,
                rest_in_sec,
            })
        }
        LimiterBackend::Memory(store) => {
            let previous_count = store
                .get(&prev_key)
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);
            let current_count = store
                .get(&curr_key)
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);

            // Calculate the fraction of the previous window that overlaps with our sliding window.
            let mut weight = (window_size as f64 - elapsed as f64) / window_size as f64;
            if weight < 0.0 {
                weight = 0.0;
            }

            // Estimate the number of requests made in the rolling sliding window.
            let estimated_count = current_count + (previous_count * weight);
            let allowed = estimated_count < max_requests as f64;

            let final_estimated_count = if allowed {
                let next_count = store.incr(&curr_key);
                if next_count == 1 {
                    store.pexpire(&curr_key, window_size * 2);
                }
                next_count as f64 + (previous_count * weight)
            } else {
                estimated_count
            };

            let remaining = if final_estimated_count >= max_requests as f64 {
                0
            } else {
                (max_requests as f64 - final_estimated_count).floor() as u64
            };

            let rest_in_sec = if !allowed {
                if current_count >= max_requests as f64 {
                    (window_size - elapsed + 999) / 1000
                } else if previous_count > 0.0 {
                    let target_elapsed = window_size as f64 * (1.0 - (max_requests as f64 - current_count) / previous_count);
                    if target_elapsed > elapsed as f64 {
                        ((target_elapsed - elapsed as f64) / 1000.0).ceil() as u64
                    } else {
                        1
                    }
                } else {
                    (window_size - elapsed + 999) / 1000
                }
            } else {
                (window_size - elapsed + 999) / 1000
            };

            Ok(RateLimitResponse {
                allowed,
                limit: max_requests,
                remaining,
                rest_in_sec,
            })
        }
    }
}

/// Query the current sliding window rate limit status without incrementing.
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
    let curr_key: String = format!("rate_limit:{}:{}", client_id, curr);
    let prev_suffix = if curr > 0 {
        (curr - 1).to_string()
    } else {
        "none".to_string()
    };
    let prev_key: String = format!("rate_limit:{}:{}", client_id, prev_suffix);

    match &client.backend {
        LimiterBackend::Redis(pool) => {
            let mut conn = pool
                .get()
                .await
                .map_err(|e| LimiterError::ConnectionError(e.to_string()))?;

            let current_count: f64 = redis::cmd("GET")
                .arg(&curr_key)
                .query_async::<_, Option<String>>(&mut conn)
                .await
                .map_err(|e| LimiterError::ScriptError(e.to_string()))?
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);

            let previous_count: f64 = redis::cmd("GET")
                .arg(&prev_key)
                .query_async::<_, Option<String>>(&mut conn)
                .await
                .map_err(|e| LimiterError::ScriptError(e.to_string()))?
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);

            let mut weight = (window_size as f64 - elapsed as f64) / window_size as f64;
            if weight < 0.0 {
                weight = 0.0;
            }

            let estimated_count = current_count + (previous_count * weight);
            let allowed = estimated_count < max_requests as f64;

            let remaining = if estimated_count >= max_requests as f64 {
                0
            } else {
                (max_requests as f64 - estimated_count).floor() as u64
            };

            let rest_in_sec = if !allowed {
                if current_count >= max_requests as f64 {
                    (window_size - elapsed + 999) / 1000
                } else if previous_count > 0.0 {
                    let target_elapsed = window_size as f64 * (1.0 - (max_requests as f64 - current_count) / previous_count);
                    if target_elapsed > elapsed as f64 {
                        ((target_elapsed - elapsed as f64) / 1000.0).ceil() as u64
                    } else {
                        1
                    }
                } else {
                    (window_size - elapsed + 999) / 1000
                }
            } else {
                (window_size - elapsed + 999) / 1000
            };

            Ok(RateLimitResponse {
                allowed,
                limit: max_requests,
                remaining,
                rest_in_sec,
            })
        }
        LimiterBackend::Memory(store) => {
            let previous_count = store
                .get(&prev_key)
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);
            let current_count = store
                .get(&curr_key)
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);

            let mut weight = (window_size as f64 - elapsed as f64) / window_size as f64;
            if weight < 0.0 {
                weight = 0.0;
            }

            let estimated_count = current_count + (previous_count * weight);
            let allowed = estimated_count < max_requests as f64;

            let remaining = if estimated_count >= max_requests as f64 {
                0
            } else {
                (max_requests as f64 - estimated_count).floor() as u64
            };

            let rest_in_sec = if !allowed {
                if current_count >= max_requests as f64 {
                    (window_size - elapsed + 999) / 1000
                } else if previous_count > 0.0 {
                    let target_elapsed = window_size as f64 * (1.0 - (max_requests as f64 - current_count) / previous_count);
                    if target_elapsed > elapsed as f64 {
                        ((target_elapsed - elapsed as f64) / 1000.0).ceil() as u64
                    } else {
                        1
                    }
                } else {
                    (window_size - elapsed + 999) / 1000
                }
            } else {
                (window_size - elapsed + 999) / 1000
            };

            Ok(RateLimitResponse {
                allowed,
                limit: max_requests,
                remaining,
                rest_in_sec,
            })
        }
    }
}
