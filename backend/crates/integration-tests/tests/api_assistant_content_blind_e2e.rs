mod support;

use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode, header};
use axum::routing::post;
use base64::Engine as _;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use chrono::{Duration, Utc};
use serde_json::{Value, json};
use serial_test::serial;
use sha2::{Digest, Sha256};
use shared::assistant_crypto::{
    ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305, ASSISTANT_ENVELOPE_VERSION_V1,
    AssistantIngressKeyMaterial, AssistantIngressKeyring, decrypt_assistant_request,
    derive_public_key_b64, encrypt_assistant_response,
};
use shared::enclave::{
    AttestedIdentityPayload, ENCLAVE_RPC_CONTRACT_VERSION,
    ENCLAVE_RPC_PATH_FETCH_ASSISTANT_ATTESTED_KEY, ENCLAVE_RPC_PATH_PROCESS_ASSISTANT_QUERY,
    EnclaveRpcFetchAssistantAttestedKeyRequest, EnclaveRpcFetchAssistantAttestedKeyResponse,
    EnclaveRpcProcessAssistantQueryRequest, EnclaveRpcProcessAssistantQueryResponse,
};
use shared::models::{
    AssistantAttestedKeyResponse, AssistantEncryptedRequestEnvelope,
    AssistantPlaintextQueryRequest, AssistantPlaintextQueryResponse, AssistantQueryCapability,
    AssistantQueryRequest, AssistantQueryResponse, AssistantSessionStateEnvelope,
};
use tokio::sync::Mutex;
use tower::ServiceExt;
use uuid::Uuid;
use x25519_dalek::{PublicKey, StaticSecret};

use support::api_app::{build_test_router_with_enclave_base_url, user_id_for_subject};
use support::clerk::TestClerkAuth;
use support::enclave_mock::MockEnclaveServer;

#[tokio::test]
#[serial]
async fn mock_ios_encrypted_query_round_trip_keeps_host_content_blind() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let keyring = test_ingress_keyring();
    let captured_plaintext_query = Arc::new(Mutex::new(None::<String>));
    let mock_enclave =
        start_assistant_mock_enclave(keyring.clone(), captured_plaintext_query.clone()).await;

    let clerk = TestClerkAuth::start().await;
    let subject = "assistant-e2e-user";
    let user_id = user_id_for_subject(&clerk.issuer, subject);
    let auth = format!("Bearer {}", clerk.token_for_subject(subject));
    let app =
        build_test_router_with_enclave_base_url(store.clone(), &clerk, &mock_enclave.base_url)
            .await;

    let challenge_now = Utc::now().timestamp();
    let key_response = send_json(
        &app,
        request(
            Method::POST,
            "/v1/assistant/attested-key",
            Some(&auth),
            Some(json!({
                "challenge_nonce": "ios-challenge-1",
                "issued_at": challenge_now - 1,
                "expires_at": challenge_now + 60,
                "request_id": "ios-attested-key-req-1"
            })),
        ),
    )
    .await;
    assert_eq!(key_response.status, StatusCode::OK);
    let attested_key: AssistantAttestedKeyResponse =
        serde_json::from_value(key_response.body).expect("attested key response should decode");
    assert_eq!(
        attested_key.algorithm,
        ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305
    );

    let plaintext_query = "top secret: board meeting strategy";
    let request_id = Uuid::new_v4().to_string();
    let (envelope, client_private_key) = encrypt_mock_ios_request(
        request_id.as_str(),
        plaintext_query,
        None,
        attested_key.key_id.as_str(),
        attested_key.public_key.as_str(),
    );

    let assistant_query = send_json(
        &app,
        request(
            Method::POST,
            "/v1/assistant/query",
            Some(&auth),
            Some(
                serde_json::to_value(AssistantQueryRequest {
                    envelope,
                    session_id: None,
                })
                .expect("assistant query request should serialize"),
            ),
        ),
    )
    .await;
    assert_eq!(assistant_query.status, StatusCode::OK);

    let api_response: AssistantQueryResponse = serde_json::from_value(assistant_query.body)
        .expect("assistant query response should decode");
    let decrypted_response = decrypt_mock_ios_response(
        api_response.envelope.request_id.as_str(),
        &api_response.envelope,
        &client_private_key,
        attested_key.public_key.as_str(),
    );
    assert_eq!(
        decrypted_response.display_text,
        "Encrypted response from enclave"
    );

    let captured_query = captured_plaintext_query.lock().await.clone();
    assert_eq!(captured_query.as_deref(), Some(plaintext_query));

    let persisted_state_json: String = sqlx::query_scalar(
        "SELECT state_json
         FROM assistant_encrypted_sessions
         WHERE user_id = $1 AND session_id = $2",
    )
    .bind(user_id)
    .bind(api_response.session_id)
    .fetch_one(store.pool())
    .await
    .expect("assistant session state should persist");
    assert!(
        persisted_state_json.contains("\"ciphertext\""),
        "persisted session state should remain an encrypted envelope"
    );
    assert!(
        !persisted_state_json.contains(plaintext_query),
        "host persistence must remain content blind"
    );
    assert!(
        !persisted_state_json.contains("\"query\""),
        "plaintext query field must never be stored in session envelope state"
    );

    let plaintext_leak_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint
         FROM assistant_encrypted_sessions
         WHERE user_id = $1
           AND state_json ILIKE $2",
    )
    .bind(user_id)
    .bind(format!("%{plaintext_query}%"))
    .fetch_one(store.pool())
    .await
    .expect("leak check query should succeed");
    assert_eq!(plaintext_leak_count, 0);
}

async fn start_assistant_mock_enclave(
    keyring: AssistantIngressKeyring,
    captured_plaintext_query: Arc<Mutex<Option<String>>>,
) -> MockEnclaveServer {
    let attested_key = keyring.active.clone();
    let query_keyring = keyring.clone();

    MockEnclaveServer::start(
        axum::Router::new()
            .route(
                ENCLAVE_RPC_PATH_FETCH_ASSISTANT_ATTESTED_KEY,
                post(
                    move |axum::Json(request): axum::Json<
                        EnclaveRpcFetchAssistantAttestedKeyRequest,
                    >| {
                        let attested_key = attested_key.clone();
                        async move {
                            axum::Json(EnclaveRpcFetchAssistantAttestedKeyResponse {
                                contract_version: ENCLAVE_RPC_CONTRACT_VERSION.to_string(),
                                request_id: request.request_id,
                                runtime: "nitro".to_string(),
                                measurement: "dev-local-enclave".to_string(),
                                challenge_nonce: request.challenge_nonce,
                                issued_at: request.issued_at,
                                expires_at: request.expires_at,
                                evidence_issued_at: Utc::now().timestamp(),
                                key_id: attested_key.key_id,
                                algorithm: ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305
                                    .to_string(),
                                public_key: attested_key.public_key,
                                key_expires_at: attested_key.key_expires_at,
                                signature: None,
                            })
                        }
                    },
                ),
            )
            .route(
                ENCLAVE_RPC_PATH_PROCESS_ASSISTANT_QUERY,
                post(
                    move |axum::Json(request): axum::Json<
                        EnclaveRpcProcessAssistantQueryRequest,
                    >| {
                        let query_keyring = query_keyring.clone();
                        let captured_plaintext_query = captured_plaintext_query.clone();
                        async move {
                            let (plaintext_request, selected_key) =
                                decrypt_assistant_request(&query_keyring, &request.envelope)
                                    .expect("mock enclave should decrypt request");
                            *captured_plaintext_query.lock().await =
                                Some(plaintext_request.query.clone());

                            let response_payload = AssistantPlaintextQueryResponse {
                                session_id: Uuid::new_v4(),
                                capability: AssistantQueryCapability::GeneralChat,
                                display_text: "Encrypted response from enclave".to_string(),
                                payload: shared::models::AssistantStructuredPayload {
                                    title: "Encrypted".to_string(),
                                    summary: "Host cannot read plaintext".to_string(),
                                    key_points: vec!["Enclave-only decrypt path".to_string()],
                                    follow_ups: vec![],
                                },
                                response_parts: vec![],
                            };

                            let response_envelope = encrypt_assistant_response(
                                &selected_key,
                                request.envelope.request_id.as_str(),
                                request.envelope.client_ephemeral_public_key.as_str(),
                                &response_payload,
                            )
                            .expect("mock enclave should encrypt response");

                            let session_id =
                                request.session_id.unwrap_or(response_payload.session_id);
                            let session_state = AssistantSessionStateEnvelope {
                                version: ASSISTANT_ENVELOPE_VERSION_V1.to_string(),
                                algorithm: ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305
                                    .to_string(),
                                key_id: selected_key.key_id,
                                nonce: base64::engine::general_purpose::STANDARD.encode([7_u8; 12]),
                                ciphertext: base64::engine::general_purpose::STANDARD
                                    .encode("session-state-ciphertext".as_bytes()),
                                expires_at: Utc::now() + Duration::minutes(10),
                            };

                            axum::Json(EnclaveRpcProcessAssistantQueryResponse {
                                contract_version: ENCLAVE_RPC_CONTRACT_VERSION.to_string(),
                                request_id: request.request_id,
                                session_id,
                                envelope: response_envelope,
                                session_state: Some(session_state),
                                attested_identity: AttestedIdentityPayload {
                                    runtime: "nitro".to_string(),
                                    measurement: "dev-local-enclave".to_string(),
                                },
                            })
                        }
                    },
                ),
            ),
    )
    .await
}

fn test_ingress_keyring() -> AssistantIngressKeyring {
    let private_key = [9_u8; 32];
    AssistantIngressKeyring {
        active: AssistantIngressKeyMaterial {
            key_id: "assistant-ingress-v1".to_string(),
            private_key,
            public_key: derive_public_key_b64(private_key),
            key_expires_at: Utc::now().timestamp() + 3600,
        },
        previous: None,
    }
}

fn encrypt_mock_ios_request(
    request_id: &str,
    query: &str,
    session_id: Option<Uuid>,
    key_id: &str,
    enclave_public_key_b64: &str,
) -> (AssistantEncryptedRequestEnvelope, StaticSecret) {
    let client_private_key = StaticSecret::from([5_u8; 32]);
    let client_public_key = PublicKey::from(&client_private_key);
    let enclave_public_key_raw = base64::engine::general_purpose::STANDARD
        .decode(enclave_public_key_b64.as_bytes())
        .expect("attested key public_key should decode");
    let enclave_public_key: [u8; 32] = enclave_public_key_raw
        .try_into()
        .expect("attested key public_key should decode to 32 bytes");
    let enclave_public_key = PublicKey::from(enclave_public_key);

    let shared_secret = client_private_key.diffie_hellman(&enclave_public_key);
    let mut hasher = Sha256::new();
    hasher.update(shared_secret.as_bytes());
    hasher.update(b"|");
    hasher.update(request_id.as_bytes());
    hasher.update(b"|");
    hasher.update(b"request");
    let derived_key: [u8; 32] = hasher.finalize().into();
    let cipher = ChaCha20Poly1305::new_from_slice(&derived_key)
        .expect("request encrypt key should initialize");

    let nonce_source = Uuid::new_v4();
    let request_nonce = &nonce_source.as_bytes()[..12];
    let nonce = Nonce::from_slice(request_nonce);
    let plaintext = serde_json::to_vec(&AssistantPlaintextQueryRequest {
        query: query.to_string(),
        session_id,
    })
    .expect("plaintext assistant request should serialize");
    let ciphertext = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext.as_slice(),
                aad: request_id.as_bytes(),
            },
        )
        .expect("mock iOS request encryption should succeed");

    (
        AssistantEncryptedRequestEnvelope {
            version: ASSISTANT_ENVELOPE_VERSION_V1.to_string(),
            algorithm: ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305.to_string(),
            key_id: key_id.to_string(),
            request_id: request_id.to_string(),
            client_ephemeral_public_key: base64::engine::general_purpose::STANDARD
                .encode(client_public_key.as_bytes()),
            nonce: base64::engine::general_purpose::STANDARD.encode(request_nonce),
            ciphertext: base64::engine::general_purpose::STANDARD.encode(ciphertext),
        },
        client_private_key,
    )
}

fn decrypt_mock_ios_response(
    request_id: &str,
    envelope: &shared::models::AssistantEncryptedResponseEnvelope,
    client_private_key: &StaticSecret,
    enclave_public_key_b64: &str,
) -> AssistantPlaintextQueryResponse {
    let enclave_public_key_raw = base64::engine::general_purpose::STANDARD
        .decode(enclave_public_key_b64.as_bytes())
        .expect("attested key public_key should decode");
    let enclave_public_key: [u8; 32] = enclave_public_key_raw
        .try_into()
        .expect("attested key public_key should decode to 32 bytes");
    let enclave_public_key = PublicKey::from(enclave_public_key);

    let shared_secret = client_private_key.diffie_hellman(&enclave_public_key);
    let mut hasher = Sha256::new();
    hasher.update(shared_secret.as_bytes());
    hasher.update(b"|");
    hasher.update(request_id.as_bytes());
    hasher.update(b"|");
    hasher.update(b"response");
    let derived_key: [u8; 32] = hasher.finalize().into();
    let cipher = ChaCha20Poly1305::new_from_slice(&derived_key)
        .expect("response decrypt key should initialize");

    let nonce_bytes = base64::engine::general_purpose::STANDARD
        .decode(envelope.nonce.as_bytes())
        .expect("response nonce should decode");
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = base64::engine::general_purpose::STANDARD
        .decode(envelope.ciphertext.as_bytes())
        .expect("response ciphertext should decode");
    let plaintext = cipher
        .decrypt(
            nonce,
            Payload {
                msg: ciphertext.as_slice(),
                aad: request_id.as_bytes(),
            },
        )
        .expect("response should decrypt");
    serde_json::from_slice::<AssistantPlaintextQueryResponse>(&plaintext)
        .expect("response plaintext should decode")
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
