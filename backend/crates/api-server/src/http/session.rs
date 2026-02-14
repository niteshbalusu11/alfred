use std::collections::HashMap;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::{Duration, Utc};
use shared::models::{CreateSessionRequest, CreateSessionResponse};
use shared::repos::AuditResult;

use super::AppState;
use super::errors::store_error_response;
use super::tokens::{generate_secure_token, hash_token};

pub(super) async fn create_session(
    State(state): State<AppState>,
    Json(_req): Json<CreateSessionRequest>,
) -> Response {
    let user_id = match state.store.create_user().await {
        Ok(user_id) => user_id,
        Err(err) => return store_error_response(err),
    };

    let access_token = generate_secure_token("at");
    let refresh_token = generate_secure_token("rt");
    let access_hash = hash_token(&access_token);
    let refresh_hash = hash_token(&refresh_token);
    let expires_in = state.session_ttl_seconds;

    if let Err(err) = state
        .store
        .create_session(
            user_id,
            &access_hash,
            &refresh_hash,
            Utc::now() + Duration::seconds(expires_in as i64),
        )
        .await
    {
        return store_error_response(err);
    }

    let mut metadata = HashMap::new();
    metadata.insert("action".to_string(), "session_created".to_string());

    if let Err(err) = state
        .store
        .add_audit_event(
            user_id,
            "AUTH_SESSION_CREATED",
            None,
            AuditResult::Success,
            &metadata,
        )
        .await
    {
        return store_error_response(err);
    }

    let response = CreateSessionResponse {
        access_token,
        refresh_token,
        expires_in: expires_in as u32,
    };

    (StatusCode::OK, Json(response)).into_response()
}
