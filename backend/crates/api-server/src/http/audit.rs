use axum::Json;
use axum::extract::{Extension, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use shared::models::ListAuditEventsResponse;

use super::errors::store_error_response;
use super::{AppState, AuthUser};

#[derive(serde::Deserialize)]
pub(super) struct AuditEventsQuery {
    cursor: Option<String>,
}

pub(super) async fn list_audit_events(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Query(query): Query<AuditEventsQuery>,
) -> Response {
    match state
        .store
        .list_audit_events(user.user_id, query.cursor.as_deref(), 50)
        .await
    {
        Ok((items, next_cursor)) => (
            StatusCode::OK,
            Json(ListAuditEventsResponse { items, next_cursor }),
        )
            .into_response(),
        Err(err) => store_error_response(err),
    }
}
