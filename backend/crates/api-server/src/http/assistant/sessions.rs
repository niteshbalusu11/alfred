use axum::Json;
use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use shared::models::{
    AssistantSessionSummary, ErrorBody, ErrorResponse, ListAssistantSessionsResponse, OkResponse,
};
use uuid::Uuid;

use super::super::errors::store_error_response;
use super::super::{AppState, AuthUser};

const ASSISTANT_SESSIONS_LIST_LIMIT: i64 = 200;

pub(crate) async fn list_assistant_sessions(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> Response {
    let now = Utc::now();
    let sessions = match state
        .store
        .list_assistant_encrypted_sessions(user.user_id, now, ASSISTANT_SESSIONS_LIST_LIMIT)
        .await
    {
        Ok(sessions) => sessions,
        Err(err) => return store_error_response(err),
    };

    let items = sessions
        .into_iter()
        .map(|session| AssistantSessionSummary {
            session_id: session.session_id,
            created_at: session.created_at,
            updated_at: session.updated_at,
            expires_at: session.expires_at,
        })
        .collect();

    (
        StatusCode::OK,
        Json(ListAssistantSessionsResponse { items }),
    )
        .into_response()
}

pub(crate) async fn delete_assistant_session(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(session_id): Path<String>,
) -> Response {
    let session_id = match Uuid::parse_str(&session_id) {
        Ok(session_id) => session_id,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: ErrorBody {
                        code: "not_found".to_string(),
                        message: "Assistant session not found".to_string(),
                    },
                }),
            )
                .into_response();
        }
    };

    let deleted = match state
        .store
        .delete_assistant_encrypted_session(user.user_id, session_id)
        .await
    {
        Ok(deleted) => deleted,
        Err(err) => return store_error_response(err),
    };

    if deleted {
        return (StatusCode::OK, Json(OkResponse { ok: true })).into_response();
    }

    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: ErrorBody {
                code: "not_found".to_string(),
                message: "Assistant session not found".to_string(),
            },
        }),
    )
        .into_response()
}

pub(crate) async fn delete_all_assistant_sessions(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> Response {
    match state
        .store
        .delete_all_assistant_encrypted_sessions(user.user_id)
        .await
    {
        Ok(_) => (StatusCode::OK, Json(OkResponse { ok: true })).into_response(),
        Err(err) => store_error_response(err),
    }
}
