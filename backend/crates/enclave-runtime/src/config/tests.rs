use shared::enclave_runtime::{AlfredEnvironment, AttestationChallengeRequest, EnclaveRuntimeMode};

use super::{AttestationSource, RuntimeConfig};

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
        attestation_source: AttestationSource::Missing,
        attestation_signing_private_key: [7_u8; 32],
    }
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
