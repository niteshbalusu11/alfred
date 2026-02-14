use std::collections::HashMap;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::{Duration, Utc};
use shared::models::{
    CreateSessionRequest, CreateSessionResponse, ErrorBody, ErrorResponse, OkResponse,
    RefreshSessionRequest, RevokeSessionRequest,
};
use shared::repos::{AuditResult, SessionTokenStatus};
use tracing::warn;
use uuid::Uuid;

use super::AppState;
use super::apple_identity::{AppleIdentityError, verify_identity_token};
use super::errors::{bad_gateway_response, bad_request_response, store_error_response};
use super::tokens::{generate_secure_token, hash_token};

const APPLE_SUBJECT_NAMESPACE: Uuid = Uuid::from_u128(0x6b1e3eecf9d64f3a98611f9922905dc0);

pub(super) async fn create_session(
    State(state): State<AppState>,
    Json(req): Json<CreateSessionRequest>,
) -> Response {
    if req.device_id.trim().is_empty() {
        return bad_request_response("invalid_device_id", "device_id is required");
    }

    let identity = match verify_identity_token(
        &state.http_client,
        &state.apple_ios_audience,
        req.apple_identity_token.trim(),
    )
    .await
    {
        Ok(identity) => identity,
        Err(AppleIdentityError::InvalidToken { code, message }) => {
            return session_error_response(code, message);
        }
        Err(AppleIdentityError::UpstreamUnavailable { code, message }) => {
            return bad_gateway_response(code, message);
        }
    };

    let user_id = user_id_for_apple_subject(&identity.subject);

    if let Err(err) = state.store.ensure_user(user_id).await {
        return store_error_response(err);
    }

    let response = match issue_session_tokens(&state, user_id).await {
        Ok(response) => response,
        Err(err) => return store_error_response(err),
    };

    write_auth_audit_event(
        &state,
        user_id,
        "AUTH_SESSION_CREATED",
        AuditResult::Success,
        "session_create",
        Some("apple_sign_in"),
    )
    .await;

    (StatusCode::OK, Json(response)).into_response()
}

pub(super) async fn refresh_session(
    State(state): State<AppState>,
    Json(req): Json<RefreshSessionRequest>,
) -> Response {
    let refresh_token = req.refresh_token.trim();
    if refresh_token.is_empty() {
        return bad_request_response("invalid_refresh_token", "refresh_token is required");
    }

    let refresh_hash = hash_token(refresh_token);
    let access_token = generate_secure_token("at");
    let refresh_token = generate_secure_token("rt");
    let access_hash = hash_token(&access_token);
    let new_refresh_hash = hash_token(&refresh_token);
    let now = Utc::now();
    let expires_at = now + Duration::seconds(state.session_ttl_seconds as i64);

    let (status, user_id) = match state
        .store
        .rotate_session_by_refresh_token(
            &refresh_hash,
            &access_hash,
            &new_refresh_hash,
            expires_at,
            now,
        )
        .await
    {
        Ok(result) => result,
        Err(err) => return store_error_response(err),
    };

    match status {
        SessionTokenStatus::Active => {
            if let Some(user_id) = user_id {
                write_auth_audit_event(
                    &state,
                    user_id,
                    "AUTH_SESSION_REFRESH",
                    AuditResult::Success,
                    "session_refresh",
                    None,
                )
                .await;
            }
            (
                StatusCode::OK,
                Json(CreateSessionResponse {
                    access_token,
                    refresh_token,
                    expires_in: state.session_ttl_seconds as u32,
                }),
            )
                .into_response()
        }
        SessionTokenStatus::Expired => {
            if let Some(user_id) = user_id {
                write_auth_audit_event(
                    &state,
                    user_id,
                    "AUTH_SESSION_REFRESH",
                    AuditResult::Failure,
                    "session_refresh",
                    Some("expired_refresh_token"),
                )
                .await;
            }
            session_error_response("expired_refresh_token", "Refresh token is expired")
        }
        SessionTokenStatus::Revoked => {
            if let Some(user_id) = user_id {
                write_auth_audit_event(
                    &state,
                    user_id,
                    "AUTH_SESSION_REFRESH",
                    AuditResult::Failure,
                    "session_refresh",
                    Some("revoked_refresh_token"),
                )
                .await;
            }
            session_error_response("revoked_refresh_token", "Refresh token is revoked")
        }
        SessionTokenStatus::NotFound => {
            session_error_response("invalid_refresh_token", "Refresh token is invalid")
        }
    }
}

pub(super) async fn revoke_session(
    State(state): State<AppState>,
    Json(req): Json<RevokeSessionRequest>,
) -> Response {
    let refresh_token = req.refresh_token.trim();
    if refresh_token.is_empty() {
        return bad_request_response("invalid_refresh_token", "refresh_token is required");
    }

    let now = Utc::now();
    let (status, user_id) = match state
        .store
        .revoke_session_by_refresh_token(&hash_token(refresh_token), now)
        .await
    {
        Ok(result) => result,
        Err(err) => return store_error_response(err),
    };

    match status {
        SessionTokenStatus::Active | SessionTokenStatus::Revoked => {
            if let Some(user_id) = user_id {
                let reason = if status == SessionTokenStatus::Revoked {
                    Some("already_revoked")
                } else {
                    None
                };
                write_auth_audit_event(
                    &state,
                    user_id,
                    "AUTH_SESSION_REVOKE",
                    AuditResult::Success,
                    "session_revoke",
                    reason,
                )
                .await;
            }
            (StatusCode::OK, Json(OkResponse { ok: true })).into_response()
        }
        SessionTokenStatus::Expired => {
            if let Some(user_id) = user_id {
                write_auth_audit_event(
                    &state,
                    user_id,
                    "AUTH_SESSION_REVOKE",
                    AuditResult::Failure,
                    "session_revoke",
                    Some("expired_refresh_token"),
                )
                .await;
            }
            session_error_response("expired_refresh_token", "Refresh token is expired")
        }
        SessionTokenStatus::NotFound => {
            session_error_response("invalid_refresh_token", "Refresh token is invalid")
        }
    }
}

async fn issue_session_tokens(
    state: &AppState,
    user_id: Uuid,
) -> Result<CreateSessionResponse, shared::repos::StoreError> {
    let access_token = generate_secure_token("at");
    let refresh_token = generate_secure_token("rt");
    let access_hash = hash_token(&access_token);
    let refresh_hash = hash_token(&refresh_token);
    let expires_in = state.session_ttl_seconds;

    state
        .store
        .create_session(
            user_id,
            &access_hash,
            &refresh_hash,
            Utc::now() + Duration::seconds(expires_in as i64),
        )
        .await?;

    Ok(CreateSessionResponse {
        access_token,
        refresh_token,
        expires_in: expires_in as u32,
    })
}

fn user_id_for_apple_subject(subject: &str) -> Uuid {
    Uuid::new_v5(&APPLE_SUBJECT_NAMESPACE, subject.as_bytes())
}

fn session_error_response(code: &str, message: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(ErrorResponse {
            error: ErrorBody {
                code: code.to_string(),
                message: message.to_string(),
            },
        }),
    )
        .into_response()
}

async fn write_auth_audit_event(
    state: &AppState,
    user_id: Uuid,
    event_type: &str,
    result: AuditResult,
    action: &str,
    reason: Option<&str>,
) {
    let mut metadata = HashMap::new();
    metadata.insert("action".to_string(), action.to_string());
    if let Some(reason) = reason {
        metadata.insert("reason".to_string(), reason.to_string());
    }

    if let Err(err) = state
        .store
        .add_audit_event(user_id, event_type, None, result, &metadata)
        .await
    {
        warn!(
            user_id = %user_id,
            event_type,
            "failed to persist auth audit event: {err}",
        );
    }
}
