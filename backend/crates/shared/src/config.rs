use std::env;
use std::io::ErrorKind;
use std::net::IpAddr;
use std::path::PathBuf;

use thiserror::Error;

use crate::config_enclave_runtime::{
    parse_alfred_environment, parse_enclave_rpc_shared_secret, parse_enclave_runtime_mode,
    validate_enclave_runtime_guards,
};
use crate::enclave_runtime::EnclaveRuntimeMode;

#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub bind_addr: String,
    pub database_url: String,
    pub database_max_connections: u32,
    pub migrations_dir: PathBuf,
    pub data_encryption_key: String,
    pub oauth_state_ttl_seconds: u64,
    pub clerk_issuer: String,
    pub clerk_audience: String,
    pub clerk_secret_key: String,
    pub clerk_jwks_url: String,
    pub redis_url: String,
    pub clerk_jwks_cache_key: String,
    pub clerk_jwks_cache_default_ttl_seconds: u64,
    pub clerk_jwks_cache_stale_ttl_seconds: u64,
    pub google_client_id: String,
    pub google_client_secret: String,
    pub google_redirect_uri: String,
    pub google_auth_url: String,
    pub google_token_url: String,
    pub google_revoke_url: String,
    pub trusted_proxy_ips: Vec<IpAddr>,
    pub tee_attestation_required: bool,
    pub tee_expected_runtime: String,
    pub tee_allowed_measurements: Vec<String>,
    pub tee_attestation_public_key: Option<String>,
    pub tee_attestation_max_age_seconds: u64,
    pub tee_attestation_challenge_timeout_ms: u64,
    pub tee_allow_insecure_dev_attestation: bool,
    pub kms_key_id: String,
    pub kms_key_version: i32,
    pub kms_allowed_measurements: Vec<String>,
    pub enclave_runtime_mode: EnclaveRuntimeMode,
    pub enclave_runtime_base_url: String,
    pub enclave_runtime_probe_timeout_ms: u64,
    pub enclave_rpc_shared_secret: String,
    pub enclave_rpc_auth_max_skew_seconds: u64,
}

#[derive(Debug, Clone)]
pub struct WorkerConfig {
    pub tick_seconds: u64,
    pub batch_size: u32,
    pub lease_seconds: u64,
    pub per_user_concurrency_limit: u32,
    pub retry_base_delay_seconds: u64,
    pub retry_max_delay_seconds: u64,
    pub apns_sandbox_endpoint: Option<String>,
    pub apns_production_endpoint: Option<String>,
    pub apns_auth_token: Option<String>,
    pub google_client_id: String,
    pub google_client_secret: String,
    pub google_token_url: String,
    pub google_revoke_url: String,
    pub privacy_delete_batch_size: u32,
    pub privacy_delete_lease_seconds: u64,
    pub privacy_delete_sla_hours: u64,
    pub tee_attestation_required: bool,
    pub tee_expected_runtime: String,
    pub tee_allowed_measurements: Vec<String>,
    pub tee_attestation_public_key: Option<String>,
    pub tee_attestation_max_age_seconds: u64,
    pub tee_attestation_challenge_timeout_ms: u64,
    pub tee_allow_insecure_dev_attestation: bool,
    pub kms_key_id: String,
    pub kms_key_version: i32,
    pub kms_allowed_measurements: Vec<String>,
    pub enclave_runtime_mode: EnclaveRuntimeMode,
    pub enclave_runtime_base_url: String,
    pub enclave_runtime_probe_timeout_ms: u64,
    pub enclave_rpc_shared_secret: String,
    pub enclave_rpc_auth_max_skew_seconds: u64,
    pub database_url: String,
    pub database_max_connections: u32,
    pub data_encryption_key: String,
    pub redis_url: String,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("missing required env var {0}")]
    MissingVar(String),
    #[error("invalid integer in env var {0}")]
    ParseInt(String),
    #[error("invalid boolean in env var {0}")]
    ParseBool(String),
    #[error("invalid configuration: {0}")]
    InvalidConfiguration(String),
    #[error("failed to load .env: {0}")]
    Dotenv(String),
}

pub fn load_dotenv() -> Result<(), ConfigError> {
    match dotenvy::dotenv() {
        Ok(_) => Ok(()),
        Err(dotenvy::Error::Io(err)) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(ConfigError::Dotenv(err.to_string())),
    }
}

impl ApiConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let alfred_environment = parse_alfred_environment()?;
        let tee_allowed_measurements =
            parse_list_env("TEE_ALLOWED_MEASUREMENTS", &["dev-local-enclave"]);
        let tee_attestation_required = parse_bool_env("TEE_ATTESTATION_REQUIRED", true)?;
        let tee_allow_insecure_dev_attestation =
            parse_bool_env("TEE_ALLOW_INSECURE_DEV_ATTESTATION", false)?;
        let tee_attestation_challenge_timeout_ms =
            parse_u64_env("TEE_ATTESTATION_CHALLENGE_TIMEOUT_MS", 2000)?;
        if tee_attestation_challenge_timeout_ms == 0 {
            return Err(ConfigError::InvalidConfiguration(
                "TEE_ATTESTATION_CHALLENGE_TIMEOUT_MS must be greater than 0".to_string(),
            ));
        }
        let enclave_runtime_mode =
            parse_enclave_runtime_mode("ENCLAVE_RUNTIME_MODE", alfred_environment)?;
        validate_enclave_runtime_guards(
            alfred_environment,
            enclave_runtime_mode,
            tee_attestation_required,
            tee_allow_insecure_dev_attestation,
        )?;
        let enclave_runtime_probe_timeout_ms =
            parse_u64_env("ENCLAVE_RUNTIME_PROBE_TIMEOUT_MS", 2000)?;
        if enclave_runtime_probe_timeout_ms == 0 {
            return Err(ConfigError::InvalidConfiguration(
                "ENCLAVE_RUNTIME_PROBE_TIMEOUT_MS must be greater than 0".to_string(),
            ));
        }
        let enclave_rpc_auth_max_skew_seconds =
            parse_u64_env("ENCLAVE_RPC_AUTH_MAX_SKEW_SECONDS", 30)?;
        if enclave_rpc_auth_max_skew_seconds == 0 {
            return Err(ConfigError::InvalidConfiguration(
                "ENCLAVE_RPC_AUTH_MAX_SKEW_SECONDS must be greater than 0".to_string(),
            ));
        }
        let enclave_rpc_shared_secret = parse_enclave_rpc_shared_secret(alfred_environment)?;

        let clerk_issuer = require_env("CLERK_ISSUER")?;
        if clerk_issuer.trim().is_empty() {
            return Err(ConfigError::InvalidConfiguration(
                "CLERK_ISSUER must not be empty".to_string(),
            ));
        }
        let clerk_audience = require_env("CLERK_AUDIENCE")?;
        if clerk_audience.trim().is_empty() {
            return Err(ConfigError::InvalidConfiguration(
                "CLERK_AUDIENCE must not be empty".to_string(),
            ));
        }
        let clerk_secret_key = require_env("CLERK_SECRET_KEY")?;
        if clerk_secret_key.trim().is_empty() {
            return Err(ConfigError::InvalidConfiguration(
                "CLERK_SECRET_KEY must not be empty".to_string(),
            ));
        }
        let clerk_backend_api_url = optional_trimmed_env("CLERK_BACKEND_API_URL")
            .unwrap_or_else(|| "https://api.clerk.com/v1".to_string());
        if clerk_backend_api_url.trim().is_empty() {
            return Err(ConfigError::InvalidConfiguration(
                "CLERK_BACKEND_API_URL must not be empty".to_string(),
            ));
        }
        let clerk_jwks_url = format!("{}/jwks", clerk_backend_api_url.trim_end_matches('/'));
        let clerk_jwks_cache_default_ttl_seconds =
            parse_u64_env("CLERK_JWKS_CACHE_DEFAULT_TTL_SECONDS", 300)?;
        if clerk_jwks_cache_default_ttl_seconds == 0 {
            return Err(ConfigError::InvalidConfiguration(
                "CLERK_JWKS_CACHE_DEFAULT_TTL_SECONDS must be greater than 0".to_string(),
            ));
        }
        let clerk_jwks_cache_stale_ttl_seconds =
            parse_u64_env("CLERK_JWKS_CACHE_STALE_TTL_SECONDS", 300)?;
        if clerk_jwks_cache_stale_ttl_seconds == 0 {
            return Err(ConfigError::InvalidConfiguration(
                "CLERK_JWKS_CACHE_STALE_TTL_SECONDS must be greater than 0".to_string(),
            ));
        }

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
            oauth_state_ttl_seconds: parse_u64_env("OAUTH_STATE_TTL_SECONDS", 600)?,
            clerk_issuer,
            clerk_audience,
            clerk_secret_key,
            clerk_jwks_url,
            redis_url: optional_trimmed_env("REDIS_URL")
                .unwrap_or_else(|| "redis://127.0.0.1:6379/0".to_string()),
            clerk_jwks_cache_key: optional_trimmed_env("CLERK_JWKS_CACHE_KEY")
                .unwrap_or_else(|| "alfred:clerk:jwks:v1".to_string()),
            clerk_jwks_cache_default_ttl_seconds,
            clerk_jwks_cache_stale_ttl_seconds,
            google_client_id: require_env("GOOGLE_OAUTH_CLIENT_ID")?,
            google_client_secret: require_env("GOOGLE_OAUTH_CLIENT_SECRET")?,
            google_redirect_uri: require_env("GOOGLE_OAUTH_REDIRECT_URI")?,
            google_auth_url: env::var("GOOGLE_OAUTH_AUTH_URL")
                .unwrap_or_else(|_| "https://accounts.google.com/o/oauth2/v2/auth".to_string()),
            google_token_url: env::var("GOOGLE_OAUTH_TOKEN_URL")
                .unwrap_or_else(|_| "https://oauth2.googleapis.com/token".to_string()),
            google_revoke_url: env::var("GOOGLE_OAUTH_REVOKE_URL")
                .unwrap_or_else(|_| "https://oauth2.googleapis.com/revoke".to_string()),
            trusted_proxy_ips: parse_ip_list_env("TRUSTED_PROXY_IPS")?,
            tee_attestation_required,
            tee_expected_runtime: env::var("TEE_EXPECTED_RUNTIME")
                .unwrap_or_else(|_| "nitro".to_string()),
            tee_allowed_measurements: tee_allowed_measurements.clone(),
            tee_attestation_public_key: env::var("TEE_ATTESTATION_PUBLIC_KEY").ok(),
            tee_attestation_max_age_seconds: parse_u64_env("TEE_ATTESTATION_MAX_AGE_SECONDS", 300)?,
            tee_attestation_challenge_timeout_ms,
            tee_allow_insecure_dev_attestation,
            kms_key_id: env::var("KMS_KEY_ID")
                .unwrap_or_else(|_| "kms/local/alfred-refresh-token".to_string()),
            kms_key_version: parse_i32_env("KMS_KEY_VERSION", 1)?,
            kms_allowed_measurements: parse_list_env_with_fallback(
                "KMS_ALLOWED_MEASUREMENTS",
                &tee_allowed_measurements,
            ),
            enclave_runtime_mode,
            enclave_runtime_base_url: env::var("ENCLAVE_RUNTIME_BASE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8181".to_string()),
            enclave_runtime_probe_timeout_ms,
            enclave_rpc_shared_secret,
            enclave_rpc_auth_max_skew_seconds,
        })
    }
}

impl WorkerConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let alfred_environment = parse_alfred_environment()?;
        let tee_allowed_measurements =
            parse_list_env("TEE_ALLOWED_MEASUREMENTS", &["dev-local-enclave"]);
        let tick_seconds = match env::var("WORKER_TICK_SECONDS") {
            Ok(raw) => raw
                .parse::<u64>()
                .map_err(|_| ConfigError::ParseInt("WORKER_TICK_SECONDS".to_string()))?,
            Err(_) => 30,
        };
        let batch_size = parse_u32_env("WORKER_BATCH_SIZE", 25)?;
        let lease_seconds = parse_u64_env("WORKER_LEASE_SECONDS", 60)?;
        let per_user_concurrency_limit = parse_u32_env("WORKER_PER_USER_CONCURRENCY_LIMIT", 1)?;
        let retry_base_delay_seconds = parse_u64_env("WORKER_RETRY_BASE_DELAY_SECONDS", 30)?;
        let retry_max_delay_seconds = parse_u64_env("WORKER_RETRY_MAX_DELAY_SECONDS", 1800)?;
        let privacy_delete_batch_size = parse_u32_env("WORKER_PRIVACY_DELETE_BATCH_SIZE", 10)?;
        let privacy_delete_lease_seconds =
            parse_u64_env("WORKER_PRIVACY_DELETE_LEASE_SECONDS", 120)?;
        let privacy_delete_sla_hours = parse_u64_env("PRIVACY_DELETE_SLA_HOURS", 24)?;

        if batch_size == 0 {
            return Err(ConfigError::InvalidConfiguration(
                "WORKER_BATCH_SIZE must be greater than 0".to_string(),
            ));
        }
        if lease_seconds == 0 {
            return Err(ConfigError::InvalidConfiguration(
                "WORKER_LEASE_SECONDS must be greater than 0".to_string(),
            ));
        }
        if per_user_concurrency_limit == 0 {
            return Err(ConfigError::InvalidConfiguration(
                "WORKER_PER_USER_CONCURRENCY_LIMIT must be greater than 0".to_string(),
            ));
        }
        if retry_base_delay_seconds == 0 {
            return Err(ConfigError::InvalidConfiguration(
                "WORKER_RETRY_BASE_DELAY_SECONDS must be greater than 0".to_string(),
            ));
        }
        if retry_max_delay_seconds < retry_base_delay_seconds {
            return Err(ConfigError::InvalidConfiguration(
                "WORKER_RETRY_MAX_DELAY_SECONDS must be >= WORKER_RETRY_BASE_DELAY_SECONDS"
                    .to_string(),
            ));
        }
        if privacy_delete_batch_size == 0 {
            return Err(ConfigError::InvalidConfiguration(
                "WORKER_PRIVACY_DELETE_BATCH_SIZE must be greater than 0".to_string(),
            ));
        }
        if privacy_delete_lease_seconds == 0 {
            return Err(ConfigError::InvalidConfiguration(
                "WORKER_PRIVACY_DELETE_LEASE_SECONDS must be greater than 0".to_string(),
            ));
        }
        if privacy_delete_sla_hours == 0 {
            return Err(ConfigError::InvalidConfiguration(
                "PRIVACY_DELETE_SLA_HOURS must be greater than 0".to_string(),
            ));
        }

        let tee_attestation_required = parse_bool_env("TEE_ATTESTATION_REQUIRED", true)?;
        let tee_allow_insecure_dev_attestation =
            parse_bool_env("TEE_ALLOW_INSECURE_DEV_ATTESTATION", false)?;
        let tee_attestation_challenge_timeout_ms =
            parse_u64_env("TEE_ATTESTATION_CHALLENGE_TIMEOUT_MS", 2000)?;
        if tee_attestation_challenge_timeout_ms == 0 {
            return Err(ConfigError::InvalidConfiguration(
                "TEE_ATTESTATION_CHALLENGE_TIMEOUT_MS must be greater than 0".to_string(),
            ));
        }
        let enclave_runtime_mode =
            parse_enclave_runtime_mode("ENCLAVE_RUNTIME_MODE", alfred_environment)?;
        validate_enclave_runtime_guards(
            alfred_environment,
            enclave_runtime_mode,
            tee_attestation_required,
            tee_allow_insecure_dev_attestation,
        )?;
        let enclave_runtime_probe_timeout_ms =
            parse_u64_env("ENCLAVE_RUNTIME_PROBE_TIMEOUT_MS", 2000)?;
        if enclave_runtime_probe_timeout_ms == 0 {
            return Err(ConfigError::InvalidConfiguration(
                "ENCLAVE_RUNTIME_PROBE_TIMEOUT_MS must be greater than 0".to_string(),
            ));
        }
        let enclave_rpc_auth_max_skew_seconds =
            parse_u64_env("ENCLAVE_RPC_AUTH_MAX_SKEW_SECONDS", 30)?;
        if enclave_rpc_auth_max_skew_seconds == 0 {
            return Err(ConfigError::InvalidConfiguration(
                "ENCLAVE_RPC_AUTH_MAX_SKEW_SECONDS must be greater than 0".to_string(),
            ));
        }
        let enclave_rpc_shared_secret = parse_enclave_rpc_shared_secret(alfred_environment)?;

        Ok(Self {
            tick_seconds,
            batch_size,
            lease_seconds,
            per_user_concurrency_limit,
            retry_base_delay_seconds,
            retry_max_delay_seconds,
            apns_sandbox_endpoint: optional_trimmed_env("APNS_SANDBOX_ENDPOINT"),
            apns_production_endpoint: optional_trimmed_env("APNS_PRODUCTION_ENDPOINT"),
            apns_auth_token: optional_trimmed_env("APNS_AUTH_TOKEN"),
            google_client_id: require_env("GOOGLE_OAUTH_CLIENT_ID")?,
            google_client_secret: require_env("GOOGLE_OAUTH_CLIENT_SECRET")?,
            google_token_url: env::var("GOOGLE_OAUTH_TOKEN_URL")
                .unwrap_or_else(|_| "https://oauth2.googleapis.com/token".to_string()),
            google_revoke_url: env::var("GOOGLE_OAUTH_REVOKE_URL")
                .unwrap_or_else(|_| "https://oauth2.googleapis.com/revoke".to_string()),
            privacy_delete_batch_size,
            privacy_delete_lease_seconds,
            privacy_delete_sla_hours,
            tee_attestation_required,
            tee_expected_runtime: env::var("TEE_EXPECTED_RUNTIME")
                .unwrap_or_else(|_| "nitro".to_string()),
            tee_allowed_measurements: tee_allowed_measurements.clone(),
            tee_attestation_public_key: env::var("TEE_ATTESTATION_PUBLIC_KEY").ok(),
            tee_attestation_max_age_seconds: parse_u64_env("TEE_ATTESTATION_MAX_AGE_SECONDS", 300)?,
            tee_attestation_challenge_timeout_ms,
            tee_allow_insecure_dev_attestation,
            kms_key_id: env::var("KMS_KEY_ID")
                .unwrap_or_else(|_| "kms/local/alfred-refresh-token".to_string()),
            kms_key_version: parse_i32_env("KMS_KEY_VERSION", 1)?,
            kms_allowed_measurements: parse_list_env_with_fallback(
                "KMS_ALLOWED_MEASUREMENTS",
                &tee_allowed_measurements,
            ),
            enclave_runtime_mode,
            enclave_runtime_base_url: env::var("ENCLAVE_RUNTIME_BASE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8181".to_string()),
            enclave_runtime_probe_timeout_ms,
            enclave_rpc_shared_secret,
            enclave_rpc_auth_max_skew_seconds,
            database_url: require_env("DATABASE_URL")?,
            database_max_connections: parse_u32_env("DATABASE_MAX_CONNECTIONS", 5)?,
            data_encryption_key: require_env("DATA_ENCRYPTION_KEY")?,
            redis_url: optional_trimmed_env("REDIS_URL")
                .unwrap_or_else(|| "redis://127.0.0.1:6379/0".to_string()),
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

fn parse_i32_env(key: &str, default: i32) -> Result<i32, ConfigError> {
    match env::var(key) {
        Ok(raw) => raw
            .parse::<i32>()
            .map_err(|_| ConfigError::ParseInt(key.to_string())),
        Err(_) => Ok(default),
    }
}

fn parse_bool_env(key: &str, default: bool) -> Result<bool, ConfigError> {
    match env::var(key) {
        Ok(raw) => {
            let normalized = raw.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "true" | "1" | "yes" | "on" => Ok(true),
                "false" | "0" | "no" | "off" => Ok(false),
                _ => Err(ConfigError::ParseBool(key.to_string())),
            }
        }
        Err(_) => Ok(default),
    }
}

fn parse_ip_list_env(key: &str) -> Result<Vec<IpAddr>, ConfigError> {
    let Some(raw) = optional_trimmed_env(key) else {
        return Ok(Vec::new());
    };

    raw.split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| {
            item.parse::<IpAddr>().map_err(|_| {
                ConfigError::InvalidConfiguration(format!(
                    "{key} contains invalid IP address '{item}'"
                ))
            })
        })
        .collect()
}

fn parse_list_env(key: &str, default: &[&str]) -> Vec<String> {
    match env::var(key) {
        Ok(raw) => parse_csv_list(raw),
        Err(_) => default.iter().map(|item| (*item).to_string()).collect(),
    }
}

fn parse_list_env_with_fallback(key: &str, fallback: &[String]) -> Vec<String> {
    match env::var(key) {
        Ok(raw) => parse_csv_list(raw),
        Err(_) => fallback.to_vec(),
    }
}

fn parse_csv_list(raw: String) -> Vec<String> {
    let parsed = raw
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    if parsed.is_empty() {
        vec!["dev-local-enclave".to_string()]
    } else {
        parsed
    }
}

fn optional_trimmed_env(key: &str) -> Option<String> {
    env::var(key).ok().and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}
