use std::collections::HashMap;

use axum::Json;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use shared::models::DeleteAllResponse;
use shared::repos::AuditResult;

use super::errors::store_error_response;
use super::{AppState, AuthUser};

pub(super) async fn delete_all(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> Response {
    let request_id = match state.store.queue_delete_all(user.user_id).await {
        Ok(request_id) => request_id,
        Err(err) => return store_error_response(err),
    };

    let mut metadata = HashMap::new();
    metadata.insert("request_id".to_string(), request_id.to_string());

    if let Err(err) = state
        .store
        .add_audit_event(
            user.user_id,
            "PRIVACY_DELETE_ALL_REQUESTED",
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
        Json(DeleteAllResponse {
            request_id: request_id.to_string(),
            status: "QUEUED".to_string(),
        }),
    )
        .into_response()
}
