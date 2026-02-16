use std::collections::HashMap;
use std::sync::Mutex;

use axum::http::{HeaderMap, HeaderValue, StatusCode};
use chrono::Utc;
use shared::enclave::{
    ENCLAVE_RPC_AUTH_NONCE_HEADER, ENCLAVE_RPC_AUTH_SIGNATURE_HEADER,
    ENCLAVE_RPC_AUTH_TIMESTAMP_HEADER, ENCLAVE_RPC_CONTRACT_VERSION,
    ENCLAVE_RPC_CONTRACT_VERSION_HEADER, ENCLAVE_RPC_PATH_EXCHANGE_GOOGLE_TOKEN,
    EnclaveRpcAuthConfig, sign_rpc_request,
};

use super::rpc::authorize_request;

fn signed_headers(
    auth: &EnclaveRpcAuthConfig,
    path: &str,
    body: &[u8],
    timestamp: i64,
    nonce: &str,
) -> HeaderMap {
    let signature = sign_rpc_request(&auth.shared_secret, "POST", path, timestamp, nonce, body);

    let mut headers = HeaderMap::new();
    headers.insert(
        ENCLAVE_RPC_CONTRACT_VERSION_HEADER,
        HeaderValue::from_static(ENCLAVE_RPC_CONTRACT_VERSION),
    );
    headers.insert(
        ENCLAVE_RPC_AUTH_TIMESTAMP_HEADER,
        HeaderValue::from_str(&timestamp.to_string()).expect("timestamp header should parse"),
    );
    headers.insert(
        ENCLAVE_RPC_AUTH_NONCE_HEADER,
        HeaderValue::from_str(nonce).expect("nonce header should parse"),
    );
    headers.insert(
        ENCLAVE_RPC_AUTH_SIGNATURE_HEADER,
        HeaderValue::from_str(signature.as_str()).expect("signature header should parse"),
    );

    headers
}

fn default_auth() -> EnclaveRpcAuthConfig {
    EnclaveRpcAuthConfig {
        shared_secret: "unit-test-shared-secret-123".to_string(),
        max_clock_skew_seconds: 30,
    }
}

#[test]
fn authorize_request_allows_valid_signed_request() {
    let auth = default_auth();
    let body = br#"{"request_id":"req-1"}"#;
    let nonce = "rpc-nonce-1";
    let timestamp = Utc::now().timestamp();
    let headers = signed_headers(
        &auth,
        ENCLAVE_RPC_PATH_EXCHANGE_GOOGLE_TOKEN,
        body,
        timestamp,
        nonce,
    );
    let replay_guard = Mutex::new(HashMap::new());

    let result = authorize_request(
        &auth,
        &replay_guard,
        &headers,
        ENCLAVE_RPC_PATH_EXCHANGE_GOOGLE_TOKEN,
        body,
    );
    assert!(result.is_ok(), "valid RPC auth request should pass");
}

#[test]
fn authorize_request_rejects_missing_signature_header() {
    let auth = default_auth();
    let body = br#"{"request_id":"req-1"}"#;
    let nonce = "rpc-nonce-2";
    let timestamp = Utc::now().timestamp();
    let mut headers = signed_headers(
        &auth,
        ENCLAVE_RPC_PATH_EXCHANGE_GOOGLE_TOKEN,
        body,
        timestamp,
        nonce,
    );
    headers.remove(ENCLAVE_RPC_AUTH_SIGNATURE_HEADER);
    let replay_guard = Mutex::new(HashMap::new());

    let err = authorize_request(
        &auth,
        &replay_guard,
        &headers,
        ENCLAVE_RPC_PATH_EXCHANGE_GOOGLE_TOKEN,
        body,
    )
    .expect_err("missing auth signature header must fail");

    assert_eq!(err.status, StatusCode::UNAUTHORIZED);
    assert_eq!(err.body.error.code, "missing_request_header");
}

#[test]
fn authorize_request_rejects_invalid_signature() {
    let auth = default_auth();
    let body = br#"{"request_id":"req-1"}"#;
    let nonce = "rpc-nonce-3";
    let timestamp = Utc::now().timestamp();
    let mut headers = signed_headers(
        &auth,
        ENCLAVE_RPC_PATH_EXCHANGE_GOOGLE_TOKEN,
        body,
        timestamp,
        nonce,
    );
    headers.insert(
        ENCLAVE_RPC_AUTH_SIGNATURE_HEADER,
        HeaderValue::from_static("deadbeef"),
    );
    let replay_guard = Mutex::new(HashMap::new());

    let err = authorize_request(
        &auth,
        &replay_guard,
        &headers,
        ENCLAVE_RPC_PATH_EXCHANGE_GOOGLE_TOKEN,
        body,
    )
    .expect_err("signature mismatch must fail");

    assert_eq!(err.status, StatusCode::UNAUTHORIZED);
    assert_eq!(err.body.error.code, "invalid_request_signature");
}

#[test]
fn authorize_request_rejects_timestamp_outside_skew() {
    let auth = default_auth();
    let body = br#"{"request_id":"req-1"}"#;
    let nonce = "rpc-nonce-4";
    let timestamp = Utc::now().timestamp() - 120;
    let headers = signed_headers(
        &auth,
        ENCLAVE_RPC_PATH_EXCHANGE_GOOGLE_TOKEN,
        body,
        timestamp,
        nonce,
    );
    let replay_guard = Mutex::new(HashMap::new());

    let err = authorize_request(
        &auth,
        &replay_guard,
        &headers,
        ENCLAVE_RPC_PATH_EXCHANGE_GOOGLE_TOKEN,
        body,
    )
    .expect_err("stale timestamp must fail");

    assert_eq!(err.status, StatusCode::UNAUTHORIZED);
    assert_eq!(err.body.error.code, "invalid_request_timestamp");
}

#[test]
fn authorize_request_rejects_nonce_replay() {
    let auth = default_auth();
    let body = br#"{"request_id":"req-1"}"#;
    let nonce = "rpc-replay-nonce";
    let timestamp = Utc::now().timestamp();
    let headers = signed_headers(
        &auth,
        ENCLAVE_RPC_PATH_EXCHANGE_GOOGLE_TOKEN,
        body,
        timestamp,
        nonce,
    );
    let replay_guard = Mutex::new(HashMap::new());

    let first = authorize_request(
        &auth,
        &replay_guard,
        &headers,
        ENCLAVE_RPC_PATH_EXCHANGE_GOOGLE_TOKEN,
        body,
    );
    assert!(first.is_ok(), "first nonce use should pass");

    let err = authorize_request(
        &auth,
        &replay_guard,
        &headers,
        ENCLAVE_RPC_PATH_EXCHANGE_GOOGLE_TOKEN,
        body,
    )
    .expect_err("nonce replay should fail");

    assert_eq!(err.status, StatusCode::UNAUTHORIZED);
    assert_eq!(err.body.error.code, "request_replay_detected");
}
