use std::env;

use thiserror::Error;

#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub bind_addr: String,
    pub database_url: String,
    pub database_max_connections: u32,
}

#[derive(Debug, Clone)]
pub struct WorkerConfig {
    pub tick_seconds: u64,
    pub database_url: String,
    pub database_max_connections: u32,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("invalid integer in env var {0}")]
    ParseInt(String),
}

impl ApiConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            bind_addr: env::var("API_BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string()),
            database_url: env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://postgres:postgres@127.0.0.1:5432/alfred".to_string()
            }),
            database_max_connections: parse_u32_env("DATABASE_MAX_CONNECTIONS", 10)?,
        })
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

        Ok(Self {
            tick_seconds,
            database_url: env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://postgres:postgres@127.0.0.1:5432/alfred".to_string()
            }),
            database_max_connections: parse_u32_env("DATABASE_MAX_CONNECTIONS", 5)?,
        })
    }
}

fn parse_u32_env(key: &str, default: u32) -> Result<u32, ConfigError> {
    match env::var(key) {
        Ok(raw) => raw
            .parse::<u32>()
            .map_err(|_| ConfigError::ParseInt(key.to_string())),
        Err(_) => Ok(default),
    }
}
