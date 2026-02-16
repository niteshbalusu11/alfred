use std::env;
use std::net::IpAddr;

use crate::config::ConfigError;

pub(crate) fn require_env(key: &str) -> Result<String, ConfigError> {
    env::var(key).map_err(|_| ConfigError::MissingVar(key.to_string()))
}

pub(crate) fn parse_u32_env(key: &str, default: u32) -> Result<u32, ConfigError> {
    match env::var(key) {
        Ok(raw) => raw
            .parse::<u32>()
            .map_err(|_| ConfigError::ParseInt(key.to_string())),
        Err(_) => Ok(default),
    }
}

pub(crate) fn parse_u64_env(key: &str, default: u64) -> Result<u64, ConfigError> {
    match env::var(key) {
        Ok(raw) => raw
            .parse::<u64>()
            .map_err(|_| ConfigError::ParseInt(key.to_string())),
        Err(_) => Ok(default),
    }
}

pub(crate) fn parse_i32_env(key: &str, default: i32) -> Result<i32, ConfigError> {
    match env::var(key) {
        Ok(raw) => raw
            .parse::<i32>()
            .map_err(|_| ConfigError::ParseInt(key.to_string())),
        Err(_) => Ok(default),
    }
}

pub(crate) fn parse_bool_env(key: &str, default: bool) -> Result<bool, ConfigError> {
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

pub(crate) fn parse_ip_list_env(key: &str) -> Result<Vec<IpAddr>, ConfigError> {
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

pub(crate) fn parse_list_env(key: &str, default: &[&str]) -> Vec<String> {
    match env::var(key) {
        Ok(raw) => parse_csv_list(raw),
        Err(_) => default.iter().map(|item| (*item).to_string()).collect(),
    }
}

pub(crate) fn parse_list_env_with_fallback(key: &str, fallback: &[String]) -> Vec<String> {
    match env::var(key) {
        Ok(raw) => parse_csv_list(raw),
        Err(_) => fallback.to_vec(),
    }
}

pub(crate) fn optional_trimmed_env(key: &str) -> Option<String> {
    env::var(key).ok().and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
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
