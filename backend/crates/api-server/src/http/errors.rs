use axum::Json;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use shared::models::{ErrorBody, ErrorResponse};
use shared::repos::StoreError;
use tracing::error;

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

pub(super) fn too_many_requests_response(retry_after_seconds: u64) -> Response {
    let mut response = (
        StatusCode::TOO_MANY_REQUESTS,
        Json(ErrorResponse {
            error: ErrorBody {
                code: "rate_limited".to_string(),
                message: "Too many requests; retry later".to_string(),
            },
        }),
    )
        .into_response();

    if let Ok(retry_after_value) = HeaderValue::from_str(&retry_after_seconds.to_string()) {
        response
            .headers_mut()
            .insert(header::RETRY_AFTER, retry_after_value);
    }

    response
}

pub(super) fn decrypt_not_authorized_response() -> Response {
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
