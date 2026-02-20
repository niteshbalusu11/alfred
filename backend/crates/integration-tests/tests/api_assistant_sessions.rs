mod support;

use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode, header};
use chrono::{Duration, Utc};
use serde_json::{Value, json};
use serial_test::serial;
use shared::models::{AssistantSessionStateEnvelope, ListAssistantSessionsResponse, OkResponse};
use tower::ServiceExt;
use uuid::Uuid;

use support::api_app::{build_test_router, user_id_for_subject};
use support::clerk::TestClerkAuth;

#[tokio::test]
#[serial]
async fn assistant_sessions_list_and_delete_respect_user_boundaries() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let clerk = TestClerkAuth::start().await;
    let subject_a = "assistant-sessions-user-a";
    let subject_b = "assistant-sessions-user-b";
    let user_a = user_id_for_subject(&clerk.issuer, subject_a);
    let user_b = user_id_for_subject(&clerk.issuer, subject_b);
    let auth_a = format!("Bearer {}", clerk.token_for_subject(subject_a));
    let auth_b = format!("Bearer {}", clerk.token_for_subject(subject_b));
    let app = build_test_router(store.clone(), &clerk).await;

    let now = Utc::now();
    let session_a_old = Uuid::new_v4();
    let session_a_new = Uuid::new_v4();
    let session_b = Uuid::new_v4();

    store
        .upsert_assistant_encrypted_session(
            user_a,
            session_a_old,
            &test_state("cipher-a-old", now + Duration::days(3)),
            now - Duration::minutes(30),
            3600,
        )
        .await
        .expect("older session insert should succeed");
    store
        .upsert_assistant_encrypted_session(
            user_a,
            session_a_new,
            &test_state("cipher-a-new", now + Duration::days(3)),
            now - Duration::minutes(10),
            3600,
        )
        .await
        .expect("newer session insert should succeed");
    store
        .upsert_assistant_encrypted_session(
            user_b,
            session_b,
            &test_state("cipher-b", now + Duration::days(3)),
            now - Duration::minutes(5),
            3600,
        )
        .await
        .expect("user-b session insert should succeed");

    let initial_list = send_json(
        &app,
        request(
            Method::GET,
            "/v1/assistant/sessions",
            Some(auth_a.as_str()),
            None,
        ),
    )
    .await;
    assert_eq!(initial_list.status, StatusCode::OK);
    let initial_list_body: ListAssistantSessionsResponse =
        serde_json::from_value(initial_list.body).expect("list response should decode");
    assert_eq!(initial_list_body.items.len(), 2);
    assert_eq!(initial_list_body.items[0].session_id, session_a_new);
    assert_eq!(initial_list_body.items[1].session_id, session_a_old);

    let cross_user_delete = send_json(
        &app,
        request(
            Method::DELETE,
            &format!("/v1/assistant/sessions/{session_a_old}"),
            Some(auth_b.as_str()),
            None,
        ),
    )
    .await;
    assert_eq!(cross_user_delete.status, StatusCode::NOT_FOUND);
    assert_eq!(error_code(&cross_user_delete.body), Some("not_found"));

    let malformed_delete = send_json(
        &app,
        request(
            Method::DELETE,
            "/v1/assistant/sessions/not-a-uuid",
            Some(auth_a.as_str()),
            None,
        ),
    )
    .await;
    assert_eq!(malformed_delete.status, StatusCode::NOT_FOUND);
    assert_eq!(error_code(&malformed_delete.body), Some("not_found"));

    let single_delete = send_json(
        &app,
        request(
            Method::DELETE,
            &format!("/v1/assistant/sessions/{session_a_old}"),
            Some(auth_a.as_str()),
            None,
        ),
    )
    .await;
    assert_eq!(single_delete.status, StatusCode::OK);
    let single_delete_body: OkResponse =
        serde_json::from_value(single_delete.body).expect("single delete response should decode");
    assert!(single_delete_body.ok);

    let list_after_single_delete = send_json(
        &app,
        request(
            Method::GET,
            "/v1/assistant/sessions",
            Some(auth_a.as_str()),
            None,
        ),
    )
    .await;
    assert_eq!(list_after_single_delete.status, StatusCode::OK);
    let list_after_single_delete_body: ListAssistantSessionsResponse =
        serde_json::from_value(list_after_single_delete.body)
            .expect("post-delete list response should decode");
    assert_eq!(list_after_single_delete_body.items.len(), 1);
    assert_eq!(
        list_after_single_delete_body.items[0].session_id,
        session_a_new
    );

    let delete_all = send_json(
        &app,
        request(
            Method::DELETE,
            "/v1/assistant/sessions",
            Some(auth_a.as_str()),
            None,
        ),
    )
    .await;
    assert_eq!(delete_all.status, StatusCode::OK);
    let delete_all_body: OkResponse =
        serde_json::from_value(delete_all.body).expect("delete-all response should decode");
    assert!(delete_all_body.ok);

    let list_after_delete_all = send_json(
        &app,
        request(
            Method::GET,
            "/v1/assistant/sessions",
            Some(auth_a.as_str()),
            None,
        ),
    )
    .await;
    assert_eq!(list_after_delete_all.status, StatusCode::OK);
    let list_after_delete_all_body: ListAssistantSessionsResponse =
        serde_json::from_value(list_after_delete_all.body)
            .expect("post-delete-all list response should decode");
    assert!(list_after_delete_all_body.items.is_empty());

    let user_b_unchanged = send_json(
        &app,
        request(
            Method::GET,
            "/v1/assistant/sessions",
            Some(auth_b.as_str()),
            None,
        ),
    )
    .await;
    assert_eq!(user_b_unchanged.status, StatusCode::OK);
    let user_b_unchanged_body: ListAssistantSessionsResponse =
        serde_json::from_value(user_b_unchanged.body).expect("user-b list response should decode");
    assert_eq!(user_b_unchanged_body.items.len(), 1);
    assert_eq!(user_b_unchanged_body.items[0].session_id, session_b);
}

fn test_state(
    ciphertext: &str,
    expires_at: chrono::DateTime<Utc>,
) -> AssistantSessionStateEnvelope {
    AssistantSessionStateEnvelope {
        version: "v1".to_string(),
        algorithm: "x25519-chacha20poly1305".to_string(),
        key_id: "assistant-ingress-v1".to_string(),
        nonce: "nonce".to_string(),
        ciphertext: ciphertext.to_string(),
        expires_at,
    }
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

fn request(method: Method, path: &str, bearer: Option<&str>, body: Option<Value>) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header(header::ACCEPT, "application/json");

    if let Some(token) = bearer {
        builder = builder.header(header::AUTHORIZATION, token);
    }

    let request_body = body
        .map(|value| {
            serde_json::to_vec(&value).expect("json body should serialize for integration request")
        })
        .unwrap_or_default();
    if !request_body.is_empty() {
        builder = builder.header(header::CONTENT_TYPE, "application/json");
    }

    builder
        .body(Body::from(request_body))
        .expect("integration request should build")
}

fn error_code(body: &Value) -> Option<&str> {
    body.get("error")?.get("code")?.as_str()
}
