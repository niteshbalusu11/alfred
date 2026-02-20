use std::collections::HashMap;

use axum::Json;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use serde_json::json;
use shared::models::{
    OkResponse, RegisterDeviceRequest, SendTestNotificationRequest, SendTestNotificationResponse,
};
use shared::repos::{AuditResult, JobType};
use uuid::Uuid;

use super::errors::{bad_request_response, store_error_response};
use super::observability::RequestContext;
use super::{AppState, AuthUser};

pub(super) async fn register_device(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Json(req): Json<RegisterDeviceRequest>,
) -> Response {
    if let Err(err) = state
        .store
        .register_device(
            user.user_id,
            &req.device_id,
            &req.apns_token,
            &req.environment,
        )
        .await
    {
        return store_error_response(err);
    }

    let mut metadata = HashMap::new();
    metadata.insert("device_id".to_string(), req.device_id);

    if let Err(err) = state
        .store
        .add_audit_event(
            user.user_id,
            "DEVICE_REGISTERED",
            None,
            AuditResult::Success,
            &metadata,
        )
        .await
    {
        return store_error_response(err);
    }

    (StatusCode::OK, Json(OkResponse { ok: true })).into_response()
}

pub(super) async fn send_test_notification(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Extension(request_context): Extension<RequestContext>,
    Json(req): Json<SendTestNotificationRequest>,
) -> Response {
    match state.store.has_registered_device(user.user_id).await {
        Ok(true) => {}
        Ok(false) => {
            return bad_request_response(
                "no_registered_device",
                "Register an APNs device before requesting a test notification",
            );
        }
        Err(err) => return store_error_response(err),
    }

    let title = req
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Alfred test notification");
    let body = req
        .body
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("This notification confirms your push pipeline is active.");

    if title.chars().count() > 120 {
        return bad_request_response(
            "invalid_title",
            "Notification title must be at most 120 characters",
        );
    }

    if body.chars().count() > 500 {
        return bad_request_response(
            "invalid_body",
            "Notification body must be at most 500 characters",
        );
    }

    let payload = super::observability::attach_request_trace(
        json!({
            "notification": {
                "title": title,
                "body": body
            }
        }),
        &request_context.request_id,
    );

    let idempotency_key = format!("TEST_NOTIFICATION:{}", Uuid::new_v4());
    let job_id = match state
        .store
        .enqueue_job_with_idempotency_key(
            user.user_id,
            JobType::AutomationRun,
            Utc::now(),
            Some(&payload),
            &idempotency_key,
        )
        .await
    {
        Ok(job_id) => job_id,
        Err(err) => return store_error_response(err),
    };

    let mut metadata = HashMap::new();
    metadata.insert("job_id".to_string(), job_id.to_string());
    metadata.insert(
        "job_type".to_string(),
        JobType::AutomationRun.as_str().to_string(),
    );

    if let Err(err) = state
        .store
        .add_audit_event(
            user.user_id,
            "TEST_NOTIFICATION_QUEUED",
            None,
            AuditResult::Success,
            &metadata,
        )
        .await
    {
        return store_error_response(err);
    }

    (
        StatusCode::OK,
        Json(SendTestNotificationResponse {
            queued_job_id: job_id.to_string(),
            status: "QUEUED".to_string(),
        }),
    )
        .into_response()
}
