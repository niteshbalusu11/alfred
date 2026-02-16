use std::env;
use std::path::PathBuf;

use base64::Engine as _;
use chrono::Utc;
use ed25519_dalek::{Signer, SigningKey};
use serde::Deserialize;
use serde_json::{Value, json};
use shared::enclave::{EnclaveRpcAuthConfig, GoogleEnclaveOauthConfig};
use shared::enclave_runtime::{
    AlfredEnvironment, AttestationChallengeRequest, AttestationChallengeResponse,
    EnclaveRuntimeMode, attestation_signing_payload,
};

#[derive(Debug, Clone)]
pub(crate) struct RuntimeConfig {
    pub(crate) bind_addr: String,
    pub(crate) environment: AlfredEnvironment,
    pub(crate) mode: EnclaveRuntimeMode,
    pub(crate) runtime_id: String,
    pub(crate) measurement: String,
    pub(crate) database_url: String,
    pub(crate) database_max_connections: u32,
    pub(crate) data_encryption_key: String,
    pub(crate) tee_attestation_required: bool,
    pub(crate) tee_expected_runtime: String,
    pub(crate) tee_allowed_measurements: Vec<String>,
    pub(crate) tee_attestation_public_key: Option<String>,
    pub(crate) tee_attestation_max_age_seconds: u64,
    pub(crate) tee_attestation_challenge_timeout_ms: u64,
    pub(crate) tee_allow_insecure_dev_attestation: bool,
    pub(crate) kms_key_id: String,
    pub(crate) kms_key_version: i32,
    pub(crate) kms_allowed_measurements: Vec<String>,
    pub(crate) enclave_runtime_base_url: String,
    pub(crate) oauth: GoogleEnclaveOauthConfig,
    pub(crate) enclave_rpc_auth: EnclaveRpcAuthConfig,
    attestation_source: AttestationSource,
    attestation_signing_private_key: [u8; 32],
}

#[derive(Debug, Clone)]
enum AttestationSource {
    Inline(String),
    FilePath(PathBuf),
    Missing,
}

#[derive(Debug, Deserialize)]
struct AttestationIdentityDocument {
    runtime: String,
    measurement: String,
}

impl RuntimeConfig {
    pub(crate) fn from_env() -> Result<Self, String> {
        let environment = env::var("ALFRED_ENV")
            .unwrap_or_else(|_| "local".to_string())
            .parse::<AlfredEnvironment>()
            .map_err(|err| format!("invalid environment: {err}"))?;
        let default_mode = if matches!(environment, AlfredEnvironment::Local) {
            "dev-shim"
        } else {
            "remote"
        };
        let mode = env::var("ENCLAVE_RUNTIME_MODE")
            .unwrap_or_else(|_| default_mode.to_string())
            .parse::<EnclaveRuntimeMode>()
            .map_err(|err| format!("invalid enclave runtime mode: {err}"))?;

        let runtime_id = env::var("TEE_EXPECTED_RUNTIME").unwrap_or_else(|_| "nitro".to_string());
        let measurement = env::var("ENCLAVE_RUNTIME_MEASUREMENT")
            .unwrap_or_else(|_| "dev-local-enclave".to_string());
        let attestation_source = if let Ok(path) = env::var("TEE_ATTESTATION_DOCUMENT_PATH") {
            AttestationSource::FilePath(PathBuf::from(path))
        } else if let Ok(document) = env::var("TEE_ATTESTATION_DOCUMENT") {
            AttestationSource::Inline(document)
        } else {
            AttestationSource::Missing
        };

        let attestation_signing_private_key = if let Ok(encoded_key) =
            env::var("TEE_ATTESTATION_SIGNING_PRIVATE_KEY")
        {
            decode_signing_key_bytes(encoded_key.as_str())?
        } else if matches!(mode, EnclaveRuntimeMode::DevShim) {
            [7_u8; 32]
        } else {
            return Err(
                "remote mode requires TEE_ATTESTATION_SIGNING_PRIVATE_KEY (base64 32-byte Ed25519 key)"
                    .to_string(),
            );
        };

        if matches!(mode, EnclaveRuntimeMode::Disabled) {
            return Err(
                "ENCLAVE_RUNTIME_MODE=disabled is invalid for enclave-runtime process".to_string(),
            );
        }

        if !matches!(environment, AlfredEnvironment::Local)
            && matches!(mode, EnclaveRuntimeMode::DevShim)
        {
            return Err(
                "ENCLAVE_RUNTIME_MODE=dev-shim is only allowed when ALFRED_ENV=local".to_string(),
            );
        }

        if matches!(mode, EnclaveRuntimeMode::Remote)
            && matches!(attestation_source, AttestationSource::Missing)
        {
            return Err(
                "remote mode requires TEE_ATTESTATION_DOCUMENT_PATH or TEE_ATTESTATION_DOCUMENT"
                    .to_string(),
            );
        }
        if !matches!(environment, AlfredEnvironment::Local)
            && matches!(attestation_source, AttestationSource::Inline(_))
        {
            return Err(
                "TEE_ATTESTATION_DOCUMENT inline mode is only allowed when ALFRED_ENV=local"
                    .to_string(),
            );
        }

        let tee_allowed_measurements =
            parse_list_env("TEE_ALLOWED_MEASUREMENTS", &["dev-local-enclave"]);
        let tee_attestation_required = parse_bool_env("TEE_ATTESTATION_REQUIRED", true)?;
        let tee_allow_insecure_dev_attestation =
            parse_bool_env("TEE_ALLOW_INSECURE_DEV_ATTESTATION", false)?;

        let tee_attestation_challenge_timeout_ms =
            parse_u64_env("TEE_ATTESTATION_CHALLENGE_TIMEOUT_MS", 2000)?;
        if tee_attestation_challenge_timeout_ms == 0 {
            return Err("TEE_ATTESTATION_CHALLENGE_TIMEOUT_MS must be > 0".to_string());
        }

        let enclave_rpc_auth_max_skew_seconds =
            parse_u64_env("ENCLAVE_RPC_AUTH_MAX_SKEW_SECONDS", 30)?;
        if enclave_rpc_auth_max_skew_seconds == 0 {
            return Err("ENCLAVE_RPC_AUTH_MAX_SKEW_SECONDS must be > 0".to_string());
        }
        let kms_allowed_measurements =
            parse_list_env_with_fallback("KMS_ALLOWED_MEASUREMENTS", &tee_allowed_measurements);
        let enclave_runtime_base_url = env::var("ENCLAVE_RUNTIME_BASE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8181".to_string());
        validate_non_local_security_posture(
            environment,
            tee_attestation_required,
            tee_allow_insecure_dev_attestation,
            &tee_allowed_measurements,
            &kms_allowed_measurements,
            enclave_runtime_base_url.as_str(),
        )?;

        Ok(Self {
            bind_addr: env::var("ENCLAVE_RUNTIME_BIND_ADDR")
                .unwrap_or_else(|_| "127.0.0.1:8181".to_string()),
            environment,
            mode,
            runtime_id,
            measurement,
            database_url: require_env("DATABASE_URL")?,
            database_max_connections: parse_u32_env("DATABASE_MAX_CONNECTIONS", 5)?,
            data_encryption_key: require_env("DATA_ENCRYPTION_KEY")?,
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
            kms_allowed_measurements,
            enclave_runtime_base_url,
            oauth: GoogleEnclaveOauthConfig {
                client_id: require_env("GOOGLE_OAUTH_CLIENT_ID")?,
                client_secret: require_env("GOOGLE_OAUTH_CLIENT_SECRET")?,
                token_url: env::var("GOOGLE_OAUTH_TOKEN_URL")
                    .unwrap_or_else(|_| "https://oauth2.googleapis.com/token".to_string()),
                revoke_url: env::var("GOOGLE_OAUTH_REVOKE_URL")
                    .unwrap_or_else(|_| "https://oauth2.googleapis.com/revoke".to_string()),
            },
            enclave_rpc_auth: EnclaveRpcAuthConfig {
                shared_secret: parse_enclave_rpc_shared_secret(environment)?,
                max_clock_skew_seconds: enclave_rpc_auth_max_skew_seconds,
            },
            attestation_source,
            attestation_signing_private_key,
        })
    }

    pub(crate) fn attestation_document(&self) -> Result<Value, String> {
        if matches!(self.mode, EnclaveRuntimeMode::DevShim) {
            return Ok(json!({
                "runtime": self.runtime_id,
                "measurement": self.measurement,
                "issued_at": Utc::now().timestamp(),
                "signature": null,
                "dev_shim": true
            }));
        }

        let raw = match &self.attestation_source {
            AttestationSource::Inline(document) => document.to_string(),
            AttestationSource::FilePath(path) => std::fs::read_to_string(path)
                .map_err(|err| format!("failed to read attestation document: {err}"))?,
            AttestationSource::Missing => {
                return Err(
                    "attestation document is missing for remote enclave runtime mode".to_string(),
                );
            }
        };

        serde_json::from_str::<Value>(&raw)
            .map_err(|err| format!("failed to parse attestation document: {err}"))
    }

    pub(crate) fn attestation_challenge_response(
        &self,
        challenge: AttestationChallengeRequest,
    ) -> Result<AttestationChallengeResponse, String> {
        if challenge.challenge_nonce.trim().is_empty() {
            return Err("invalid challenge: challenge_nonce is required".to_string());
        }
        if challenge.request_id.trim().is_empty() {
            return Err("invalid challenge: request_id is required".to_string());
        }
        if challenge.operation_purpose.trim().is_empty() {
            return Err("invalid challenge: operation_purpose is required".to_string());
        }
        if challenge.expires_at <= challenge.issued_at {
            return Err("invalid challenge: expires_at must be greater than issued_at".to_string());
        }

        let now = Utc::now().timestamp();
        if now > challenge.expires_at {
            return Err("invalid challenge: challenge has expired".to_string());
        }

        let (runtime, measurement) = self.attestation_identity()?;
        let evidence_issued_at = now;

        let mut response = AttestationChallengeResponse {
            runtime,
            measurement,
            challenge_nonce: challenge.challenge_nonce,
            issued_at: challenge.issued_at,
            expires_at: challenge.expires_at,
            operation_purpose: challenge.operation_purpose,
            request_id: challenge.request_id,
            evidence_issued_at,
            signature: None,
        };

        let payload = attestation_signing_payload(&response);
        let signing_key = SigningKey::from_bytes(&self.attestation_signing_private_key);
        let signature = signing_key.sign(payload.as_bytes());
        response.signature =
            Some(base64::engine::general_purpose::STANDARD.encode(signature.to_bytes().as_ref()));

        Ok(response)
    }

    fn attestation_identity(&self) -> Result<(String, String), String> {
        if matches!(self.mode, EnclaveRuntimeMode::DevShim) {
            return Ok((self.runtime_id.clone(), self.measurement.clone()));
        }

        let document = self.attestation_document()?;
        let parsed: AttestationIdentityDocument = serde_json::from_value(document)
            .map_err(|err| format!("failed to parse attestation identity document: {err}"))?;

        if parsed.runtime.trim().is_empty() || parsed.measurement.trim().is_empty() {
            return Err(
                "attestation identity document requires runtime and measurement".to_string(),
            );
        }

        Ok((parsed.runtime, parsed.measurement))
    }
}

fn parse_enclave_rpc_shared_secret(environment: AlfredEnvironment) -> Result<String, String> {
    if let Some(secret) = optional_trimmed_env("ENCLAVE_RPC_SHARED_SECRET") {
        if secret.len() < 16 {
            return Err("ENCLAVE_RPC_SHARED_SECRET must be at least 16 characters".to_string());
        }
        return Ok(secret);
    }

    if matches!(environment, AlfredEnvironment::Local) {
        return Ok("local-dev-enclave-rpc-secret".to_string());
    }

    Err("ENCLAVE_RPC_SHARED_SECRET is required outside local env".to_string())
}

fn validate_non_local_security_posture(
    environment: AlfredEnvironment,
    tee_attestation_required: bool,
    tee_allow_insecure_dev_attestation: bool,
    tee_allowed_measurements: &[String],
    kms_allowed_measurements: &[String],
    enclave_runtime_base_url: &str,
) -> Result<(), String> {
    if matches!(environment, AlfredEnvironment::Local) {
        return Ok(());
    }

    if !tee_attestation_required {
        return Err("TEE_ATTESTATION_REQUIRED must be true outside local environment".to_string());
    }
    if tee_allow_insecure_dev_attestation {
        return Err(
            "TEE_ALLOW_INSECURE_DEV_ATTESTATION must be false outside local environment"
                .to_string(),
        );
    }

    validate_measurement_allowlist("TEE_ALLOWED_MEASUREMENTS", tee_allowed_measurements)?;
    validate_measurement_allowlist("KMS_ALLOWED_MEASUREMENTS", kms_allowed_measurements)?;
    validate_non_local_runtime_base_url(enclave_runtime_base_url)?;

    Ok(())
}

fn validate_measurement_allowlist(key: &str, measurements: &[String]) -> Result<(), String> {
    if measurements.is_empty() {
        return Err(format!(
            "{key} must contain at least one non-dev measurement outside local environment"
        ));
    }

    if measurements
        .iter()
        .any(|measurement| measurement == "dev-local-enclave")
    {
        return Err(format!(
            "{key} must not include dev-local-enclave outside local environment"
        ));
    }

    Ok(())
}

fn validate_non_local_runtime_base_url(base_url: &str) -> Result<(), String> {
    let parsed = reqwest::Url::parse(base_url)
        .map_err(|_| "ENCLAVE_RUNTIME_BASE_URL must be a valid URL".to_string())?;
    if parsed.scheme() == "https" {
        return Ok(());
    }

    if parsed.scheme() == "http"
        && matches!(
            parsed.host_str(),
            Some("127.0.0.1") | Some("localhost") | Some("::1")
        )
    {
        return Ok(());
    }

    Err(
        "ENCLAVE_RUNTIME_BASE_URL must use https outside local environment unless it is loopback http"
            .to_string(),
    )
}

fn decode_signing_key_bytes(encoded_key: &str) -> Result<[u8; 32], String> {
    let key_bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded_key.as_bytes())
        .map_err(|_| {
            "TEE_ATTESTATION_SIGNING_PRIVATE_KEY must be valid base64 for a 32-byte Ed25519 key"
                .to_string()
        })?;

    key_bytes.try_into().map_err(|_| {
        "TEE_ATTESTATION_SIGNING_PRIVATE_KEY must decode to exactly 32 bytes".to_string()
    })
}

fn require_env(key: &str) -> Result<String, String> {
    env::var(key).map_err(|_| format!("missing required env var {key}"))
}

fn parse_u32_env(key: &str, default: u32) -> Result<u32, String> {
    match env::var(key) {
        Ok(raw) => raw
            .parse::<u32>()
            .map_err(|_| format!("invalid integer in env var {key}")),
        Err(_) => Ok(default),
    }
}

fn parse_u64_env(key: &str, default: u64) -> Result<u64, String> {
    match env::var(key) {
        Ok(raw) => raw
            .parse::<u64>()
            .map_err(|_| format!("invalid integer in env var {key}")),
        Err(_) => Ok(default),
    }
}

fn parse_i32_env(key: &str, default: i32) -> Result<i32, String> {
    match env::var(key) {
        Ok(raw) => raw
            .parse::<i32>()
            .map_err(|_| format!("invalid integer in env var {key}")),
        Err(_) => Ok(default),
    }
}

fn parse_bool_env(key: &str, default: bool) -> Result<bool, String> {
    match env::var(key) {
        Ok(raw) => {
            let normalized = raw.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "true" | "1" | "yes" | "on" => Ok(true),
                "false" | "0" | "no" | "off" => Ok(false),
                _ => Err(format!("invalid boolean in env var {key}")),
            }
        }
        Err(_) => Ok(default),
    }
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

#[cfg(test)]
mod tests;
