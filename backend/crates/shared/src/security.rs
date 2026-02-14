use base64::Engine as _;
use chrono::Utc;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::Deserialize;
use std::path::PathBuf;
use thiserror::Error;

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
    attestation_source: AttestationDocumentSource,
}

#[derive(Debug, Clone)]
enum AttestationDocumentSource {
    Inline(String),
    FilePath(PathBuf),
    Missing,
}

impl SecretRuntime {
    pub fn new(
        tee_policy: TeeAttestationPolicy,
        kms_policy: KmsDecryptPolicy,
        attestation_document: Option<String>,
        attestation_document_path: Option<PathBuf>,
    ) -> Self {
        let attestation_source = if let Some(attestation_document_path) = attestation_document_path
        {
            AttestationDocumentSource::FilePath(attestation_document_path)
        } else if let Some(attestation_document) = attestation_document {
            AttestationDocumentSource::Inline(attestation_document)
        } else {
            AttestationDocumentSource::Missing
        };

        Self {
            tee_policy,
            kms_policy,
            attestation_source,
        }
    }

    pub fn kms_key_id(&self) -> &str {
        &self.kms_policy.key_id
    }

    pub fn kms_key_version(&self) -> i32 {
        self.kms_policy.key_version
    }

    pub fn authorize_connector_decrypt(
        &self,
        key_metadata: &ConnectorKeyMetadata,
    ) -> Result<AttestedIdentity, SecurityError> {
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

        let attestation_document = self.load_attestation_document()?;
        let identity = verify_attestation(&self.tee_policy, attestation_document.as_str())?;

        if self.tee_policy.required
            && !self
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

    fn load_attestation_document(&self) -> Result<String, SecurityError> {
        match &self.attestation_source {
            AttestationDocumentSource::Inline(attestation_document) => {
                Ok(attestation_document.clone())
            }
            AttestationDocumentSource::FilePath(path) => std::fs::read_to_string(path)
                .map_err(|err| SecurityError::AttestationDocumentReadFailed(err.to_string())),
            AttestationDocumentSource::Missing => Err(SecurityError::MissingAttestationDocument),
        }
    }
}

#[derive(Debug, Deserialize)]
struct AttestationDocument {
    runtime: String,
    measurement: String,
    issued_at: i64,
    signature: Option<String>,
}

fn verify_attestation(
    policy: &TeeAttestationPolicy,
    attestation_document: &str,
) -> Result<AttestedIdentity, SecurityError> {
    if !policy.required {
        return Ok(AttestedIdentity {
            runtime: "unenforced".to_string(),
            measurement: "unenforced".to_string(),
        });
    }

    let parsed: AttestationDocument = serde_json::from_str(attestation_document)
        .map_err(|err| SecurityError::InvalidAttestationDocument(err.to_string()))?;

    if !parsed
        .runtime
        .eq_ignore_ascii_case(&policy.expected_runtime)
    {
        return Err(SecurityError::RuntimeMismatch {
            expected: policy.expected_runtime.clone(),
            actual: parsed.runtime,
        });
    }

    if !policy
        .allowed_measurements
        .iter()
        .any(|measurement| measurement == &parsed.measurement)
    {
        return Err(SecurityError::MeasurementNotAllowed {
            measurement: parsed.measurement,
        });
    }

    let now = Utc::now().timestamp();
    let max_age = policy.max_attestation_age_seconds as i64;
    if parsed.issued_at < now - max_age || parsed.issued_at > now + max_age {
        return Err(SecurityError::StaleAttestation {
            issued_at: parsed.issued_at,
            now,
        });
    }

    if !policy.allow_insecure_dev_attestation {
        let encoded_public_key = policy
            .attestation_public_key
            .as_deref()
            .ok_or(SecurityError::MissingAttestationPublicKey)?;
        verify_attestation_signature(
            encoded_public_key,
            parsed
                .signature
                .as_deref()
                .ok_or(SecurityError::MissingAttestationSignature)?,
            &parsed.runtime,
            &parsed.measurement,
            parsed.issued_at,
        )?;
    }

    Ok(AttestedIdentity {
        runtime: parsed.runtime,
        measurement: parsed.measurement,
    })
}

fn verify_attestation_signature(
    encoded_public_key: &str,
    encoded_signature: &str,
    runtime: &str,
    measurement: &str,
    issued_at: i64,
) -> Result<(), SecurityError> {
    let public_key_bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded_public_key.as_bytes())
        .map_err(|_| SecurityError::InvalidAttestationPublicKey)?;
    let public_key_bytes: [u8; 32] = public_key_bytes
        .try_into()
        .map_err(|_| SecurityError::InvalidAttestationPublicKey)?;
    let public_key = VerifyingKey::from_bytes(&public_key_bytes)
        .map_err(|_| SecurityError::InvalidAttestationPublicKey)?;

    let signature_bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded_signature.as_bytes())
        .map_err(|_| SecurityError::InvalidAttestationSignature)?;
    let signature_bytes: [u8; 64] = signature_bytes
        .try_into()
        .map_err(|_| SecurityError::InvalidAttestationSignature)?;
    let signature = Signature::try_from(&signature_bytes[..])
        .map_err(|_| SecurityError::InvalidAttestationSignature)?;

    public_key
        .verify(
            attestation_signing_payload(runtime, measurement, issued_at).as_bytes(),
            &signature,
        )
        .map_err(|_| SecurityError::InvalidAttestationSignature)?;

    Ok(())
}

fn attestation_signing_payload(runtime: &str, measurement: &str, issued_at: i64) -> String {
    format!("{runtime}|{measurement}|{issued_at}")
}

#[derive(Debug, Error)]
pub enum SecurityError {
    #[error("attestation document is invalid: {0}")]
    InvalidAttestationDocument(String),
    #[error("attestation document is missing")]
    MissingAttestationDocument,
    #[error("failed to read attestation document: {0}")]
    AttestationDocumentReadFailed(String),
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
}

#[cfg(test)]
mod tests {
    use base64::Engine as _;
    use chrono::Utc;
    use ed25519_dalek::{Signer, SigningKey};

    use super::{
        ConnectorKeyMetadata, KmsDecryptPolicy, SecretRuntime, SecurityError, TeeAttestationPolicy,
    };

    fn signing_key() -> SigningKey {
        SigningKey::from_bytes(&[7_u8; 32])
    }

    fn runtime() -> SecretRuntime {
        let signing_key = signing_key();
        let verifying_key = signing_key.verifying_key();
        let public_key_b64 =
            base64::engine::general_purpose::STANDARD.encode(verifying_key.as_bytes());
        let issued_at = Utc::now().timestamp();

        let payload = format!("{}|{}|{}", "nitro", "mr_enclave_1", issued_at);
        let signature = signing_key.sign(payload.as_bytes());
        let signature_b64 = base64::engine::general_purpose::STANDARD.encode(signature.to_bytes());

        SecretRuntime::new(
            TeeAttestationPolicy {
                required: true,
                expected_runtime: "nitro".to_string(),
                allowed_measurements: vec!["mr_enclave_1".to_string()],
                attestation_public_key: Some(public_key_b64),
                max_attestation_age_seconds: 300,
                allow_insecure_dev_attestation: false,
            },
            KmsDecryptPolicy {
                key_id: "kms/alfred/token".to_string(),
                key_version: 7,
                allowed_measurements: vec!["mr_enclave_1".to_string()],
            },
            Some(format!(
                "{{\"runtime\":\"nitro\",\"measurement\":\"mr_enclave_1\",\"issued_at\":{issued_at},\"signature\":\"{signature_b64}\"}}"
            )),
            None,
        )
    }

    #[test]
    fn authorize_connector_decrypt_allows_valid_attestation_and_key_binding() {
        let runtime = runtime();
        let key_metadata = ConnectorKeyMetadata {
            key_id: "kms/alfred/token".to_string(),
            key_version: 7,
        };

        let identity = runtime
            .authorize_connector_decrypt(&key_metadata)
            .expect("decrypt should be authorized");

        assert_eq!(identity.runtime, "nitro");
        assert_eq!(identity.measurement, "mr_enclave_1");
    }

    #[test]
    fn authorize_connector_decrypt_denies_key_mismatch() {
        let runtime = runtime();
        let key_metadata = ConnectorKeyMetadata {
            key_id: "kms/other".to_string(),
            key_version: 7,
        };

        let err = runtime
            .authorize_connector_decrypt(&key_metadata)
            .expect_err("decrypt should be denied");

        assert!(matches!(err, SecurityError::KmsKeyMismatch { .. }));
    }

    #[test]
    fn authorize_connector_decrypt_denies_key_version_mismatch() {
        let runtime = runtime();
        let key_metadata = ConnectorKeyMetadata {
            key_id: "kms/alfred/token".to_string(),
            key_version: 3,
        };

        let err = runtime
            .authorize_connector_decrypt(&key_metadata)
            .expect_err("decrypt should be denied");

        assert!(matches!(err, SecurityError::KmsVersionMismatch { .. }));
    }

    #[test]
    fn authorize_connector_decrypt_denies_bad_runtime() {
        let signing_key = signing_key();
        let verifying_key = signing_key.verifying_key();
        let public_key_b64 =
            base64::engine::general_purpose::STANDARD.encode(verifying_key.as_bytes());
        let issued_at = Utc::now().timestamp();
        let payload = format!("{}|{}|{}", "other", "mr_enclave_1", issued_at);
        let signature = signing_key.sign(payload.as_bytes());
        let signature_b64 = base64::engine::general_purpose::STANDARD.encode(signature.to_bytes());

        let runtime = SecretRuntime::new(
            TeeAttestationPolicy {
                required: true,
                expected_runtime: "nitro".to_string(),
                allowed_measurements: vec!["mr_enclave_1".to_string()],
                attestation_public_key: Some(public_key_b64),
                max_attestation_age_seconds: 300,
                allow_insecure_dev_attestation: false,
            },
            KmsDecryptPolicy {
                key_id: "kms/alfred/token".to_string(),
                key_version: 7,
                allowed_measurements: vec!["mr_enclave_1".to_string()],
            },
            Some(format!(
                "{{\"runtime\":\"other\",\"measurement\":\"mr_enclave_1\",\"issued_at\":{issued_at},\"signature\":\"{signature_b64}\"}}"
            )),
            None,
        );

        let err = runtime
            .authorize_connector_decrypt(&ConnectorKeyMetadata {
                key_id: "kms/alfred/token".to_string(),
                key_version: 7,
            })
            .expect_err("decrypt should be denied");

        assert!(matches!(err, SecurityError::RuntimeMismatch { .. }));
    }

    #[test]
    fn authorize_connector_decrypt_denies_disallowed_measurement() {
        let signing_key = signing_key();
        let verifying_key = signing_key.verifying_key();
        let public_key_b64 =
            base64::engine::general_purpose::STANDARD.encode(verifying_key.as_bytes());
        let issued_at = Utc::now().timestamp();
        let payload = format!("{}|{}|{}", "nitro", "mr_enclave_2", issued_at);
        let signature = signing_key.sign(payload.as_bytes());
        let signature_b64 = base64::engine::general_purpose::STANDARD.encode(signature.to_bytes());

        let runtime = SecretRuntime::new(
            TeeAttestationPolicy {
                required: true,
                expected_runtime: "nitro".to_string(),
                allowed_measurements: vec!["mr_enclave_1".to_string()],
                attestation_public_key: Some(public_key_b64),
                max_attestation_age_seconds: 300,
                allow_insecure_dev_attestation: false,
            },
            KmsDecryptPolicy {
                key_id: "kms/alfred/token".to_string(),
                key_version: 7,
                allowed_measurements: vec!["mr_enclave_1".to_string()],
            },
            Some(format!(
                "{{\"runtime\":\"nitro\",\"measurement\":\"mr_enclave_2\",\"issued_at\":{issued_at},\"signature\":\"{signature_b64}\"}}"
            )),
            None,
        );

        let err = runtime
            .authorize_connector_decrypt(&ConnectorKeyMetadata {
                key_id: "kms/alfred/token".to_string(),
                key_version: 7,
            })
            .expect_err("decrypt should be denied");

        assert!(matches!(
            err,
            SecurityError::MeasurementNotAllowed { .. } | SecurityError::KmsPolicyDenied { .. }
        ));
    }

    #[test]
    fn authorize_connector_decrypt_denies_stale_attestation() {
        let signing_key = signing_key();
        let verifying_key = signing_key.verifying_key();
        let public_key_b64 =
            base64::engine::general_purpose::STANDARD.encode(verifying_key.as_bytes());
        let issued_at = Utc::now().timestamp() - 1200;
        let payload = format!("{}|{}|{}", "nitro", "mr_enclave_1", issued_at);
        let signature = signing_key.sign(payload.as_bytes());
        let signature_b64 = base64::engine::general_purpose::STANDARD.encode(signature.to_bytes());

        let runtime = SecretRuntime::new(
            TeeAttestationPolicy {
                required: true,
                expected_runtime: "nitro".to_string(),
                allowed_measurements: vec!["mr_enclave_1".to_string()],
                attestation_public_key: Some(public_key_b64),
                max_attestation_age_seconds: 300,
                allow_insecure_dev_attestation: false,
            },
            KmsDecryptPolicy {
                key_id: "kms/alfred/token".to_string(),
                key_version: 7,
                allowed_measurements: vec!["mr_enclave_1".to_string()],
            },
            Some(format!(
                "{{\"runtime\":\"nitro\",\"measurement\":\"mr_enclave_1\",\"issued_at\":{issued_at},\"signature\":\"{signature_b64}\"}}"
            )),
            None,
        );

        let err = runtime
            .authorize_connector_decrypt(&ConnectorKeyMetadata {
                key_id: "kms/alfred/token".to_string(),
                key_version: 7,
            })
            .expect_err("decrypt should be denied");

        assert!(matches!(err, SecurityError::StaleAttestation { .. }));
    }
}
