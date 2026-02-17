mod support;

use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode, header};
use axum::routing::post;
use chrono::{Duration, Utc};
use serde_json::{Value, json};
use serial_test::serial;
use sha2::{Digest, Sha256};
use shared::enclave::{
    ENCLAVE_RPC_CONTRACT_VERSION, ENCLAVE_RPC_PATH_FETCH_ASSISTANT_ATTESTED_KEY,
    ENCLAVE_RPC_PATH_REVOKE_GOOGLE_TOKEN, EnclaveRpcErrorEnvelope,
    EnclaveRpcFetchAssistantAttestedKeyRequest, EnclaveRpcFetchAssistantAttestedKeyResponse,
};
use tower::ServiceExt;

use support::api_app::{
    build_test_router, build_test_router_with_enclave_base_url, oauth_redirect_uri,
    user_id_for_subject,
};
use support::clerk::TestClerkAuth;
use support::enclave_mock::MockEnclaveServer;

#[tokio::test]
#[serial]
async fn oauth_callback_state_is_single_use_and_ttl_bound() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let clerk = TestClerkAuth::start().await;
    let subject = "oauth-state-user";
    let user_id = user_id_for_subject(&clerk.issuer, subject);
    let auth = format!("Bearer {}", clerk.token_for_subject(subject));
    let app = build_test_router(store.clone(), &clerk).await;

    let state = "state-single-use";
    let state_hash = Sha256::digest(state.as_bytes()).to_vec();
    store
        .store_oauth_state(
            user_id,
            &state_hash,
            oauth_redirect_uri(),
            Utc::now() + Duration::minutes(5),
        )
        .await
        .expect("oauth state should store");

    let first_callback = send_json(
        &app,
        request(
            Method::POST,
            "/v1/connectors/google/callback",
            Some(&auth),
            Some(json!({ "code": "code-1", "state": state })),
        ),
    )
    .await;
    assert_eq!(first_callback.status, StatusCode::BAD_GATEWAY);
    assert_eq!(error_code(&first_callback.body), Some("enclave_rpc_failed"));

    let second_callback = send_json(
        &app,
        request(
            Method::POST,
            "/v1/connectors/google/callback",
            Some(&auth),
            Some(json!({ "code": "code-2", "state": state })),
        ),
    )
    .await;
    assert_eq!(second_callback.status, StatusCode::BAD_REQUEST);
    assert_eq!(error_code(&second_callback.body), Some("invalid_state"));

    let expired_state = "state-expired";
    let expired_hash = Sha256::digest(expired_state.as_bytes()).to_vec();
    store
        .store_oauth_state(
            user_id,
            &expired_hash,
            oauth_redirect_uri(),
            Utc::now() - Duration::seconds(1),
        )
        .await
        .expect("expired oauth state should store");

    let expired_callback = send_json(
        &app,
        request(
            Method::POST,
            "/v1/connectors/google/callback",
            Some(&auth),
            Some(json!({ "code": "code-3", "state": expired_state })),
        ),
    )
    .await;
    assert_eq!(expired_callback.status, StatusCode::BAD_REQUEST);
    assert_eq!(error_code(&expired_callback.body), Some("invalid_state"));
}

#[tokio::test]
#[serial]
async fn revoke_fails_closed_when_enclave_reports_connector_token_unavailable() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let mock_enclave = MockEnclaveServer::start(axum::Router::new().route(
        ENCLAVE_RPC_PATH_REVOKE_GOOGLE_TOKEN,
        post(|| async move {
            (
                StatusCode::BAD_REQUEST,
                axum::Json(EnclaveRpcErrorEnvelope::new(
                    None,
                    "connector_token_unavailable",
                    "connector token unavailable",
                    false,
                )),
            )
        }),
    ))
    .await;

    let clerk = TestClerkAuth::start().await;
    let subject = "revoke-drift-user";
    let user_id = user_id_for_subject(&clerk.issuer, subject);
    let auth = format!("Bearer {}", clerk.token_for_subject(subject));
    let app =
        build_test_router_with_enclave_base_url(store.clone(), &clerk, &mock_enclave.base_url)
            .await;

    let connector_id = store
        .upsert_google_connector(
            user_id,
            "refresh-token",
            &["https://www.googleapis.com/auth/calendar.readonly".to_string()],
            "kms/local/alfred-refresh-token",
            1,
        )
        .await
        .expect("connector insert should succeed");

    let revoke = send_json(
        &app,
        request(
            Method::DELETE,
            &format!("/v1/connectors/{connector_id}"),
            Some(&auth),
            None,
        ),
    )
    .await;
    assert_eq!(revoke.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        error_code(&revoke.body),
        Some("connector_token_unavailable")
    );

    let connector_metadata = store
        .get_active_connector_key_metadata(user_id, connector_id)
        .await
        .expect("connector metadata lookup should succeed");
    assert!(
        connector_metadata.is_some(),
        "connector should stay active when enclave revoke fails closed"
    );
}

#[tokio::test]
#[serial]
async fn assistant_attested_key_fails_closed_on_nonce_or_request_mismatch() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let mock_enclave =
        MockEnclaveServer::start(
            axum::Router::new().route(
                ENCLAVE_RPC_PATH_FETCH_ASSISTANT_ATTESTED_KEY,
                post(
                    |axum::Json(request): axum::Json<
                        EnclaveRpcFetchAssistantAttestedKeyRequest,
                    >| async move {
                        axum::Json(EnclaveRpcFetchAssistantAttestedKeyResponse {
                            contract_version: ENCLAVE_RPC_CONTRACT_VERSION.to_string(),
                            request_id: request.request_id,
                            runtime: "nitro".to_string(),
                            measurement: "dev-local-enclave".to_string(),
                            challenge_nonce: "mismatched-nonce".to_string(),
                            issued_at: request.issued_at,
                            expires_at: request.expires_at,
                            evidence_issued_at: request.issued_at,
                            key_id: "assistant-ingress-v1".to_string(),
                            algorithm: "x25519-chacha20poly1305".to_string(),
                            public_key: "AA==".to_string(),
                            key_expires_at: request.expires_at + 60,
                            signature: None,
                        })
                    },
                ),
            ),
        )
        .await;

    let clerk = TestClerkAuth::start().await;
    let auth = format!("Bearer {}", clerk.token_for_subject("attested-key-user"));
    let app = build_test_router_with_enclave_base_url(store, &clerk, &mock_enclave.base_url).await;

    let now = Utc::now().timestamp();
    let attested_key = send_json(
        &app,
        request(
            Method::POST,
            "/v1/assistant/attested-key",
            Some(&auth),
            Some(json!({
                "challenge_nonce": "expected-nonce",
                "issued_at": now - 1,
                "expires_at": now + 30,
                "request_id": "req-123"
            })),
        ),
    )
    .await;

    assert_eq!(attested_key.status, StatusCode::BAD_GATEWAY);
    assert_eq!(
        error_code(&attested_key.body),
        Some("attestation_challenge_mismatch")
    );
}

struct JsonResponse {
    status: StatusCode,
    body: Value,
}

async fn send_json(app: &axum::Router, request: Request<Body>) -> JsonResponse {
    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("request should succeed");
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body should read");
    let body = serde_json::from_slice::<Value>(&body).unwrap_or_else(|_| json!({}));

    JsonResponse { status, body }
}

fn request(
    method: Method,
    uri: &str,
    auth_header: Option<&str>,
    json_body: Option<Value>,
) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(auth_header) = auth_header {
        builder = builder.header(header::AUTHORIZATION, auth_header);
    }

    match json_body {
        Some(body) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .expect("request should build"),
        None => builder.body(Body::empty()).expect("request should build"),
    }
}

fn error_code(body: &Value) -> Option<&str> {
    body.get("error")
        .and_then(|error| error.get("code"))
        .and_then(Value::as_str)
}
