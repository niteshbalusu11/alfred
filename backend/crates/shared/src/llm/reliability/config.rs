use std::env;
use std::time::Duration;

use thiserror::Error;

const DEFAULT_RATE_LIMIT_WINDOW_SECONDS: u64 = 60;
const DEFAULT_RATE_LIMIT_GLOBAL_MAX_REQUESTS: u32 = 120;
const DEFAULT_RATE_LIMIT_PER_USER_MAX_REQUESTS: u32 = 30;
const DEFAULT_CIRCUIT_BREAKER_FAILURE_THRESHOLD: u32 = 5;
const DEFAULT_CIRCUIT_BREAKER_COOLDOWN_SECONDS: u64 = 60;
const DEFAULT_CACHE_TTL_SECONDS: u64 = 20;
const DEFAULT_CACHE_MAX_ENTRIES: usize = 256;
const DEFAULT_BUDGET_WINDOW_SECONDS: u64 = 3_600;
const DEFAULT_BUDGET_MAX_ESTIMATED_COST_USD: f64 = 1.0;
pub(crate) const DEFAULT_BUDGET_MODEL: &str = "openai/gpt-4o-mini";

#[derive(Debug, Clone)]
pub struct LlmReliabilityConfig {
    pub rate_limit_window_seconds: u64,
    pub rate_limit_global_max_requests: u32,
    pub rate_limit_per_user_max_requests: u32,
    pub circuit_breaker_failure_threshold: u32,
    pub circuit_breaker_cooldown_seconds: u64,
    pub cache_ttl_seconds: u64,
    pub cache_max_entries: usize,
    pub budget_window_seconds: u64,
    pub budget_max_estimated_cost_usd: f64,
    pub budget_model: Option<String>,
}

impl Default for LlmReliabilityConfig {
    fn default() -> Self {
        Self {
            rate_limit_window_seconds: DEFAULT_RATE_LIMIT_WINDOW_SECONDS,
            rate_limit_global_max_requests: DEFAULT_RATE_LIMIT_GLOBAL_MAX_REQUESTS,
            rate_limit_per_user_max_requests: DEFAULT_RATE_LIMIT_PER_USER_MAX_REQUESTS,
            circuit_breaker_failure_threshold: DEFAULT_CIRCUIT_BREAKER_FAILURE_THRESHOLD,
            circuit_breaker_cooldown_seconds: DEFAULT_CIRCUIT_BREAKER_COOLDOWN_SECONDS,
            cache_ttl_seconds: DEFAULT_CACHE_TTL_SECONDS,
            cache_max_entries: DEFAULT_CACHE_MAX_ENTRIES,
            budget_window_seconds: DEFAULT_BUDGET_WINDOW_SECONDS,
            budget_max_estimated_cost_usd: DEFAULT_BUDGET_MAX_ESTIMATED_COST_USD,
            budget_model: Some(DEFAULT_BUDGET_MODEL.to_string()),
        }
    }
}

impl LlmReliabilityConfig {
    pub fn from_env() -> Result<Self, LlmReliabilityConfigError> {
        let mut config = Self::default();
        config.rate_limit_window_seconds = parse_u64_env(
            "LLM_RATE_LIMIT_WINDOW_SECONDS",
            config.rate_limit_window_seconds,
        )?;
        config.rate_limit_global_max_requests = parse_u32_env(
            "LLM_RATE_LIMIT_GLOBAL_MAX_REQUESTS",
            config.rate_limit_global_max_requests,
        )?;
        config.rate_limit_per_user_max_requests = parse_u32_env(
            "LLM_RATE_LIMIT_PER_USER_MAX_REQUESTS",
            config.rate_limit_per_user_max_requests,
        )?;
        config.circuit_breaker_failure_threshold = parse_u32_env(
            "LLM_CIRCUIT_BREAKER_FAILURE_THRESHOLD",
            config.circuit_breaker_failure_threshold,
        )?;
        config.circuit_breaker_cooldown_seconds = parse_u64_env(
            "LLM_CIRCUIT_BREAKER_COOLDOWN_SECONDS",
            config.circuit_breaker_cooldown_seconds,
        )?;
        config.cache_ttl_seconds =
            parse_u64_env("LLM_CACHE_TTL_SECONDS", config.cache_ttl_seconds)?;
        config.cache_max_entries =
            parse_usize_env("LLM_CACHE_MAX_ENTRIES", config.cache_max_entries)?;
        config.budget_window_seconds =
            parse_u64_env("LLM_BUDGET_WINDOW_SECONDS", config.budget_window_seconds)?;
        config.budget_max_estimated_cost_usd = parse_f64_env(
            "LLM_BUDGET_MAX_ESTIMATED_COST_USD",
            config.budget_max_estimated_cost_usd,
        )?;
        config.budget_model = optional_trimmed_env("LLM_BUDGET_MODEL").or(config.budget_model);
        config.validate()?;
        Ok(config)
    }

    pub(crate) fn validate(&self) -> Result<(), LlmReliabilityConfigError> {
        if self.rate_limit_window_seconds == 0 {
            return Err(LlmReliabilityConfigError::InvalidConfiguration(
                "LLM_RATE_LIMIT_WINDOW_SECONDS must be greater than 0".to_string(),
            ));
        }
        if self.rate_limit_global_max_requests == 0 {
            return Err(LlmReliabilityConfigError::InvalidConfiguration(
                "LLM_RATE_LIMIT_GLOBAL_MAX_REQUESTS must be greater than 0".to_string(),
            ));
        }
        if self.rate_limit_per_user_max_requests == 0 {
            return Err(LlmReliabilityConfigError::InvalidConfiguration(
                "LLM_RATE_LIMIT_PER_USER_MAX_REQUESTS must be greater than 0".to_string(),
            ));
        }
        if self.circuit_breaker_failure_threshold == 0 {
            return Err(LlmReliabilityConfigError::InvalidConfiguration(
                "LLM_CIRCUIT_BREAKER_FAILURE_THRESHOLD must be greater than 0".to_string(),
            ));
        }
        if self.circuit_breaker_cooldown_seconds == 0 {
            return Err(LlmReliabilityConfigError::InvalidConfiguration(
                "LLM_CIRCUIT_BREAKER_COOLDOWN_SECONDS must be greater than 0".to_string(),
            ));
        }
        if self.cache_ttl_seconds == 0 {
            return Err(LlmReliabilityConfigError::InvalidConfiguration(
                "LLM_CACHE_TTL_SECONDS must be greater than 0".to_string(),
            ));
        }
        if self.cache_max_entries == 0 {
            return Err(LlmReliabilityConfigError::InvalidConfiguration(
                "LLM_CACHE_MAX_ENTRIES must be greater than 0".to_string(),
            ));
        }
        if self.budget_window_seconds == 0 {
            return Err(LlmReliabilityConfigError::InvalidConfiguration(
                "LLM_BUDGET_WINDOW_SECONDS must be greater than 0".to_string(),
            ));
        }
        if !self.budget_max_estimated_cost_usd.is_finite()
            || self.budget_max_estimated_cost_usd <= 0.0
        {
            return Err(LlmReliabilityConfigError::InvalidConfiguration(
                "LLM_BUDGET_MAX_ESTIMATED_COST_USD must be a positive finite number".to_string(),
            ));
        }
        Ok(())
    }

    pub(crate) fn rate_limit_window(&self) -> Duration {
        Duration::from_secs(self.rate_limit_window_seconds)
    }

    pub(crate) fn circuit_breaker_cooldown(&self) -> Duration {
        Duration::from_secs(self.circuit_breaker_cooldown_seconds)
    }

    pub(crate) fn cache_ttl(&self) -> Duration {
        Duration::from_secs(self.cache_ttl_seconds)
    }

    pub(crate) fn budget_window(&self) -> Duration {
        Duration::from_secs(self.budget_window_seconds)
    }
}

#[derive(Debug, Error)]
pub enum LlmReliabilityConfigError {
    #[error("invalid integer in env var {key}: {value}")]
    ParseInt { key: String, value: String },
    #[error("invalid float in env var {key}: {value}")]
    ParseFloat { key: String, value: String },
    #[error("invalid configuration: {0}")]
    InvalidConfiguration(String),
}

fn optional_trimmed_env(key: &str) -> Option<String> {
    env::var(key).ok().and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn parse_u64_env(key: &str, default: u64) -> Result<u64, LlmReliabilityConfigError> {
    match optional_trimmed_env(key) {
        Some(value) => value
            .parse::<u64>()
            .map_err(|_| LlmReliabilityConfigError::ParseInt {
                key: key.to_string(),
                value,
            }),
        None => Ok(default),
    }
}

fn parse_u32_env(key: &str, default: u32) -> Result<u32, LlmReliabilityConfigError> {
    match optional_trimmed_env(key) {
        Some(value) => value
            .parse::<u32>()
            .map_err(|_| LlmReliabilityConfigError::ParseInt {
                key: key.to_string(),
                value,
            }),
        None => Ok(default),
    }
}

fn parse_usize_env(key: &str, default: usize) -> Result<usize, LlmReliabilityConfigError> {
    match optional_trimmed_env(key) {
        Some(value) => value
            .parse::<usize>()
            .map_err(|_| LlmReliabilityConfigError::ParseInt {
                key: key.to_string(),
                value,
            }),
        None => Ok(default),
    }
}

fn parse_f64_env(key: &str, default: f64) -> Result<f64, LlmReliabilityConfigError> {
    match optional_trimmed_env(key) {
        Some(value) => value
            .parse::<f64>()
            .map_err(|_| LlmReliabilityConfigError::ParseFloat {
                key: key.to_string(),
                value,
            }),
        None => Ok(default),
    }
}
