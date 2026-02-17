mod support;

use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode, header};
use chrono::{Duration, Utc};
use serde_json::{Value, json};
use serial_test::serial;
use sha2::{Digest, Sha256};
use tower::ServiceExt;

use support::api_app::{build_test_router, user_id_for_subject};
use support::clerk::TestClerkAuth;

#[tokio::test]
#[serial]
async fn protected_routes_require_valid_bearer_token() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let clerk = TestClerkAuth::start().await;
    let app = build_test_router(store, &clerk).await;

    let missing_auth = send_json(&app, request(Method::GET, "/v1/preferences", None, None)).await;
    assert_eq!(missing_auth.status, StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(&missing_auth.body), Some("unauthorized"));

    let invalid_auth = send_json(
        &app,
        request(
            Method::GET,
            "/v1/preferences",
            Some("Bearer not-a-jwt-token"),
            None,
        ),
    )
    .await;
    assert_eq!(invalid_auth.status, StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(&invalid_auth.body), Some("unauthorized"));
}

#[tokio::test]
#[serial]
async fn clerk_token_validation_fails_closed_for_wrong_claims() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let clerk = TestClerkAuth::start().await;
    let app = build_test_router(store.clone(), &clerk).await;

    let wrong_audience = send_json(
        &app,
        request(
            Method::GET,
            "/v1/preferences",
            Some(&format!(
                "Bearer {}",
                clerk.token_with_audience("user-a", "wrong-audience")
            )),
            None,
        ),
    )
    .await;
    assert_eq!(wrong_audience.status, StatusCode::UNAUTHORIZED);

    let wrong_issuer = send_json(
        &app,
        request(
            Method::GET,
            "/v1/preferences",
            Some(&format!(
                "Bearer {}",
                clerk.token_with_issuer("user-a", "https://other-issuer.test")
            )),
            None,
        ),
    )
    .await;
    assert_eq!(wrong_issuer.status, StatusCode::UNAUTHORIZED);

    let expired = send_json(
        &app,
        request(
            Method::GET,
            "/v1/preferences",
            Some(&format!(
                "Bearer {}",
                clerk.expired_token_for_subject("user-a")
            )),
            None,
        ),
    )
    .await;
    assert_eq!(expired.status, StatusCode::UNAUTHORIZED);

    let user_count: i64 = sqlx::query_scalar("SELECT COUNT(*)::bigint FROM users")
        .fetch_one(store.pool())
        .await
        .expect("user count should query");
    assert_eq!(user_count, 0);
}

#[tokio::test]
#[serial]
async fn valid_identity_is_stable_and_does_not_duplicate_user_rows() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let clerk = TestClerkAuth::start().await;
    let token = clerk.token_for_subject("stable-user");
    let auth = format!("Bearer {token}");

    let app = build_test_router(store.clone(), &clerk).await;

    let first = send_json(
        &app,
        request(Method::GET, "/v1/preferences", Some(&auth), None),
    )
    .await;
    assert_eq!(first.status, StatusCode::OK);

    let second = send_json(
        &app,
        request(Method::GET, "/v1/preferences", Some(&auth), None),
    )
    .await;
    assert_eq!(second.status, StatusCode::OK);

    let expected_user_id = user_id_for_subject(&clerk.issuer, "stable-user");

    let user_count: i64 = sqlx::query_scalar("SELECT COUNT(*)::bigint FROM users")
        .fetch_one(store.pool())
        .await
        .expect("user count should query");
    assert_eq!(user_count, 1);

    let matching_rows: i64 = sqlx::query_scalar("SELECT COUNT(*)::bigint FROM users WHERE id = $1")
        .bind(expected_user_id)
        .fetch_one(store.pool())
        .await
        .expect("matching user count should query");
    assert_eq!(matching_rows, 1);
}

#[tokio::test]
#[serial]
async fn user_cannot_read_or_mutate_other_users_privacy_or_connector_resources() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let clerk = TestClerkAuth::start().await;
    let user_a_subject = "user-a";
    let user_b_subject = "user-b";
    let user_a_auth = format!("Bearer {}", clerk.token_for_subject(user_a_subject));
    let user_b_auth = format!("Bearer {}", clerk.token_for_subject(user_b_subject));
    let user_a_id = user_id_for_subject(&clerk.issuer, user_a_subject);

    let app = build_test_router(store.clone(), &clerk).await;

    let delete_all = send_json(
        &app,
        request(
            Method::POST,
            "/v1/privacy/delete-all",
            Some(&user_a_auth),
            Some(json!({})),
        ),
    )
    .await;
    assert_eq!(delete_all.status, StatusCode::OK);
    let request_id = delete_all
        .body
        .get("request_id")
        .and_then(Value::as_str)
        .expect("delete-all response should include request_id");

    let forbidden_status = send_json(
        &app,
        request(
            Method::GET,
            &format!("/v1/privacy/delete-all/{request_id}"),
            Some(&user_b_auth),
            None,
        ),
    )
    .await;
    assert_eq!(forbidden_status.status, StatusCode::NOT_FOUND);

    let connector_id = store
        .upsert_google_connector(
            user_a_id,
            "refresh-token-a",
            &["https://www.googleapis.com/auth/calendar.readonly".to_string()],
            "kms/local/alfred-refresh-token",
            1,
        )
        .await
        .expect("connector insert should succeed");

    let revoke_other_user_connector = send_json(
        &app,
        request(
            Method::DELETE,
            &format!("/v1/connectors/{connector_id}"),
            Some(&user_b_auth),
            None,
        ),
    )
    .await;
    assert_eq!(revoke_other_user_connector.status, StatusCode::NOT_FOUND);

    let connector_row = store
        .get_active_connector_key_metadata(user_a_id, connector_id)
        .await
        .expect("connector metadata lookup should succeed");
    assert!(connector_row.is_some());
}

#[tokio::test]
#[serial]
async fn callback_state_and_revoke_authorization_fail_closed_for_non_owner() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let clerk = TestClerkAuth::start().await;
    let user_a_subject = "owner-user";
    let user_b_subject = "other-user";
    let user_a_id = user_id_for_subject(&clerk.issuer, user_a_subject);
    let user_b_auth = format!("Bearer {}", clerk.token_for_subject(user_b_subject));

    let app = build_test_router(store.clone(), &clerk).await;

    let state_token = "test-state-token";
    let state_hash = Sha256::digest(state_token.as_bytes()).to_vec();
    store
        .store_oauth_state(
            user_a_id,
            &state_hash,
            "alfred://oauth/google/callback",
            Utc::now() + Duration::minutes(5),
        )
        .await
        .expect("oauth state should store");

    let callback_as_wrong_user = send_json(
        &app,
        request(
            Method::POST,
            "/v1/connectors/google/callback",
            Some(&user_b_auth),
            Some(json!({"code": "auth-code", "state": state_token})),
        ),
    )
    .await;
    assert_eq!(callback_as_wrong_user.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        error_code(&callback_as_wrong_user.body),
        Some("invalid_state")
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
