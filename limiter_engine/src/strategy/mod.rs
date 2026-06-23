pub mod fixed;
pub mod sliding;
#[derive(Debug, Clone, Copy)]
pub struct RateLimitResponse {
    pub allowed: bool,
    pub limit: u64,
    pub remaining: u64,
    pub rest_in_sec: u64,
}
