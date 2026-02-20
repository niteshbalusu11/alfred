use shared::assistant_crypto::{
    ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305, AssistantIngressKeyMaterial,
    AssistantIngressKeyring, derive_public_key_b64,
};
use shared::enclave_runtime::AssistantAttestedKeyChallengeRequest;
use shared::enclave_runtime::{AlfredEnvironment, AttestationChallengeRequest, EnclaveRuntimeMode};

use super::{
    AttestationSource, DEFAULT_ASSISTANT_INGRESS_SESSION_TTL_SECONDS, RuntimeConfig,
    validate_non_local_runtime_base_url, validate_non_local_security_posture,
};

fn build_config(mode: EnclaveRuntimeMode) -> RuntimeConfig {
    RuntimeConfig {
        bind_addr: "127.0.0.1:8181".to_string(),
        environment: AlfredEnvironment::Local,
        mode,
        runtime_id: "nitro".to_string(),
        measurement: "dev-local-enclave".to_string(),
        database_url: "postgres://localhost/alfred".to_string(),
        database_max_connections: 5,
        data_encryption_key: "01234567890123456789012345678901".to_string(),
        tee_attestation_required: false,
        tee_expected_runtime: "nitro".to_string(),
        tee_allowed_measurements: vec!["dev-local-enclave".to_string()],
        tee_attestation_public_key: None,
        tee_attestation_max_age_seconds: 300,
        tee_attestation_challenge_timeout_ms: 2000,
        tee_allow_insecure_dev_attestation: true,
        kms_key_id: "kms/local/alfred-refresh-token".to_string(),
        kms_key_version: 1,
        kms_allowed_measurements: vec!["dev-local-enclave".to_string()],
        enclave_runtime_base_url: "http://127.0.0.1:8181".to_string(),
        oauth: shared::enclave::GoogleEnclaveOauthConfig {
            client_id: "client-id".to_string(),
            client_secret: "client-secret".to_string(),
            token_url: "https://oauth2.googleapis.com/token".to_string(),
            revoke_url: "https://oauth2.googleapis.com/revoke".to_string(),
        },
        enclave_rpc_auth: shared::enclave::EnclaveRpcAuthConfig {
            shared_secret: "local-dev-enclave-rpc-secret".to_string(),
            max_clock_skew_seconds: 30,
        },
        assistant_ingress_keys: AssistantIngressKeyring {
            active: AssistantIngressKeyMaterial {
                key_id: "assistant-ingress-v1".to_string(),
                private_key: [11_u8; 32],
                public_key: derive_public_key_b64([11_u8; 32]),
                key_expires_at: chrono::Utc::now().timestamp() + 900,
            },
            previous: None,
        },
        assistant_ingress_key_ttl_seconds: 900,
        assistant_session_ttl_seconds: DEFAULT_ASSISTANT_INGRESS_SESSION_TTL_SECONDS,
        attestation_source: AttestationSource::Missing,
        attestation_signing_private_key: [7_u8; 32],
    }
}

#[test]
fn assistant_session_ttl_default_targets_sixty_days() {
    assert_eq!(
        DEFAULT_ASSISTANT_INGRESS_SESSION_TTL_SECONDS,
        60 * 24 * 60 * 60
    );
}

#[test]
fn dev_shim_attestation_document_is_generated() {
    let config = build_config(EnclaveRuntimeMode::DevShim);

    let document = config
        .attestation_document()
        .expect("dev-shim document should be generated");
    assert_eq!(document["runtime"], "nitro");
    assert_eq!(document["measurement"], "dev-local-enclave");
    assert_eq!(document["dev_shim"], true);
}

#[test]
fn remote_attestation_document_fails_when_source_missing() {
    let config = build_config(EnclaveRuntimeMode::Remote);

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
    let config = build_config(EnclaveRuntimeMode::DevShim);

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

#[test]
fn assistant_attested_key_response_is_signed_and_binds_key_fields() {
    let mut config = build_config(EnclaveRuntimeMode::DevShim);
    config.assistant_ingress_keys.active.key_expires_at = chrono::Utc::now().timestamp() - 10;
    let challenge = AssistantAttestedKeyChallengeRequest {
        challenge_nonce: "nonce-key-1".to_string(),
        issued_at: chrono::Utc::now().timestamp() - 2,
        expires_at: chrono::Utc::now().timestamp() + 30,
        request_id: "req-key-1".to_string(),
    };

    let response = config
        .assistant_attested_key_challenge_response(challenge)
        .expect("assistant key challenge should succeed");

    assert_eq!(response.challenge_nonce, "nonce-key-1");
    assert_eq!(response.request_id, "req-key-1");
    assert_eq!(
        response.algorithm,
        ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305
    );
    assert_eq!(response.key_id, "assistant-ingress-v1");
    assert!(response.key_expires_at > chrono::Utc::now().timestamp());
    assert!(response.signature.is_some());
}

#[test]
fn non_local_security_posture_rejects_insecure_attestation_flags() {
    let err = validate_non_local_security_posture(
        AlfredEnvironment::Production,
        false,
        true,
        &["mr-enclave-prod-a".to_string()],
        &["mr-enclave-prod-a".to_string()],
        "https://enclave.internal:8181",
    )
    .expect_err("insecure non-local posture should fail");

    assert!(err.contains("TEE_ATTESTATION_REQUIRED") || err.contains("TEE_ALLOW_INSECURE"));
}

#[test]
fn non_local_security_posture_rejects_dev_measurement() {
    let err = validate_non_local_security_posture(
        AlfredEnvironment::Staging,
        true,
        false,
        &["dev-local-enclave".to_string()],
        &["mr-enclave-stage-a".to_string()],
        "https://enclave.internal:8181",
    )
    .expect_err("dev measurement should fail outside local");

    assert!(err.contains("TEE_ALLOWED_MEASUREMENTS"));
}

#[test]
fn non_local_runtime_base_url_allows_https_or_loopback_http() {
    validate_non_local_runtime_base_url("https://enclave.internal:8181")
        .expect("https URL should pass");
    validate_non_local_runtime_base_url("http://127.0.0.1:8181")
        .expect("loopback http URL should pass");
}
