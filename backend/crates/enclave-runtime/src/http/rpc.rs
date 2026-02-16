use axum::Json;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use shared::enclave::{
    ENCLAVE_RPC_AUTH_NONCE_HEADER, ENCLAVE_RPC_AUTH_SIGNATURE_HEADER,
    ENCLAVE_RPC_AUTH_TIMESTAMP_HEADER, ENCLAVE_RPC_CONTRACT_VERSION,
    ENCLAVE_RPC_CONTRACT_VERSION_HEADER, EnclaveRpcError, EnclaveRpcErrorEnvelope,
    sign_rpc_request,
};

pub(super) struct RpcRejection {
    pub(super) status: StatusCode,
    pub(super) body: EnclaveRpcErrorEnvelope,
}

impl RpcRejection {
    pub(super) fn into_response(self) -> Response {
        error_response(self.status, self.body)
    }
}

pub(super) type RpcResult<T> = Result<T, Box<RpcRejection>>;

pub(super) fn reject(status: StatusCode, body: EnclaveRpcErrorEnvelope) -> Box<RpcRejection> {
    Box::new(RpcRejection { status, body })
}

pub(super) fn authorize_request(
    auth: &shared::enclave::EnclaveRpcAuthConfig,
    replay_guard: &std::sync::Mutex<std::collections::HashMap<String, i64>>,
    headers: &HeaderMap,
    path: &str,
    body: &[u8],
) -> RpcResult<()> {
    let contract_header = require_header(headers, ENCLAVE_RPC_CONTRACT_VERSION_HEADER)?;
    if contract_header != ENCLAVE_RPC_CONTRACT_VERSION {
        return Err(reject(
            StatusCode::BAD_REQUEST,
            EnclaveRpcErrorEnvelope::new(
                None,
                "invalid_contract_version",
                "Unsupported enclave RPC contract version",
                false,
            ),
        ));
    }

    let timestamp = require_header(headers, ENCLAVE_RPC_AUTH_TIMESTAMP_HEADER).and_then(|raw| {
        raw.parse::<i64>().map_err(|_| {
            reject(
                StatusCode::UNAUTHORIZED,
                EnclaveRpcErrorEnvelope::new(
                    None,
                    "invalid_request_header",
                    "Invalid request timestamp header",
                    false,
                ),
            )
        })
    })?;

    let nonce = require_header(headers, ENCLAVE_RPC_AUTH_NONCE_HEADER)?;
    if nonce.trim().is_empty() {
        return Err(reject(
            StatusCode::UNAUTHORIZED,
            EnclaveRpcErrorEnvelope::new(
                None,
                "invalid_request_header",
                "Nonce header must not be empty",
                false,
            ),
        ));
    }

    let signature = require_header(headers, ENCLAVE_RPC_AUTH_SIGNATURE_HEADER)?;
    let now = Utc::now().timestamp();
    let max_skew = auth.max_clock_skew_seconds as i64;
    if (now - timestamp).abs() > max_skew {
        return Err(reject(
            StatusCode::UNAUTHORIZED,
            EnclaveRpcErrorEnvelope::new(
                None,
                "invalid_request_timestamp",
                "Request timestamp outside allowed skew",
                false,
            ),
        ));
    }

    let expected_signature =
        sign_rpc_request(&auth.shared_secret, "POST", path, timestamp, &nonce, body);
    if !shared::enclave::constant_time_eq(&expected_signature, &signature) {
        return Err(reject(
            StatusCode::UNAUTHORIZED,
            EnclaveRpcErrorEnvelope::new(
                None,
                "invalid_request_signature",
                "Request signature mismatch",
                false,
            ),
        ));
    }

    let replay_window_expires = timestamp.checked_add(max_skew).ok_or_else(|| {
        reject(
            StatusCode::UNAUTHORIZED,
            EnclaveRpcErrorEnvelope::new(
                None,
                "invalid_request_timestamp",
                "Request timestamp is invalid",
                false,
            ),
        )
    })?;

    let mut replay_guard = replay_guard.lock().map_err(|_| {
        reject(
            StatusCode::INTERNAL_SERVER_ERROR,
            EnclaveRpcErrorEnvelope::new(
                None,
                "rpc_internal_error",
                "Replay guard unavailable",
                true,
            ),
        )
    })?;
    replay_guard.retain(|_, expires_at| *expires_at >= now);

    if replay_guard
        .insert(nonce.to_string(), replay_window_expires)
        .is_some()
    {
        return Err(reject(
            StatusCode::UNAUTHORIZED,
            EnclaveRpcErrorEnvelope::new(
                None,
                "request_replay_detected",
                "Replay detected for RPC nonce",
                false,
            ),
        ));
    }

    Ok(())
}

fn require_header(headers: &HeaderMap, key: &str) -> RpcResult<String> {
    headers
        .get(key)
        .ok_or_else(|| {
            reject(
                StatusCode::UNAUTHORIZED,
                EnclaveRpcErrorEnvelope::new(
                    None,
                    "missing_request_header",
                    format!("Missing required header {key}"),
                    false,
                ),
            )
        })
        .and_then(|value| {
            value.to_str().map(ToString::to_string).map_err(|_| {
                reject(
                    StatusCode::UNAUTHORIZED,
                    EnclaveRpcErrorEnvelope::new(
                        None,
                        "invalid_request_header",
                        format!("Invalid header value for {key}"),
                        false,
                    ),
                )
            })
        })
}

pub(super) fn map_rpc_service_error(
    err: EnclaveRpcError,
    request_id: Option<String>,
) -> (StatusCode, Json<EnclaveRpcErrorEnvelope>) {
    match err {
        EnclaveRpcError::DecryptNotAuthorized { .. } => (
            StatusCode::FORBIDDEN,
            Json(EnclaveRpcErrorEnvelope::new(
                request_id,
                "decrypt_not_authorized",
                "Connector decrypt denied by policy",
                false,
            )),
        ),
        EnclaveRpcError::ConnectorTokenDecryptFailed { .. } => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(EnclaveRpcErrorEnvelope::new(
                request_id,
                "connector_token_decrypt_failed",
                "Connector token decrypt failed",
                true,
            )),
        ),
        EnclaveRpcError::ConnectorTokenUnavailable => (
            StatusCode::BAD_REQUEST,
            Json(EnclaveRpcErrorEnvelope::new(
                request_id,
                "connector_token_unavailable",
                "Connector token metadata changed; retry request",
                false,
            )),
        ),
        EnclaveRpcError::ProviderRequestUnavailable { .. } => (
            StatusCode::BAD_GATEWAY,
            Json(EnclaveRpcErrorEnvelope::new(
                request_id,
                "provider_unavailable",
                "Provider endpoint unavailable",
                true,
            )),
        ),
        EnclaveRpcError::ProviderRequestFailed {
            status,
            oauth_error,
            ..
        } => (
            StatusCode::BAD_GATEWAY,
            Json(EnclaveRpcErrorEnvelope::with_provider_failure(
                request_id,
                status,
                oauth_error,
            )),
        ),
        EnclaveRpcError::ProviderResponseInvalid { .. } => (
            StatusCode::BAD_GATEWAY,
            Json(EnclaveRpcErrorEnvelope::new(
                request_id,
                "provider_response_invalid",
                "Provider response invalid",
                true,
            )),
        ),
        EnclaveRpcError::RpcUnauthorized { code } => (
            StatusCode::UNAUTHORIZED,
            Json(EnclaveRpcErrorEnvelope::new(
                request_id,
                code,
                "RPC request unauthorized",
                false,
            )),
        ),
        EnclaveRpcError::RpcContractRejected { code } => (
            StatusCode::BAD_REQUEST,
            Json(EnclaveRpcErrorEnvelope::new(
                request_id,
                code,
                "RPC request rejected by contract validation",
                false,
            )),
        ),
        EnclaveRpcError::RpcTransportUnavailable { .. }
        | EnclaveRpcError::RpcResponseInvalid { .. } => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(EnclaveRpcErrorEnvelope::new(
                request_id,
                "rpc_internal_error",
                "RPC internal processing failed",
                true,
            )),
        ),
    }
}

pub(super) fn error_response(status: StatusCode, body: EnclaveRpcErrorEnvelope) -> Response {
    (status, Json(body)).into_response()
}
