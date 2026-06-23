use deadpool_redis::{Config, Pool, Runtime};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::time::{Duration, Instant};
#[derive(Debug, Clone)]
pub struct KeyValue {
    pub value: String,
    pub expires_at: Option<Instant>,
}
// A thread-safe, in-memory key-value store with self-expiring keys.
// Emulates Redis operations (GET, INCR, PEXPIRE) required by the rate limiter strategies,
// enabling full local offline development and testing.
#[derive(Clone, Debug)]
pub struct MemoryStore {

    store: Arc<Mutex<HashMap<String, KeyValue>>>,
}
impl MemoryStore {
    pub fn new() -> Self {
        Self {
            store: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    pub fn get(&self, key: &str) -> Option<String> {
        let mut store = self.store.lock().unwrap();
        if let Some(kv) = store.get(key) {
            if let Some(expiry) = kv.expires_at {
                if Instant::now() > expiry {
                    store.remove(key);
                    return None;
                }
            }
            return Some(kv.value.clone());
        }
        None
    }
    pub fn incr(&self, key: &str) -> u64 {
        let mut store = self.store.lock().unwrap();
        let now = Instant::now();
        if let Some(kv) = store.get(key) {
            if let Some(expiry) = kv.expires_at {
                if now > expiry {
                    store.remove(key);
                }
            }
        }
        let entry = store.entry(key.to_string()).or_insert(KeyValue {
            value: "0".to_string(),
            expires_at: None,
        });
        let current: u64 = entry.value.parse().unwrap_or(0);
        let next = current + 1;
        entry.value = next.to_string();
        next
    }
    pub fn pexpire(&self, key: &str, ms: u64) {
        let mut store = self.store.lock().unwrap();
        if let Some(entry) = store.get_mut(key) {
            entry.expires_at = Some(Instant::now() + Duration::from_millis(ms));
        }
    }
}
#[derive(Clone)]
pub enum LimiterBackend {
    Redis(Pool),
    Memory(MemoryStore),
}
#[derive(Clone)]
pub struct LimiterClient {
    pub backend: LimiterBackend,
}
impl LimiterClient {
    pub fn new(redis_url: &str) -> Result<Self, String> {
        let cfg = Config::from_url(redis_url);
        let pool = cfg
            .create_pool(Some(Runtime::Tokio1))
            .map_err(|e| e.to_string())?;
        Ok(Self {
            backend: LimiterBackend::Redis(pool),
        })
    }
    pub fn new_in_memory() -> Self {
        Self {
            backend: LimiterBackend::Memory(MemoryStore::new()),
        }
    }
    pub async fn verify(&self) -> Result<(), String> {
        match &self.backend {
            LimiterBackend::Redis(pool) => {
                let _conn = pool.get().await.map_err(|e| e.to_string())?;
                Ok(())
            }
            LimiterBackend::Memory(_) => Ok(()),
        }
    }
}


