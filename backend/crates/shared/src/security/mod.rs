mod attestation;
mod replay;

#[cfg(test)]
mod tests;

use chrono::Utc;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thiserror::Error;
use uuid::Uuid;

use crate::enclave_runtime::{AttestationChallengeRequest, AttestationChallengeResponse};

use replay::ReplayGuard;

#[derive(Debug, Clone)]
pub struct TeeAttestationPolicy {
    pub required: bool,
    pub expected_runtime: String,
    pub allowed_measurements: Vec<String>,
    pub attestation_public_key: Option<String>,
    pub max_attestation_age_seconds: u64,
    pub allow_insecure_dev_attestation: bool,
}

#[derive(Debug, Clone)]
pub struct KmsDecryptPolicy {
    pub key_id: String,
    pub key_version: i32,
    pub allowed_measurements: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ConnectorKeyMetadata {
    pub key_id: String,
    pub key_version: i32,
}

#[derive(Debug, Clone)]
pub struct AttestedIdentity {
    pub runtime: String,
    pub measurement: String,
}

#[derive(Debug, Clone)]
pub struct SecretRuntime {
    tee_policy: TeeAttestationPolicy,
    kms_policy: KmsDecryptPolicy,
    enclave_runtime_base_url: String,
    attestation_challenge_timeout_ms: u64,
    http_client: reqwest::Client,
    replay_guard: Arc<Mutex<ReplayGuard>>,
}

impl SecretRuntime {
    pub fn new(
        tee_policy: TeeAttestationPolicy,
        kms_policy: KmsDecryptPolicy,
        enclave_runtime_base_url: String,
        attestation_challenge_timeout_ms: u64,
        http_client: reqwest::Client,
    ) -> Self {
        Self {
            tee_policy,
            kms_policy,
            enclave_runtime_base_url,
            attestation_challenge_timeout_ms,
            http_client,
            replay_guard: Arc::new(Mutex::new(ReplayGuard::default())),
        }
    }

    pub fn kms_key_id(&self) -> &str {
        &self.kms_policy.key_id
    }

    pub fn kms_key_version(&self) -> i32 {
        self.kms_policy.key_version
    }

    pub async fn authorize_connector_decrypt(
        &self,
        key_metadata: &ConnectorKeyMetadata,
    ) -> Result<AttestedIdentity, SecurityError> {
        self.validate_key_binding(key_metadata)?;

        if !self.tee_policy.required {
            return Ok(AttestedIdentity {
                runtime: "unenforced".to_string(),
                measurement: "unenforced".to_string(),
            });
        }

        let challenge = self.build_attestation_challenge("decrypt");
        let challenge_response = self.request_attestation_challenge(&challenge).await?;
        let identity = self.verify_challenge_response(&challenge, &challenge_response)?;

        if !self
            .kms_policy
            .allowed_measurements
            .iter()
            .any(|measurement| measurement == &identity.measurement)
        {
            return Err(SecurityError::KmsPolicyDenied {
                measurement: identity.measurement,
            });
        }

        Ok(identity)
    }

    fn validate_key_binding(
        &self,
        key_metadata: &ConnectorKeyMetadata,
    ) -> Result<(), SecurityError> {
        if key_metadata.key_id != self.kms_policy.key_id {
            return Err(SecurityError::KmsKeyMismatch {
                expected: self.kms_policy.key_id.clone(),
                actual: key_metadata.key_id.clone(),
            });
        }

        if key_metadata.key_version != self.kms_policy.key_version {
            return Err(SecurityError::KmsVersionMismatch {
                expected: self.kms_policy.key_version,
                actual: key_metadata.key_version,
            });
        }

        Ok(())
    }

    fn build_attestation_challenge(&self, operation_purpose: &str) -> AttestationChallengeRequest {
        let now = Utc::now().timestamp();
        let max_age = self.tee_policy.max_attestation_age_seconds as i64;

        AttestationChallengeRequest {
            challenge_nonce: Uuid::new_v4().simple().to_string(),
            issued_at: now,
            expires_at: now + max_age,
            operation_purpose: operation_purpose.to_string(),
            request_id: Uuid::new_v4().to_string(),
        }
    }

    async fn request_attestation_challenge(
        &self,
        challenge: &AttestationChallengeRequest,
    ) -> Result<AttestationChallengeResponse, SecurityError> {
        let url = format!(
            "{}/v1/attestation/challenge",
            self.enclave_runtime_base_url.trim_end_matches('/')
        );

        let response = self
            .http_client
            .post(&url)
            .timeout(Duration::from_millis(self.attestation_challenge_timeout_ms))
            .json(challenge)
            .send()
            .await
            .map_err(|err| SecurityError::AttestationChallengeRequestFailed {
                message: err.to_string(),
            })?;

        if !response.status().is_success() {
            return Err(SecurityError::AttestationChallengeRejected {
                status: response.status().as_u16(),
            });
        }

        response
            .json::<AttestationChallengeResponse>()
            .await
            .map_err(|err| SecurityError::InvalidAttestationDocument(err.to_string()))
    }

    fn verify_challenge_response(
        &self,
        challenge: &AttestationChallengeRequest,
        response: &AttestationChallengeResponse,
    ) -> Result<AttestedIdentity, SecurityError> {
        if response.challenge_nonce != challenge.challenge_nonce {
            return Err(SecurityError::ChallengeNonceMismatch {
                expected: challenge.challenge_nonce.clone(),
                actual: response.challenge_nonce.clone(),
            });
        }

        if response.request_id != challenge.request_id {
            return Err(SecurityError::ChallengeRequestIdMismatch {
                expected: challenge.request_id.clone(),
                actual: response.request_id.clone(),
            });
        }

        if response.operation_purpose != challenge.operation_purpose {
            return Err(SecurityError::ChallengePurposeMismatch {
                expected: challenge.operation_purpose.clone(),
                actual: response.operation_purpose.clone(),
            });
        }

        if response.issued_at != challenge.issued_at || response.expires_at != challenge.expires_at
        {
            return Err(SecurityError::InvalidChallengeWindow {
                issued_at: response.issued_at,
                expires_at: response.expires_at,
            });
        }

        if response.expires_at <= response.issued_at {
            return Err(SecurityError::InvalidChallengeWindow {
                issued_at: response.issued_at,
                expires_at: response.expires_at,
            });
        }

        let now = Utc::now().timestamp();
        if now < response.issued_at || now > response.expires_at {
            return Err(SecurityError::ChallengeExpired {
                issued_at: response.issued_at,
                expires_at: response.expires_at,
                now,
            });
        }

        if response.evidence_issued_at < response.issued_at
            || response.evidence_issued_at > response.expires_at
        {
            return Err(SecurityError::EvidenceNotBoundToChallengeWindow {
                evidence_issued_at: response.evidence_issued_at,
                issued_at: response.issued_at,
                expires_at: response.expires_at,
            });
        }

        let max_age = self.tee_policy.max_attestation_age_seconds as i64;
        if response.evidence_issued_at < now - max_age
            || response.evidence_issued_at > now + max_age
        {
            return Err(SecurityError::StaleAttestation {
                issued_at: response.evidence_issued_at,
                now,
            });
        }

        if !response
            .runtime
            .eq_ignore_ascii_case(&self.tee_policy.expected_runtime)
        {
            return Err(SecurityError::RuntimeMismatch {
                expected: self.tee_policy.expected_runtime.clone(),
                actual: response.runtime.clone(),
            });
        }

        if !self
            .tee_policy
            .allowed_measurements
            .iter()
            .any(|measurement| measurement == &response.measurement)
        {
            return Err(SecurityError::MeasurementNotAllowed {
                measurement: response.measurement.clone(),
            });
        }

        if !self.tee_policy.allow_insecure_dev_attestation {
            let encoded_public_key = self
                .tee_policy
                .attestation_public_key
                .as_deref()
                .ok_or(SecurityError::MissingAttestationPublicKey)?;
            let signature = response
                .signature
                .as_deref()
                .ok_or(SecurityError::MissingAttestationSignature)?;
            attestation::verify_attestation_signature(encoded_public_key, signature, response)?;
        }

        let mut replay_guard = self
            .replay_guard
            .lock()
            .map_err(|_| SecurityError::ReplayGuardUnavailable)?;
        replay_guard
            .verify_and_record(response.challenge_nonce.as_str(), response.expires_at, now)
            .map_err(|_| SecurityError::ChallengeReplayDetected {
                challenge_nonce: response.challenge_nonce.clone(),
            })?;

        Ok(AttestedIdentity {
            runtime: response.runtime.clone(),
            measurement: response.measurement.clone(),
        })
    }
}

#[derive(Debug, Error)]
pub enum SecurityError {
    #[error("attestation document is invalid: {0}")]
    InvalidAttestationDocument(String),
    #[error("runtime mismatch for attestation: expected={expected}, actual={actual}")]
    RuntimeMismatch { expected: String, actual: String },
    #[error("attestation measurement is not allowed: {measurement}")]
    MeasurementNotAllowed { measurement: String },
    #[error("attestation timestamp is stale: issued_at={issued_at}, now={now}")]
    StaleAttestation { issued_at: i64, now: i64 },
    #[error("attestation public key is required when insecure mode is disabled")]
    MissingAttestationPublicKey,
    #[error("attestation signature is required when insecure mode is disabled")]
    MissingAttestationSignature,
    #[error("attestation public key is invalid")]
    InvalidAttestationPublicKey,
    #[error("attestation signature is invalid")]
    InvalidAttestationSignature,
    #[error("kms key mismatch: expected={expected}, actual={actual}")]
    KmsKeyMismatch { expected: String, actual: String },
    #[error("kms key version mismatch: expected={expected}, actual={actual}")]
    KmsVersionMismatch { expected: i32, actual: i32 },
    #[error("kms policy denied decrypt for measurement={measurement}")]
    KmsPolicyDenied { measurement: String },
    #[error("failed to request attestation challenge response: {message}")]
    AttestationChallengeRequestFailed { message: String },
    #[error("attestation challenge request rejected with status={status}")]
    AttestationChallengeRejected { status: u16 },
    #[error("attestation challenge nonce mismatch: expected={expected}, actual={actual}")]
    ChallengeNonceMismatch { expected: String, actual: String },
    #[error("attestation challenge request_id mismatch: expected={expected}, actual={actual}")]
    ChallengeRequestIdMismatch { expected: String, actual: String },
    #[error("attestation challenge purpose mismatch: expected={expected}, actual={actual}")]
    ChallengePurposeMismatch { expected: String, actual: String },
    #[error(
        "attestation challenge window is invalid: issued_at={issued_at}, expires_at={expires_at}"
    )]
    InvalidChallengeWindow { issued_at: i64, expires_at: i64 },
    #[error(
        "attestation challenge expired: issued_at={issued_at}, expires_at={expires_at}, now={now}"
    )]
    ChallengeExpired {
        issued_at: i64,
        expires_at: i64,
        now: i64,
    },
    #[error(
        "attestation evidence timestamp is outside challenge window: evidence_issued_at={evidence_issued_at}, issued_at={issued_at}, expires_at={expires_at}"
    )]
    EvidenceNotBoundToChallengeWindow {
        evidence_issued_at: i64,
        issued_at: i64,
        expires_at: i64,
    },
    #[error("attestation challenge replay detected for nonce={challenge_nonce}")]
    ChallengeReplayDetected { challenge_nonce: String },
    #[error("attestation replay guard unavailable")]
    ReplayGuardUnavailable,
}

#[cfg(test)]
pub(crate) fn build_attestation_signing_payload_for_tests(
    response: &AttestationChallengeResponse,
) -> String {
    crate::enclave_runtime::attestation_signing_payload(response)
}
