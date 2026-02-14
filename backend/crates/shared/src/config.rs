use std::env;
use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub bind_addr: String,
    pub database_url: String,
    pub database_max_connections: u32,
    pub migrations_dir: PathBuf,
    pub data_encryption_key: String,
    pub session_ttl_seconds: u64,
    pub oauth_state_ttl_seconds: u64,
    pub google_client_id: String,
    pub google_client_secret: String,
    pub google_redirect_uri: String,
    pub google_auth_url: String,
    pub google_token_url: String,
    pub google_revoke_url: String,
}

#[derive(Debug, Clone)]
pub struct WorkerConfig {
    pub tick_seconds: u64,
    pub database_url: String,
    pub database_max_connections: u32,
    pub data_encryption_key: String,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("missing required env var {0}")]
    MissingVar(String),
    #[error("invalid integer in env var {0}")]
    ParseInt(String),
}

impl ApiConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            bind_addr: env::var("API_BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string()),
            database_url: require_env("DATABASE_URL")?,
            database_max_connections: parse_u32_env("DATABASE_MAX_CONNECTIONS", 10)?,
            migrations_dir: env::var("MIGRATIONS_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| {
                    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../db/migrations")
                }),
            data_encryption_key: require_env("DATA_ENCRYPTION_KEY")?,
            session_ttl_seconds: parse_u64_env("SESSION_TTL_SECONDS", 3600)?,
            oauth_state_ttl_seconds: parse_u64_env("OAUTH_STATE_TTL_SECONDS", 600)?,
            google_client_id: require_env("GOOGLE_OAUTH_CLIENT_ID")?,
            google_client_secret: require_env("GOOGLE_OAUTH_CLIENT_SECRET")?,
            google_redirect_uri: require_env("GOOGLE_OAUTH_REDIRECT_URI")?,
            google_auth_url: env::var("GOOGLE_OAUTH_AUTH_URL")
                .unwrap_or_else(|_| "https://accounts.google.com/o/oauth2/v2/auth".to_string()),
            google_token_url: env::var("GOOGLE_OAUTH_TOKEN_URL")
                .unwrap_or_else(|_| "https://oauth2.googleapis.com/token".to_string()),
            google_revoke_url: env::var("GOOGLE_OAUTH_REVOKE_URL")
                .unwrap_or_else(|_| "https://oauth2.googleapis.com/revoke".to_string()),
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
            database_url: require_env("DATABASE_URL")?,
            database_max_connections: parse_u32_env("DATABASE_MAX_CONNECTIONS", 5)?,
            data_encryption_key: require_env("DATA_ENCRYPTION_KEY")?,
        })
    }
}

fn require_env(key: &str) -> Result<String, ConfigError> {
    env::var(key).map_err(|_| ConfigError::MissingVar(key.to_string()))
}

fn parse_u32_env(key: &str, default: u32) -> Result<u32, ConfigError> {
    match env::var(key) {
        Ok(raw) => raw
            .parse::<u32>()
            .map_err(|_| ConfigError::ParseInt(key.to_string())),
        Err(_) => Ok(default),
    }
}

fn parse_u64_env(key: &str, default: u64) -> Result<u64, ConfigError> {
    match env::var(key) {
        Ok(raw) => raw
            .parse::<u64>()
            .map_err(|_| ConfigError::ParseInt(key.to_string())),
        Err(_) => Ok(default),
    }
}
