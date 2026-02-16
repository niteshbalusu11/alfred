use base64::Engine as _;
use chrono::Utc;
use ed25519_dalek::{Signer, SigningKey};

use crate::enclave_runtime::{AttestationChallengeRequest, AttestationChallengeResponse};

use super::{
    ConnectorKeyMetadata, KmsDecryptPolicy, SecretRuntime, SecurityError, TeeAttestationPolicy,
    build_attestation_signing_payload_for_tests,
};

fn signing_key() -> SigningKey {
    SigningKey::from_bytes(&[7_u8; 32])
}

fn runtime() -> (SecretRuntime, SigningKey) {
    let signing_key = signing_key();
    let public_key_b64 =
        base64::engine::general_purpose::STANDARD.encode(signing_key.verifying_key().as_bytes());

    (
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
            "http://127.0.0.1:8181".to_string(),
            2000,
            reqwest::Client::new(),
        ),
        signing_key,
    )
}

fn challenge() -> AttestationChallengeRequest {
    let now = Utc::now().timestamp();
    AttestationChallengeRequest {
        challenge_nonce: "nonce-123".to_string(),
        issued_at: now - 5,
        expires_at: now + 60,
        operation_purpose: "decrypt".to_string(),
        request_id: "req-123".to_string(),
    }
}

fn signed_response(
    challenge: &AttestationChallengeRequest,
    signing_key: &SigningKey,
    runtime: &str,
    measurement: &str,
    evidence_issued_at: i64,
) -> AttestationChallengeResponse {
    let mut response = AttestationChallengeResponse {
        runtime: runtime.to_string(),
        measurement: measurement.to_string(),
        challenge_nonce: challenge.challenge_nonce.clone(),
        issued_at: challenge.issued_at,
        expires_at: challenge.expires_at,
        operation_purpose: challenge.operation_purpose.clone(),
        request_id: challenge.request_id.clone(),
        evidence_issued_at,
        signature: None,
    };

    let payload = build_attestation_signing_payload_for_tests(&response);
    let signature = signing_key.sign(payload.as_bytes());
    response.signature =
        Some(base64::engine::general_purpose::STANDARD.encode(signature.to_bytes()));
    response
}

#[test]
fn validate_key_binding_denies_key_mismatch() {
    let (runtime, _) = runtime();

    let err = runtime
        .validate_key_binding(&ConnectorKeyMetadata {
            key_id: "kms/other".to_string(),
            key_version: 7,
        })
        .expect_err("key mismatch must fail");

    assert!(matches!(err, SecurityError::KmsKeyMismatch { .. }));
}

#[test]
fn validate_key_binding_denies_key_version_mismatch() {
    let (runtime, _) = runtime();

    let err = runtime
        .validate_key_binding(&ConnectorKeyMetadata {
            key_id: "kms/alfred/token".to_string(),
            key_version: 3,
        })
        .expect_err("key version mismatch must fail");

    assert!(matches!(err, SecurityError::KmsVersionMismatch { .. }));
}

#[test]
fn verify_challenge_response_allows_valid_signed_attestation() {
    let (runtime, signing_key) = runtime();
    let challenge = challenge();
    let response = signed_response(
        &challenge,
        &signing_key,
        "nitro",
        "mr_enclave_1",
        Utc::now().timestamp(),
    );

    let identity = runtime
        .verify_challenge_response(&challenge, &response)
        .expect("valid challenge response should pass");

    assert_eq!(identity.runtime, "nitro");
    assert_eq!(identity.measurement, "mr_enclave_1");
}

#[test]
fn verify_challenge_response_denies_runtime_mismatch() {
    let (runtime, signing_key) = runtime();
    let challenge = challenge();
    let response = signed_response(
        &challenge,
        &signing_key,
        "other-runtime",
        "mr_enclave_1",
        Utc::now().timestamp(),
    );

    let err = runtime
        .verify_challenge_response(&challenge, &response)
        .expect_err("runtime mismatch must fail");

    assert!(matches!(err, SecurityError::RuntimeMismatch { .. }));
}

#[test]
fn verify_challenge_response_denies_measurement_mismatch() {
    let (runtime, signing_key) = runtime();
    let challenge = challenge();
    let response = signed_response(
        &challenge,
        &signing_key,
        "nitro",
        "mr_enclave_2",
        Utc::now().timestamp(),
    );

    let err = runtime
        .verify_challenge_response(&challenge, &response)
        .expect_err("measurement mismatch must fail");

    assert!(matches!(err, SecurityError::MeasurementNotAllowed { .. }));
}

#[test]
fn verify_challenge_response_denies_stale_evidence_timestamp() {
    let (mut runtime, signing_key) = runtime();
    runtime.tee_policy.max_attestation_age_seconds = 5;
    let mut challenge = challenge();
    challenge.issued_at = Utc::now().timestamp() - 10;
    challenge.expires_at = Utc::now().timestamp() + 60;
    let response = signed_response(
        &challenge,
        &signing_key,
        "nitro",
        "mr_enclave_1",
        Utc::now().timestamp() - 8,
    );

    let err = runtime
        .verify_challenge_response(&challenge, &response)
        .expect_err("stale evidence must fail");

    assert!(matches!(err, SecurityError::StaleAttestation { .. }));
}

#[test]
fn verify_challenge_response_denies_evidence_outside_challenge_window() {
    let (runtime, signing_key) = runtime();
    let challenge = challenge();
    let response = signed_response(
        &challenge,
        &signing_key,
        "nitro",
        "mr_enclave_1",
        challenge.expires_at + 5,
    );

    let err = runtime
        .verify_challenge_response(&challenge, &response)
        .expect_err("evidence outside challenge window must fail");

    assert!(matches!(
        err,
        SecurityError::EvidenceNotBoundToChallengeWindow { .. }
    ));
}

#[test]
fn verify_challenge_response_denies_expired_challenge() {
    let (runtime, signing_key) = runtime();
    let now = Utc::now().timestamp();
    let challenge = AttestationChallengeRequest {
        challenge_nonce: "nonce-expired".to_string(),
        issued_at: now - 90,
        expires_at: now - 5,
        operation_purpose: "decrypt".to_string(),
        request_id: "req-expired".to_string(),
    };
    let response = signed_response(&challenge, &signing_key, "nitro", "mr_enclave_1", now - 10);

    let err = runtime
        .verify_challenge_response(&challenge, &response)
        .expect_err("expired challenge must fail");

    assert!(matches!(err, SecurityError::ChallengeExpired { .. }));
}

#[test]
fn verify_challenge_response_denies_nonce_replay() {
    let (runtime, signing_key) = runtime();
    let challenge = challenge();
    let response = signed_response(
        &challenge,
        &signing_key,
        "nitro",
        "mr_enclave_1",
        Utc::now().timestamp(),
    );

    runtime
        .verify_challenge_response(&challenge, &response)
        .expect("first attempt should succeed");

    let err = runtime
        .verify_challenge_response(&challenge, &response)
        .expect_err("replayed nonce must fail");

    assert!(matches!(err, SecurityError::ChallengeReplayDetected { .. }));
}

#[test]
fn verify_challenge_response_denies_signature_mismatch() {
    let (runtime, signing_key) = runtime();
    let challenge = challenge();
    let mut response = signed_response(
        &challenge,
        &signing_key,
        "nitro",
        "mr_enclave_1",
        Utc::now().timestamp(),
    );
    response.operation_purpose = "fetch".to_string();

    let err = runtime
        .verify_challenge_response(&challenge, &response)
        .expect_err("tampered payload must fail signature verification");

    assert!(matches!(
        err,
        SecurityError::ChallengePurposeMismatch { .. } | SecurityError::InvalidAttestationSignature
    ));
}
