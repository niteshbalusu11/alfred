use std::env;
use std::path::PathBuf;

use base64::Engine as _;
use chrono::Utc;
use ed25519_dalek::{Signer, SigningKey};
use serde::Deserialize;
use serde_json::{Value, json};
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

        Ok(Self {
            bind_addr: env::var("ENCLAVE_RUNTIME_BIND_ADDR")
                .unwrap_or_else(|_| "127.0.0.1:8181".to_string()),
            environment,
            mode,
            runtime_id,
            measurement,
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

#[cfg(test)]
mod tests {
    use shared::enclave_runtime::{
        AlfredEnvironment, AttestationChallengeRequest, EnclaveRuntimeMode,
    };

    use super::{AttestationSource, RuntimeConfig};

    #[test]
    fn dev_shim_attestation_document_is_generated() {
        let config = RuntimeConfig {
            bind_addr: "127.0.0.1:8181".to_string(),
            environment: AlfredEnvironment::Local,
            mode: EnclaveRuntimeMode::DevShim,
            runtime_id: "nitro".to_string(),
            measurement: "dev-local-enclave".to_string(),
            attestation_source: AttestationSource::Missing,
            attestation_signing_private_key: [7_u8; 32],
        };

        let document = config
            .attestation_document()
            .expect("dev-shim document should be generated");
        assert_eq!(document["runtime"], "nitro");
        assert_eq!(document["measurement"], "dev-local-enclave");
        assert_eq!(document["dev_shim"], true);
    }

    #[test]
    fn remote_attestation_document_fails_when_source_missing() {
        let config = RuntimeConfig {
            bind_addr: "127.0.0.1:8181".to_string(),
            environment: AlfredEnvironment::Local,
            mode: EnclaveRuntimeMode::Remote,
            runtime_id: "nitro".to_string(),
            measurement: "mr_enclave_1".to_string(),
            attestation_source: AttestationSource::Missing,
            attestation_signing_private_key: [7_u8; 32],
        };

        let err = config
            .attestation_document()
            .expect_err("missing source should fail");
        assert!(
            err.contains("attestation document is missing"),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn challenge_response_is_signed_and_echoes_challenge_fields() {
        let config = RuntimeConfig {
            bind_addr: "127.0.0.1:8181".to_string(),
            environment: AlfredEnvironment::Local,
            mode: EnclaveRuntimeMode::DevShim,
            runtime_id: "nitro".to_string(),
            measurement: "dev-local-enclave".to_string(),
            attestation_source: AttestationSource::Missing,
            attestation_signing_private_key: [7_u8; 32],
        };

        let challenge = AttestationChallengeRequest {
            challenge_nonce: "nonce-1".to_string(),
            issued_at: chrono::Utc::now().timestamp() - 2,
            expires_at: chrono::Utc::now().timestamp() + 30,
            operation_purpose: "decrypt".to_string(),
            request_id: "req-1".to_string(),
        };

        let response = config
            .attestation_challenge_response(challenge)
            .expect("challenge should succeed");

        assert_eq!(response.challenge_nonce, "nonce-1");
        assert_eq!(response.operation_purpose, "decrypt");
        assert_eq!(response.request_id, "req-1");
        assert!(response.signature.is_some());
    }
}
