use thiserror::Error;

#[derive(Error, Debug)]
pub enum LimiterError {
    #[error("Failed to acquire Redis connection: {0}")]
    ConnectionError(String),
    
    #[error("Failed to execute rate limit Lua script: {0}")]
    ScriptError(String),
    
    #[error("System time error")]
    TimeError,
}
