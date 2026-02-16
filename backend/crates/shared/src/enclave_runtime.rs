use std::str::FromStr;
use std::time::Duration;

use chrono::Utc;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttestationChallengeRequest {
    pub challenge_nonce: String,
    pub issued_at: i64,
    pub expires_at: i64,
    pub operation_purpose: String,
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttestationChallengeResponse {
    pub runtime: String,
    pub measurement: String,
    pub challenge_nonce: String,
    pub issued_at: i64,
    pub expires_at: i64,
    pub operation_purpose: String,
    pub request_id: String,
    pub evidence_issued_at: i64,
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantAttestedKeyChallengeRequest {
    pub challenge_nonce: String,
    pub issued_at: i64,
    pub expires_at: i64,
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantAttestedKeyChallengeResponse {
    pub runtime: String,
    pub measurement: String,
    pub challenge_nonce: String,
    pub issued_at: i64,
    pub expires_at: i64,
    pub request_id: String,
    pub evidence_issued_at: i64,
    pub key_id: String,
    pub algorithm: String,
    pub public_key: String,
    pub key_expires_at: i64,
    pub signature: Option<String>,
}

pub fn attestation_signing_payload(response: &AttestationChallengeResponse) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}|{}|{}",
        response.runtime,
        response.measurement,
        response.challenge_nonce,
        response.issued_at,
        response.expires_at,
        response.operation_purpose,
        response.request_id,
        response.evidence_issued_at
    )
}

pub fn assistant_key_attestation_signing_payload(
    response: &AssistantAttestedKeyChallengeResponse,
) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        response.runtime,
        response.measurement,
        response.challenge_nonce,
        response.issued_at,
        response.expires_at,
        response.request_id,
        response.evidence_issued_at,
        response.key_id,
        response.algorithm,
        response.public_key,
        response.key_expires_at
    )
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

    let now = Utc::now().timestamp();
    let challenge = AttestationChallengeRequest {
        challenge_nonce: "startup-probe".to_string(),
        issued_at: now,
        expires_at: now + 15,
        operation_purpose: "startup_probe".to_string(),
        request_id: "startup-probe".to_string(),
    };
    let challenge_url = format!(
        "{}/v1/attestation/challenge",
        config.base_url.trim_end_matches('/')
    );
    let challenge_response = client
        .post(&challenge_url)
        .timeout(timeout)
        .json(&challenge)
        .send()
        .await
        .map_err(|err| EnclaveRuntimeProbeError::RequestFailed {
            url: challenge_url.clone(),
            message: err.to_string(),
        })?;
    if challenge_response.status() != StatusCode::OK {
        return Err(EnclaveRuntimeProbeError::UnexpectedStatus {
            url: challenge_url,
            status: challenge_response.status().as_u16(),
        });
    }

    let challenge_payload = challenge_response
        .json::<AttestationChallengeResponse>()
        .await
        .map_err(|err| EnclaveRuntimeProbeError::InvalidAttestationResponse(err.to_string()))?;
    if challenge_payload.challenge_nonce != challenge.challenge_nonce
        || challenge_payload.request_id != challenge.request_id
        || challenge_payload.operation_purpose != challenge.operation_purpose
    {
        return Err(EnclaveRuntimeProbeError::InvalidAttestationResponse(
            "challenge response does not echo challenge fields".to_string(),
        ));
    }
    if challenge_payload.runtime.trim().is_empty()
        || challenge_payload.measurement.trim().is_empty()
    {
        return Err(EnclaveRuntimeProbeError::InvalidAttestationResponse(
            "challenge response runtime and measurement are required fields".to_string(),
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
