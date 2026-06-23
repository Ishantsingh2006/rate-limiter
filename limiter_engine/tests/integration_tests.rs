use limiter_engine::LimiterClient;
use limiter_engine::client::LimiterBackend;
use limiter_engine::strategy::{sliding, fixed};

async fn get_redis_client() -> Option<LimiterClient> {
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let client = LimiterClient::new(&redis_url).ok()?;
    
    if let LimiterBackend::Redis(pool) = &client.backend {
        match pool.get().await {
            Ok(_) => Some(client),
            Err(_) => {
                println!("Skipping Redis integration test: Local Redis server is not running at {}", redis_url);
                None
            }
        }
    } else {
        None
    }
}

// ---------------- IN-MEMORY TESTS ----------------

#[tokio::test]
async fn test_in_memory_sliding_window() {
    let client = LimiterClient::new_in_memory();
    let client_id = "test_user_sliding_mem";
    let max_requests = 3;
    let window_size = 5000;
    // First request should be allowed
    let res1 = sliding::check(&client, client_id, max_requests, window_size).await.unwrap();
    assert!(res1.allowed);
    assert!(res1.remaining >= 1);
    // Second request
    let res2 = sliding::check(&client, client_id, max_requests, window_size).await.unwrap();
    assert!(res2.allowed);
    // Third request
    let res3 = sliding::check(&client, client_id, max_requests, window_size).await.unwrap();
    assert!(res3.allowed);
    // Fourth request should be blocked
    let res4 = sliding::check(&client, client_id, max_requests, window_size).await.unwrap();
    assert!(!res4.allowed);
    assert_eq!(res4.remaining, 0);
}

#[tokio::test]
async fn test_in_memory_fixed_window() {
    let client = LimiterClient::new_in_memory();
    let client_id = "test_user_fixed_mem";
    let max_requests = 2;
    let window_size = 5000;

    // First request should be allowed
    let res1 = fixed::check(&client, client_id, max_requests, window_size).await.unwrap();
    assert!(res1.allowed);
    assert_eq!(res1.remaining, 1);

    // Second request
    let res2 = fixed::check(&client, client_id, max_requests, window_size).await.unwrap();
    assert!(res2.allowed);
    assert_eq!(res2.remaining, 0);

    // Third request should be blocked
    let res3 = fixed::check(&client, client_id, max_requests, window_size).await.unwrap();
    assert!(!res3.allowed);
    assert_eq!(res3.remaining, 0);
}

// ---------------- REDIS TESTS ----------------

#[tokio::test]
async fn test_redis_sliding_window_rate_limiter() {
    let client = match get_redis_client().await {
        Some(c) => c,
        None => return,
    };

    let client_id = "test_user_sliding_redis";
    let max_requests = 3;
    let window_size = 5000;

    // Clear any existing keys to ensure fresh test run
    if let LimiterBackend::Redis(pool) = &client.backend {
        let mut conn = pool.get().await.unwrap();
        let current_window = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64) / window_size;
        let previous_window = current_window - 1;
        let curr_key = format!("rate_limit:{}:{}", client_id, current_window);
        let prev_key = format!("rate_limit:{}:{}", client_id, previous_window);
        let _: Result<(), redis::RedisError> = redis::cmd("DEL").arg(&curr_key).arg(&prev_key).query_async(&mut conn).await;
    }
    // First request should be allowed
    let res1 = sliding::check(&client, client_id, max_requests, window_size).await.unwrap();
    assert!(res1.allowed);
    assert!(res1.remaining >= 1);

    // Second request
    let res2 = sliding::check(&client, client_id, max_requests, window_size).await.unwrap();
    assert!(res2.allowed);

    // Third request
    let res3 = sliding::check(&client, client_id, max_requests, window_size).await.unwrap();
    assert!(res3.allowed);

    // Fourth request should be blocked
    let res4 = sliding::check(&client, client_id, max_requests, window_size).await.unwrap();
    assert!(!res4.allowed);
    assert_eq!(res4.remaining, 0);
}

#[tokio::test]
async fn test_redis_fixed_window_rate_limiter() {
    let client = match get_redis_client().await {
        Some(c) => c,
        None => return,
    };

    let client_id = "test_user_fixed_redis";
    let max_requests = 2;
    let window_size = 5000;

    // Clear any existing keys
    if let LimiterBackend::Redis(pool) = &client.backend {
        let mut conn = pool.get().await.unwrap();
        let current_window = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64) / window_size;
        let key = format!("rate_limit_fixed:{}:{}", client_id, current_window);
        let _: Result<(), redis::RedisError> = redis::cmd("DEL").arg(&key).query_async(&mut conn).await;
    }
    // First request should be allowed
    let res1 = fixed::check(&client, client_id, max_requests, window_size).await.unwrap();
    assert!(res1.allowed);
    assert_eq!(res1.remaining, 1);

    // Second request
    let res2 = fixed::check(&client, client_id, max_requests, window_size).await.unwrap();
    assert!(res2.allowed);
    assert_eq!(res2.remaining, 0);

    // Third request should be blocked
    let res3 = fixed::check(&client, client_id, max_requests, window_size).await.unwrap();
    assert!(!res3.allowed);
    assert_eq!(res3.remaining, 0);
}

