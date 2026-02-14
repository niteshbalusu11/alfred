use std::env;

use thiserror::Error;

#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub bind_addr: String,
}

#[derive(Debug, Clone)]
pub struct WorkerConfig {
    pub tick_seconds: u64,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("invalid integer in env var {0}")]
    ParseInt(String),
}

impl ApiConfig {
    pub fn from_env() -> Self {
        Self {
            bind_addr: env::var("API_BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string()),
        }
    }
}

impl WorkerConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let tick_seconds = match env::var("WORKER_TICK_SECONDS") {
            Ok(raw) => raw
                .parse::<u64>()
                .map_err(|_| ConfigError::ParseInt("WORKER_TICK_SECONDS".to_string()))?,
            Err(_) => 30,
        };

        Ok(Self { tick_seconds })
    }
}
