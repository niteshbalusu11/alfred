mod support;

use std::sync::Arc;

use axum::http::{Method, StatusCode};
use axum::routing::post;
use base64::Engine as _;
use chrono::{Duration, Utc};
use serial_test::serial;
use shared::assistant_crypto::{
    ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305, ASSISTANT_ENVELOPE_VERSION_V1,
    AssistantIngressKeyring, decrypt_assistant_request, encrypt_assistant_response,
};
use shared::enclave::{
    AttestedIdentityPayload, ENCLAVE_RPC_CONTRACT_VERSION,
    ENCLAVE_RPC_PATH_FETCH_ASSISTANT_ATTESTED_KEY, ENCLAVE_RPC_PATH_PROCESS_ASSISTANT_QUERY,
    EnclaveRpcFetchAssistantAttestedKeyRequest, EnclaveRpcFetchAssistantAttestedKeyResponse,
    EnclaveRpcProcessAssistantQueryRequest, EnclaveRpcProcessAssistantQueryResponse,
};
use shared::models::{
    AssistantPlaintextQueryResponse, AssistantQueryCapability, AssistantQueryRequest,
    AssistantQueryResponse, AssistantResponsePart, AssistantSessionStateEnvelope,
    AssistantStructuredPayload,
};
use tokio::sync::Mutex;
use uuid::Uuid;

use support::api_app::build_test_router_with_enclave_base_url;
use support::assistant_encrypted::{
    decrypt_mock_ios_response, encrypt_mock_ios_request, fetch_attested_key, request, send_json,
    test_ingress_keyring,
};
use support::clerk::TestClerkAuth;
use support::enclave_mock::MockEnclaveServer;

const FOLLOW_UP_TEXT: &str = "Follow-up continuity processed through encrypted session state.";
const CLARIFICATION_TEXT: &str =
    "Could you clarify whether you need calendar details, email details, or both?";

#[derive(Debug, Clone)]
struct CapturedAssistantCall {
    query: String,
    session_id: Option<Uuid>,
    has_prior_session_state: bool,
}

#[tokio::test]
#[serial]
async fn assistant_follow_up_round_trip_preserves_encrypted_session_continuity() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let keyring = test_ingress_keyring();
    let captured_calls = Arc::new(Mutex::new(Vec::<CapturedAssistantCall>::new()));
    let mock_enclave = start_multiturn_mock_enclave(keyring.clone(), captured_calls.clone()).await;

    let clerk = TestClerkAuth::start().await;
    let auth = format!(
        "Bearer {}",
        clerk.token_for_subject("assistant-multiturn-user")
    );
    let app =
        build_test_router_with_enclave_base_url(store.clone(), &clerk, &mock_enclave.base_url)
            .await;

    let attested_key = fetch_attested_key(&app, auth.as_str()).await;

    let first_request_id = Uuid::new_v4().to_string();
    let (first_envelope, first_private_key) = encrypt_mock_ios_request(
        first_request_id.as_str(),
        "show my calendar this week",
        None,
        attested_key.key_id.as_str(),
        attested_key.public_key.as_str(),
    );

    let first_query_response = send_json(
        &app,
        request(
            Method::POST,
            "/v1/assistant/query",
            Some(auth.as_str()),
            Some(
                serde_json::to_value(AssistantQueryRequest {
                    envelope: first_envelope,
                    session_id: None,
                })
                .expect("assistant query should serialize"),
            ),
        ),
    )
    .await;
    assert_eq!(first_query_response.status, StatusCode::OK);

    let first_response: AssistantQueryResponse =
        serde_json::from_value(first_query_response.body).expect("first response should decode");
    let decrypted_first = decrypt_mock_ios_response(
        first_response.envelope.request_id.as_str(),
        &first_response.envelope,
        &first_private_key,
        attested_key.public_key.as_str(),
    );
    assert_eq!(decrypted_first.display_text, FOLLOW_UP_TEXT);

    let second_request_id = Uuid::new_v4().to_string();
    let (second_envelope, second_private_key) = encrypt_mock_ios_request(
        second_request_id.as_str(),
        "what about after that",
        Some(first_response.session_id),
        attested_key.key_id.as_str(),
        attested_key.public_key.as_str(),
    );

    let second_query_response = send_json(
        &app,
        request(
            Method::POST,
            "/v1/assistant/query",
            Some(auth.as_str()),
            Some(
                serde_json::to_value(AssistantQueryRequest {
                    envelope: second_envelope,
                    session_id: Some(first_response.session_id),
                })
                .expect("assistant follow-up query should serialize"),
            ),
        ),
    )
    .await;
    assert_eq!(second_query_response.status, StatusCode::OK);

    let second_response: AssistantQueryResponse =
        serde_json::from_value(second_query_response.body).expect("second response should decode");
    assert_eq!(second_response.session_id, first_response.session_id);

    let decrypted_second = decrypt_mock_ios_response(
        second_response.envelope.request_id.as_str(),
        &second_response.envelope,
        &second_private_key,
        attested_key.public_key.as_str(),
    );
    assert_eq!(decrypted_second.display_text, FOLLOW_UP_TEXT);

    let calls = captured_calls.lock().await.clone();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].query, "show my calendar this week");
    assert!(!calls[0].has_prior_session_state);
    assert_eq!(calls[1].query, "what about after that");
    assert!(calls[1].has_prior_session_state);
    assert_eq!(calls[1].session_id, Some(first_response.session_id));
}

#[tokio::test]
#[serial]
async fn clarification_response_survives_encrypted_round_trip() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let keyring = test_ingress_keyring();
    let captured_calls = Arc::new(Mutex::new(Vec::<CapturedAssistantCall>::new()));
    let mock_enclave = start_multiturn_mock_enclave(keyring.clone(), captured_calls.clone()).await;

    let clerk = TestClerkAuth::start().await;
    let auth = format!(
        "Bearer {}",
        clerk.token_for_subject("assistant-clarification-user")
    );
    let app =
        build_test_router_with_enclave_base_url(store.clone(), &clerk, &mock_enclave.base_url)
            .await;

    let attested_key = fetch_attested_key(&app, auth.as_str()).await;

    let request_id = Uuid::new_v4().to_string();
    let (envelope, private_key) = encrypt_mock_ios_request(
        request_id.as_str(),
        "unclear request",
        None,
        attested_key.key_id.as_str(),
        attested_key.public_key.as_str(),
    );

    let response = send_json(
        &app,
        request(
            Method::POST,
            "/v1/assistant/query",
            Some(auth.as_str()),
            Some(
                serde_json::to_value(AssistantQueryRequest {
                    envelope,
                    session_id: None,
                })
                .expect("assistant query should serialize"),
            ),
        ),
    )
    .await;
    assert_eq!(response.status, StatusCode::OK);

    let parsed: AssistantQueryResponse =
        serde_json::from_value(response.body).expect("assistant response should decode");
    let decrypted = decrypt_mock_ios_response(
        parsed.envelope.request_id.as_str(),
        &parsed.envelope,
        &private_key,
        attested_key.public_key.as_str(),
    );

    assert_eq!(decrypted.capability, AssistantQueryCapability::GeneralChat);
    assert_eq!(decrypted.display_text, CLARIFICATION_TEXT);
    assert_eq!(
        decrypted.response_parts,
        vec![AssistantResponsePart::chat_text(
            CLARIFICATION_TEXT.to_string()
        )]
    );

    let calls = captured_calls.lock().await.clone();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].query, "unclear request");
    assert!(!calls[0].has_prior_session_state);
}

async fn start_multiturn_mock_enclave(
    keyring: AssistantIngressKeyring,
    captured_calls: Arc<Mutex<Vec<CapturedAssistantCall>>>,
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
                        let captured_calls = captured_calls.clone();

                        async move {
                            let (plaintext, selected_key) =
                                decrypt_assistant_request(&query_keyring, &request.envelope)
                                    .expect("mock enclave should decrypt query envelope");
                            captured_calls.lock().await.push(CapturedAssistantCall {
                                query: plaintext.query.clone(),
                                session_id: request.session_id,
                                has_prior_session_state: request.prior_session_state.is_some(),
                            });

                            let display_text =
                                if plaintext.query.to_ascii_lowercase().contains("unclear") {
                                    CLARIFICATION_TEXT.to_string()
                                } else {
                                    FOLLOW_UP_TEXT.to_string()
                                };
                            let payload = AssistantStructuredPayload {
                                title: "integration".to_string(),
                                summary: display_text.clone(),
                                key_points: vec!["integration-test".to_string()],
                                follow_ups: vec![],
                            };
                            let response_payload = AssistantPlaintextQueryResponse {
                                session_id: request.session_id.unwrap_or_else(Uuid::new_v4),
                                capability: AssistantQueryCapability::GeneralChat,
                                display_text: display_text.clone(),
                                payload,
                                response_parts: vec![AssistantResponsePart::chat_text(
                                    display_text,
                                )],
                            };

                            let encrypted_response = encrypt_assistant_response(
                                &selected_key,
                                request.envelope.request_id.as_str(),
                                request.envelope.client_ephemeral_public_key.as_str(),
                                &response_payload,
                            )
                            .expect("mock enclave should encrypt response");

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
                                session_id: response_payload.session_id,
                                envelope: encrypted_response,
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
