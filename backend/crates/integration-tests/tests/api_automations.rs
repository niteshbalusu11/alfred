mod support;

use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode, header};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use serial_test::serial;
use tower::ServiceExt;

use support::api_app::build_test_router;
use support::clerk::TestClerkAuth;

#[tokio::test]
#[serial]
async fn automation_crud_flow_succeeds_for_owner() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let clerk = TestClerkAuth::start().await;
    let auth = format!("Bearer {}", clerk.token_for_subject("automation-owner"));
    let app = build_test_router(store, &clerk).await;

    let create = send_json(
        &app,
        request(
            Method::POST,
            "/v1/automations",
            Some(&auth),
            Some(json!({
                "schedule": schedule_payload("DAILY", "UTC", "09:00"),
                "prompt_envelope": prompt_envelope("create-request")
            })),
        ),
    )
    .await;
    assert_eq!(create.status, StatusCode::OK);
    assert_eq!(
        create.body.get("status").and_then(Value::as_str),
        Some("ACTIVE")
    );
    assert_eq!(
        create
            .body
            .get("schedule")
            .and_then(|value| value.get("schedule_type"))
            .and_then(Value::as_str),
        Some("DAILY")
    );
    let rule_id = create
        .body
        .get("rule_id")
        .and_then(Value::as_str)
        .expect("create response should include rule_id")
        .to_string();

    let list = send_json(
        &app,
        request(Method::GET, "/v1/automations", Some(&auth), None),
    )
    .await;
    assert_eq!(list.status, StatusCode::OK);
    let items = list
        .body
        .get("items")
        .and_then(Value::as_array)
        .expect("list response should include items");
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].get("rule_id").and_then(Value::as_str),
        Some(rule_id.as_str())
    );

    let update_schedule = send_json(
        &app,
        request(
            Method::PATCH,
            &format!("/v1/automations/{rule_id}"),
            Some(&auth),
            Some(json!({
                "schedule": schedule_payload("WEEKLY", "America/New_York", "10:30")
            })),
        ),
    )
    .await;
    assert_eq!(update_schedule.status, StatusCode::OK);
    assert_eq!(
        update_schedule
            .body
            .get("schedule")
            .and_then(|value| value.get("schedule_type"))
            .and_then(Value::as_str),
        Some("WEEKLY")
    );
    assert_eq!(
        update_schedule
            .body
            .get("schedule")
            .and_then(|value| value.get("local_time"))
            .and_then(Value::as_str),
        Some("10:30")
    );

    let pause = send_json(
        &app,
        request(
            Method::PATCH,
            &format!("/v1/automations/{rule_id}"),
            Some(&auth),
            Some(json!({"status": "PAUSED"})),
        ),
    )
    .await;
    assert_eq!(pause.status, StatusCode::OK);
    assert_eq!(
        pause.body.get("status").and_then(Value::as_str),
        Some("PAUSED")
    );

    let resume = send_json(
        &app,
        request(
            Method::PATCH,
            &format!("/v1/automations/{rule_id}"),
            Some(&auth),
            Some(json!({"status": "ACTIVE"})),
        ),
    )
    .await;
    assert_eq!(resume.status, StatusCode::OK);
    assert_eq!(
        resume.body.get("status").and_then(Value::as_str),
        Some("ACTIVE")
    );

    let update_prompt = send_json(
        &app,
        request(
            Method::PATCH,
            &format!("/v1/automations/{rule_id}"),
            Some(&auth),
            Some(json!({"prompt_envelope": prompt_envelope("update-request")})),
        ),
    )
    .await;
    assert_eq!(update_prompt.status, StatusCode::OK);

    let debug_run = send_json(
        &app,
        request(
            Method::POST,
            &format!("/v1/automations/{rule_id}/debug/run"),
            Some(&auth),
            None,
        ),
    )
    .await;
    assert_eq!(
        debug_run.status,
        StatusCode::OK,
        "debug run response body: {}",
        debug_run.body
    );
    assert_eq!(
        debug_run.body.get("status").and_then(Value::as_str),
        Some("QUEUED")
    );

    let deleted = send_json(
        &app,
        request(
            Method::DELETE,
            &format!("/v1/automations/{rule_id}"),
            Some(&auth),
            None,
        ),
    )
    .await;
    assert_eq!(deleted.status, StatusCode::OK);
    assert_eq!(deleted.body.get("ok").and_then(Value::as_bool), Some(true));

    let list_after_delete = send_json(
        &app,
        request(Method::GET, "/v1/automations", Some(&auth), None),
    )
    .await;
    assert_eq!(list_after_delete.status, StatusCode::OK);
    let items_after_delete = list_after_delete
        .body
        .get("items")
        .and_then(Value::as_array)
        .expect("list response should include items");
    assert!(items_after_delete.is_empty());
}

#[tokio::test]
#[serial]
async fn automation_create_rejects_invalid_schedule() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let clerk = TestClerkAuth::start().await;
    let auth = format!(
        "Bearer {}",
        clerk.token_for_subject("automation-invalid-schedule")
    );
    let app = build_test_router(store, &clerk).await;

    let response = send_json(
        &app,
        request(
            Method::POST,
            "/v1/automations",
            Some(&auth),
            Some(json!({
                "schedule": schedule_payload("DAILY", "UTC", "25:00"),
                "prompt_envelope": prompt_envelope("invalid-schedule")
            })),
        ),
    )
    .await;

    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    assert_eq!(error_code(&response.body), Some("invalid_local_time"));
}

#[tokio::test]
#[serial]
async fn automation_create_rejects_invalid_envelope() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let clerk = TestClerkAuth::start().await;
    let auth = format!(
        "Bearer {}",
        clerk.token_for_subject("automation-invalid-envelope")
    );
    let app = build_test_router(store, &clerk).await;

    let mut invalid = prompt_envelope("invalid-envelope");
    invalid["nonce"] = json!("not-base64");
    let response = send_json(
        &app,
        request(
            Method::POST,
            "/v1/automations",
            Some(&auth),
            Some(json!({
                "schedule": schedule_payload("DAILY", "UTC", "09:00"),
                "prompt_envelope": invalid
            })),
        ),
    )
    .await;

    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    assert_eq!(error_code(&response.body), Some("invalid_nonce"));
}

#[tokio::test]
#[serial]
async fn automation_mutations_are_user_scoped() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let clerk = TestClerkAuth::start().await;
    let auth_a = format!("Bearer {}", clerk.token_for_subject("automation-owner-a"));
    let auth_b = format!("Bearer {}", clerk.token_for_subject("automation-owner-b"));
    let app = build_test_router(store, &clerk).await;

    let create = send_json(
        &app,
        request(
            Method::POST,
            "/v1/automations",
            Some(&auth_a),
            Some(json!({
                "schedule": schedule_payload("DAILY", "UTC", "09:00"),
                "prompt_envelope": prompt_envelope("owner-a")
            })),
        ),
    )
    .await;
    assert_eq!(create.status, StatusCode::OK);
    let rule_id = create
        .body
        .get("rule_id")
        .and_then(Value::as_str)
        .expect("create response should include rule_id");

    let update_other_user = send_json(
        &app,
        request(
            Method::PATCH,
            &format!("/v1/automations/{rule_id}"),
            Some(&auth_b),
            Some(json!({"status": "PAUSED"})),
        ),
    )
    .await;
    assert_eq!(update_other_user.status, StatusCode::NOT_FOUND);

    let delete_other_user = send_json(
        &app,
        request(
            Method::DELETE,
            &format!("/v1/automations/{rule_id}"),
            Some(&auth_b),
            None,
        ),
    )
    .await;
    assert_eq!(delete_other_user.status, StatusCode::NOT_FOUND);

    let debug_other_user = send_json(
        &app,
        request(
            Method::POST,
            &format!("/v1/automations/{rule_id}/debug/run"),
            Some(&auth_b),
            None,
        ),
    )
    .await;
    assert_eq!(debug_other_user.status, StatusCode::NOT_FOUND);
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

fn prompt_envelope(request_id: &str) -> Value {
    json!({
        "version": "v1",
        "algorithm": "x25519-chacha20poly1305",
        "key_id": "assistant-ingress-v1",
        "request_id": request_id,
        "client_ephemeral_public_key": STANDARD.encode([7_u8; 32]),
        "nonce": STANDARD.encode([9_u8; 12]),
        "ciphertext": STANDARD.encode(b"encrypted-automation-prompt")
    })
}

fn schedule_payload(schedule_type: &str, time_zone: &str, local_time: &str) -> Value {
    json!({
        "schedule_type": schedule_type,
        "time_zone": time_zone,
        "local_time": local_time
    })
}

fn error_code(body: &Value) -> Option<&str> {
    body.get("error")
        .and_then(|error| error.get("code"))
        .and_then(Value::as_str)
}
