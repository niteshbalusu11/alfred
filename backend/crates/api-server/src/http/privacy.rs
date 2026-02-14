use std::collections::HashMap;

use axum::Json;
use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use shared::models::{DeleteAllResponse, DeleteAllStatusResponse, ErrorBody, ErrorResponse};
use shared::repos::AuditResult;
use uuid::Uuid;

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

pub(super) async fn get_delete_all_status(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(request_id): Path<String>,
) -> Response {
    let request_id = match Uuid::parse_str(&request_id) {
        Ok(request_id) => request_id,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: ErrorBody {
                        code: "not_found".to_string(),
                        message: "Delete request not found".to_string(),
                    },
                }),
            )
                .into_response();
        }
    };

    let delete_status = match state
        .store
        .get_delete_request_status(user.user_id, request_id)
        .await
    {
        Ok(Some(delete_status)) => delete_status,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: ErrorBody {
                        code: "not_found".to_string(),
                        message: "Delete request not found".to_string(),
                    },
                }),
            )
                .into_response();
        }
        Err(err) => return store_error_response(err),
    };

    (
        StatusCode::OK,
        Json(DeleteAllStatusResponse {
            request_id: delete_status.id.to_string(),
            status: delete_status.status.as_str().to_string(),
            created_at: delete_status.created_at,
            started_at: delete_status.started_at,
            completed_at: delete_status.completed_at,
            failed_at: delete_status.failed_at,
        }),
    )
        .into_response()
}
