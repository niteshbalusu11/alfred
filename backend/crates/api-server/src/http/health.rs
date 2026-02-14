use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use shared::models::{ErrorBody, ErrorResponse, OkResponse};
use tracing::warn;

use super::AppState;

pub(super) async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, Json(OkResponse { ok: true }))
}

pub(super) async fn readyz(State(state): State<AppState>) -> Response {
    match state.store.ping().await {
        Ok(_) => (StatusCode::OK, Json(OkResponse { ok: true })).into_response(),
        Err(err) => {
            warn!("readiness check failed: {err}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: ErrorBody {
                        code: "db_unavailable".to_string(),
                        message: "Database not ready".to_string(),
                    },
                }),
            )
                .into_response()
        }
    }
}
