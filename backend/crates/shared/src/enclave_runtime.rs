use std::str::FromStr;
use std::time::Duration;

use reqwest::StatusCode;
use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlfredEnvironment {
    Local,
    Staging,
    Production,
}

impl AlfredEnvironment {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Staging => "staging",
            Self::Production => "production",
        }
    }
}

impl FromStr for AlfredEnvironment {
    type Err = String;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "local" | "dev" => Ok(Self::Local),
            "staging" => Ok(Self::Staging),
            "production" | "prod" => Ok(Self::Production),
            _ => Err(format!(
                "ALFRED_ENV must be one of local, staging, production; got '{}'",
                raw
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnclaveRuntimeMode {
    Disabled,
    DevShim,
    Remote,
}

impl EnclaveRuntimeMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::DevShim => "dev-shim",
            Self::Remote => "remote",
        }
    }
}

impl FromStr for EnclaveRuntimeMode {
    type Err = String;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "disabled" => Ok(Self::Disabled),
            "dev-shim" | "dev_shim" => Ok(Self::DevShim),
            "remote" => Ok(Self::Remote),
            _ => Err(format!(
                "ENCLAVE_RUNTIME_MODE must be one of disabled, dev-shim, remote; got '{}'",
                raw
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EnclaveRuntimeEndpointConfig {
    pub mode: EnclaveRuntimeMode,
    pub base_url: String,
    pub probe_timeout_ms: u64,
}

#[derive(Debug, Error)]
pub enum EnclaveRuntimeProbeError {
    #[error("failed to call enclave runtime endpoint {url}: {message}")]
    RequestFailed { url: String, message: String },
    #[error("enclave runtime endpoint {url} returned HTTP {status}")]
    UnexpectedStatus { url: String, status: u16 },
    #[error("enclave runtime attestation response was invalid: {0}")]
    InvalidAttestationResponse(String),
}

pub async fn verify_connectivity(
    client: &reqwest::Client,
    config: &EnclaveRuntimeEndpointConfig,
) -> Result<(), EnclaveRuntimeProbeError> {
    if matches!(config.mode, EnclaveRuntimeMode::Disabled) {
        return Ok(());
    }

    let timeout = Duration::from_millis(config.probe_timeout_ms);
    let healthz_url = format!("{}/healthz", config.base_url.trim_end_matches('/'));
    let healthz_response = client
        .get(&healthz_url)
        .timeout(timeout)
        .send()
        .await
        .map_err(|err| EnclaveRuntimeProbeError::RequestFailed {
            url: healthz_url.clone(),
            message: err.to_string(),
        })?;
    if healthz_response.status() != StatusCode::OK {
        return Err(EnclaveRuntimeProbeError::UnexpectedStatus {
            url: healthz_url,
            status: healthz_response.status().as_u16(),
        });
    }

    let attestation_url = format!(
        "{}/v1/attestation/document",
        config.base_url.trim_end_matches('/')
    );
    let attestation_response = client
        .get(&attestation_url)
        .timeout(timeout)
        .send()
        .await
        .map_err(|err| EnclaveRuntimeProbeError::RequestFailed {
            url: attestation_url.clone(),
            message: err.to_string(),
        })?;
    if attestation_response.status() != StatusCode::OK {
        return Err(EnclaveRuntimeProbeError::UnexpectedStatus {
            url: attestation_url,
            status: attestation_response.status().as_u16(),
        });
    }

    let attestation = attestation_response
        .json::<AttestationDocument>()
        .await
        .map_err(|err| EnclaveRuntimeProbeError::InvalidAttestationResponse(err.to_string()))?;
    if attestation.runtime.trim().is_empty() || attestation.measurement.trim().is_empty() {
        return Err(EnclaveRuntimeProbeError::InvalidAttestationResponse(
            "runtime and measurement are required fields".to_string(),
        ));
    }

    Ok(())
}

#[derive(Debug, Deserialize)]
struct AttestationDocument {
    runtime: String,
    measurement: String,
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::{AlfredEnvironment, EnclaveRuntimeMode};

    #[test]
    fn parse_alfred_environment_aliases() {
        assert!(matches!(
            AlfredEnvironment::from_str("dev").expect("dev should parse"),
            AlfredEnvironment::Local
        ));
        assert!(matches!(
            AlfredEnvironment::from_str("staging").expect("staging should parse"),
            AlfredEnvironment::Staging
        ));
        assert!(matches!(
            AlfredEnvironment::from_str("prod").expect("prod should parse"),
            AlfredEnvironment::Production
        ));
    }

    #[test]
    fn parse_enclave_runtime_mode_values() {
        assert!(matches!(
            EnclaveRuntimeMode::from_str("disabled").expect("disabled should parse"),
            EnclaveRuntimeMode::Disabled
        ));
        assert!(matches!(
            EnclaveRuntimeMode::from_str("dev-shim").expect("dev-shim should parse"),
            EnclaveRuntimeMode::DevShim
        ));
        assert!(matches!(
            EnclaveRuntimeMode::from_str("remote").expect("remote should parse"),
            EnclaveRuntimeMode::Remote
        ));
    }
}
