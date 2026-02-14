use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use shared::models::{ErrorBody, ErrorResponse};
use shared::repos::StoreError;
use shared::security::SecurityError;
use tracing::{error, warn};

pub(super) fn bad_request_response(code: &str, message: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: ErrorBody {
                code: code.to_string(),
                message: message.to_string(),
            },
        }),
    )
        .into_response()
}

pub(super) fn bad_gateway_response(code: &str, message: &str) -> Response {
    (
        StatusCode::BAD_GATEWAY,
        Json(ErrorResponse {
            error: ErrorBody {
                code: code.to_string(),
                message: message.to_string(),
            },
        }),
    )
        .into_response()
}

pub(super) fn unauthorized_response() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(ErrorResponse {
            error: ErrorBody {
                code: "unauthorized".to_string(),
                message: "Missing or invalid bearer token".to_string(),
            },
        }),
    )
        .into_response()
}

pub(super) fn security_error_response(err: SecurityError) -> Response {
    match err {
        SecurityError::InvalidAttestationDocument(_) => {
            error!("security runtime misconfigured: {err}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: ErrorBody {
                        code: "security_runtime_error".to_string(),
                        message: "Security runtime is misconfigured".to_string(),
                    },
                }),
            )
                .into_response()
        }
        other => {
            warn!("decrypt denied by tee/kms policy: {other}");
            (
                StatusCode::FORBIDDEN,
                Json(ErrorResponse {
                    error: ErrorBody {
                        code: "decrypt_not_authorized".to_string(),
                        message: "Connector decrypt is denied by attestation policy".to_string(),
                    },
                }),
            )
                .into_response()
        }
    }
}

pub(super) fn store_error_response(err: StoreError) -> Response {
    match err {
        StoreError::InvalidCursor => bad_request_response("invalid_cursor", "Cursor is invalid"),
        other => {
            error!("database operation failed: {other}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: ErrorBody {
                        code: "internal_error".to_string(),
                        message: "Unexpected server error".to_string(),
                    },
                }),
            )
                .into_response()
        }
    }
}
