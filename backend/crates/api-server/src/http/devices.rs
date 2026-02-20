use std::collections::HashMap;

use axum::Json;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use base64::Engine as _;
use chrono::Utc;
use serde_json::json;
use shared::assistant_crypto::ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305;
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
    if let Some(response) = validate_notification_key_fields(&req) {
        return response;
    }
    let notification_key_algorithm = normalized_optional(req.notification_key_algorithm.as_deref());
    let notification_public_key = normalized_optional(req.notification_public_key.as_deref());

    if let Err(err) = state
        .store
        .register_device(
            user.user_id,
            &req.device_id,
            &req.apns_token,
            &req.environment,
            notification_key_algorithm.as_deref(),
            notification_public_key.as_deref(),
        )
        .await
    {
        return store_error_response(err);
    }

    let mut metadata = HashMap::new();
    metadata.insert("device_id".to_string(), req.device_id);
    metadata.insert(
        "notification_key_registered".to_string(),
        notification_public_key.is_some().to_string(),
    );

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

fn validate_notification_key_fields(req: &RegisterDeviceRequest) -> Option<Response> {
    let has_algorithm = req
        .notification_key_algorithm
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let has_public_key = req
        .notification_public_key
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());

    if !has_algorithm && !has_public_key {
        return None;
    }

    if !has_algorithm || !has_public_key {
        return Some(bad_request_response(
            "invalid_notification_key",
            "notification_key_algorithm and notification_public_key must both be provided",
        ));
    }

    let algorithm = req
        .notification_key_algorithm
        .as_deref()
        .unwrap_or_default()
        .trim();
    if algorithm != ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305 {
        return Some(bad_request_response(
            "invalid_notification_key_algorithm",
            "notification_key_algorithm is not supported",
        ));
    }

    let public_key_b64 = req
        .notification_public_key
        .as_deref()
        .unwrap_or_default()
        .trim();
    let decoded = match base64::engine::general_purpose::STANDARD.decode(public_key_b64) {
        Ok(bytes) => bytes,
        Err(_) => {
            return Some(bad_request_response(
                "invalid_notification_public_key",
                "notification_public_key must be valid base64",
            ));
        }
    };
    if decoded.len() != 32 {
        return Some(bad_request_response(
            "invalid_notification_public_key",
            "notification_public_key must decode to 32 bytes",
        ));
    }

    None
}

fn normalized_optional(value: Option<&str>) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}
