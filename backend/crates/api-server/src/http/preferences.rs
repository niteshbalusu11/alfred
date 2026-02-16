use std::collections::HashMap;

use axum::Json;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use shared::models::{OkResponse, Preferences};
use shared::repos::AuditResult;

use super::errors::store_error_response;
use super::{AppState, AuthUser};

pub(super) async fn get_preferences(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> Response {
    match state.store.get_or_create_preferences(user.user_id).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => store_error_response(err),
    }
}

pub(super) async fn update_preferences(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Json(req): Json<Preferences>,
) -> Response {
    if let Err(err) = state.store.upsert_preferences(user.user_id, &req).await {
        return store_error_response(err);
    }

    let mut metadata = HashMap::new();
    metadata.insert(
        "meeting_reminder_minutes".to_string(),
        req.meeting_reminder_minutes.to_string(),
    );
    metadata.insert("time_zone".to_string(), req.time_zone.clone());

    if let Err(err) = state
        .store
        .add_audit_event(
            user.user_id,
            "PREFERENCES_UPDATED",
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
