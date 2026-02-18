use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode, header};
use base64::Engine as _;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use chrono::Utc;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use shared::assistant_crypto::{
    ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305, ASSISTANT_ENVELOPE_VERSION_V1,
    AssistantIngressKeyMaterial, AssistantIngressKeyring, derive_public_key_b64,
};
use shared::models::{
    AssistantAttestedKeyResponse, AssistantEncryptedRequestEnvelope,
    AssistantPlaintextQueryRequest, AssistantPlaintextQueryResponse,
};
use tower::ServiceExt;
use uuid::Uuid;
use x25519_dalek::{PublicKey, StaticSecret};

pub fn test_ingress_keyring() -> AssistantIngressKeyring {
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

pub fn encrypt_mock_ios_request(
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

pub fn decrypt_mock_ios_response(
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

pub struct JsonResponse {
    pub status: StatusCode,
    pub body: Value,
}

pub async fn send_json(app: &axum::Router, request: Request<Body>) -> JsonResponse {
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

pub fn request(
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

pub async fn fetch_attested_key(
    app: &axum::Router,
    auth_header: &str,
) -> AssistantAttestedKeyResponse {
    let now = Utc::now().timestamp();
    let response = send_json(
        app,
        request(
            Method::POST,
            "/v1/assistant/attested-key",
            Some(auth_header),
            Some(json!({
                "challenge_nonce": "ios-phase-d-challenge",
                "issued_at": now - 1,
                "expires_at": now + 60,
                "request_id": "ios-phase-d-attested-key"
            })),
        ),
    )
    .await;
    assert_eq!(response.status, StatusCode::OK);

    serde_json::from_value(response.body).expect("attested key response should decode")
}
