mod support;

use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode, header};
use serde_json::{Value, json};
use serial_test::serial;
use tower::ServiceExt;
use uuid::Uuid;

use support::api_app::{build_test_router, oauth_redirect_uri, user_id_for_subject};
use support::clerk::TestClerkAuth;

#[tokio::test]
#[serial]
async fn assistant_attested_key_rejects_invalid_or_expired_challenge_windows() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let clerk = TestClerkAuth::start().await;
    let auth = format!("Bearer {}", clerk.token_for_subject("assistant-user"));
    let app = build_test_router(store, &clerk).await;

    let invalid_window = send_json(
        &app,
        request(
            Method::POST,
            "/v1/assistant/attested-key",
            Some(&auth),
            Some(json!({
                "challenge_nonce": "nonce-1",
                "issued_at": 200,
                "expires_at": 100,
                "request_id": "req-1"
            })),
        ),
    )
    .await;
    assert_eq!(invalid_window.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        error_code(&invalid_window.body),
        Some("invalid_challenge_window")
    );

    let expired = send_json(
        &app,
        request(
            Method::POST,
            "/v1/assistant/attested-key",
            Some(&auth),
            Some(json!({
                "challenge_nonce": "nonce-2",
                "issued_at": 1,
                "expires_at": 2,
                "request_id": "req-2"
            })),
        ),
    )
    .await;
    assert_eq!(expired.status, StatusCode::BAD_REQUEST);
    assert_eq!(error_code(&expired.body), Some("challenge_expired"));
}

#[tokio::test]
#[serial]
async fn assistant_query_rejects_malformed_envelopes_and_writes_no_session_rows() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let clerk = TestClerkAuth::start().await;
    let subject = "assistant-envelope-user";
    let auth = format!("Bearer {}", clerk.token_for_subject(subject));
    let user_id = user_id_for_subject(&clerk.issuer, subject);
    let app = build_test_router(store.clone(), &clerk).await;

    let malformed = send_json(
        &app,
        request(
            Method::POST,
            "/v1/assistant/query",
            Some(&auth),
            Some(json!({
                "envelope": {
                    "version": "v2",
                    "algorithm": "x25519-chacha20poly1305",
                    "key_id": "assistant-ingress-v1",
                    "request_id": "request-1",
                    "client_ephemeral_public_key": "AA==",
                    "nonce": "AA==",
                    "ciphertext": "AA=="
                }
            })),
        ),
    )
    .await;

    assert_eq!(malformed.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        error_code(&malformed.body),
        Some("invalid_envelope_version")
    );

    let session_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint
         FROM assistant_encrypted_sessions
         WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(store.pool())
    .await
    .expect("session count query should succeed");
    assert_eq!(session_count, 0);
}

#[tokio::test]
#[serial]
async fn sensitive_endpoints_enforce_deterministic_rate_limits() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let clerk = TestClerkAuth::start().await;
    let auth = format!("Bearer {}", clerk.token_for_subject("ratelimit-user"));
    let app = build_test_router(store, &clerk).await;

    for _ in 0..20 {
        let response = send_json(
            &app,
            request(
                Method::POST,
                "/v1/connectors/google/start",
                Some(&auth),
                Some(json!({"redirect_uri": oauth_redirect_uri()})),
            ),
        )
        .await;
        assert_eq!(response.status, StatusCode::OK);
    }
    let start_limited = send_raw(
        &app,
        request(
            Method::POST,
            "/v1/connectors/google/start",
            Some(&auth),
            Some(json!({"redirect_uri": oauth_redirect_uri()})),
        ),
    )
    .await;
    assert_eq!(start_limited.status, StatusCode::TOO_MANY_REQUESTS);
    assert!(start_limited.retry_after.is_some());

    for _ in 0..20 {
        let response = send_json(
            &app,
            request(
                Method::POST,
                "/v1/connectors/google/callback",
                Some(&auth),
                Some(json!({"code": "any", "state": "missing-state"})),
            ),
        )
        .await;
        assert_eq!(response.status, StatusCode::BAD_REQUEST);
    }
    let callback_limited = send_raw(
        &app,
        request(
            Method::POST,
            "/v1/connectors/google/callback",
            Some(&auth),
            Some(json!({"code": "any", "state": "missing-state"})),
        ),
    )
    .await;
    assert_eq!(callback_limited.status, StatusCode::TOO_MANY_REQUESTS);
    assert!(callback_limited.retry_after.is_some());

    let missing_connector = Uuid::new_v4();
    for _ in 0..10 {
        let response = send_json(
            &app,
            request(
                Method::DELETE,
                &format!("/v1/connectors/{missing_connector}"),
                Some(&auth),
                None,
            ),
        )
        .await;
        assert_eq!(response.status, StatusCode::NOT_FOUND);
    }
    let revoke_limited = send_raw(
        &app,
        request(
            Method::DELETE,
            &format!("/v1/connectors/{missing_connector}"),
            Some(&auth),
            None,
        ),
    )
    .await;
    assert_eq!(revoke_limited.status, StatusCode::TOO_MANY_REQUESTS);
    assert!(revoke_limited.retry_after.is_some());

    for _ in 0..3 {
        let response = send_json(
            &app,
            request(
                Method::POST,
                "/v1/privacy/delete-all",
                Some(&auth),
                Some(json!({})),
            ),
        )
        .await;
        assert_eq!(response.status, StatusCode::OK);
    }
    let delete_limited = send_raw(
        &app,
        request(
            Method::POST,
            "/v1/privacy/delete-all",
            Some(&auth),
            Some(json!({})),
        ),
    )
    .await;
    assert_eq!(delete_limited.status, StatusCode::TOO_MANY_REQUESTS);
    assert!(delete_limited.retry_after.is_some());
}

struct JsonResponse {
    status: StatusCode,
    body: Value,
}

struct RawResponse {
    status: StatusCode,
    retry_after: Option<String>,
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

async fn send_raw(app: &axum::Router, request: Request<Body>) -> RawResponse {
    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("request should succeed");

    RawResponse {
        status: response.status(),
        retry_after: response
            .headers()
            .get(header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .map(ToString::to_string),
    }
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
