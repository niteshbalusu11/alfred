use axum::Json;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use serde::Serialize;
use serde_json::{Value, json};
use shared::enclave::{
    ENCLAVE_RPC_AUTH_NONCE_HEADER, ENCLAVE_RPC_AUTH_SIGNATURE_HEADER,
    ENCLAVE_RPC_AUTH_TIMESTAMP_HEADER, ENCLAVE_RPC_CONTRACT_VERSION,
    ENCLAVE_RPC_CONTRACT_VERSION_HEADER, ENCLAVE_RPC_PATH_EXCHANGE_GOOGLE_TOKEN,
    ENCLAVE_RPC_PATH_REVOKE_GOOGLE_TOKEN, EnclaveRpcError, EnclaveRpcErrorEnvelope,
    EnclaveRpcExchangeGoogleTokenRequest, EnclaveRpcExchangeGoogleTokenResponse,
    EnclaveRpcRevokeGoogleTokenRequest, EnclaveRpcRevokeGoogleTokenResponse, constant_time_eq,
    sign_rpc_request,
};
use shared::enclave_runtime::{AttestationChallengeRequest, AttestationChallengeResponse};

use crate::RuntimeState;

#[cfg(test)]
mod tests;

#[derive(Debug, Serialize)]
pub(crate) struct HealthResponse<'a> {
    status: &'a str,
    environment: &'a str,
    mode: &'a str,
}

pub(crate) async fn healthz(State(state): State<RuntimeState>) -> Json<HealthResponse<'static>> {
    Json(HealthResponse {
        status: "ok",
        environment: state.config.environment.as_str(),
        mode: state.config.mode.as_str(),
    })
}

pub(crate) async fn attestation_document(
    State(state): State<RuntimeState>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    state
        .config
        .attestation_document()
        .map(Json)
        .map_err(|err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "code": "attestation_document_unavailable",
                    "message": err,
                })),
            )
        })
}

pub(crate) async fn attestation_challenge(
    State(state): State<RuntimeState>,
    Json(challenge): Json<AttestationChallengeRequest>,
) -> Result<Json<AttestationChallengeResponse>, (StatusCode, Json<Value>)> {
    state
        .config
        .attestation_challenge_response(challenge)
        .map(Json)
        .map_err(|err| {
            let status = if err.starts_with("invalid challenge") {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            (
                status,
                Json(json!({
                    "code": "attestation_challenge_failed",
                    "message": err,
                })),
            )
        })
}

pub(crate) async fn exchange_google_access_token(
    State(state): State<RuntimeState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let request = match validate_request::<EnclaveRpcExchangeGoogleTokenRequest>(
        &state,
        &headers,
        ENCLAVE_RPC_PATH_EXCHANGE_GOOGLE_TOKEN,
        &body,
    ) {
        Ok(request) => request,
        Err(rejection) => return rejection.into_response(),
    };

    let result = state
        .enclave_service
        .exchange_google_access_token(request.connector)
        .await;

    match result {
        Ok(token_response) => Json(EnclaveRpcExchangeGoogleTokenResponse {
            contract_version: ENCLAVE_RPC_CONTRACT_VERSION.to_string(),
            request_id: request.request_id,
            access_token: token_response.access_token,
            attested_identity: token_response.attested_identity,
        })
        .into_response(),
        Err(err) => map_rpc_service_error(err, Some(request.request_id)).into_response(),
    }
}

pub(crate) async fn revoke_google_connector_token(
    State(state): State<RuntimeState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let request = match validate_request::<EnclaveRpcRevokeGoogleTokenRequest>(
        &state,
        &headers,
        ENCLAVE_RPC_PATH_REVOKE_GOOGLE_TOKEN,
        &body,
    ) {
        Ok(request) => request,
        Err(rejection) => return rejection.into_response(),
    };

    let result = state
        .enclave_service
        .revoke_google_connector_token(request.connector)
        .await;

    match result {
        Ok(token_response) => Json(EnclaveRpcRevokeGoogleTokenResponse {
            contract_version: ENCLAVE_RPC_CONTRACT_VERSION.to_string(),
            request_id: request.request_id,
            attested_identity: token_response.attested_identity,
        })
        .into_response(),
        Err(err) => map_rpc_service_error(err, Some(request.request_id)).into_response(),
    }
}

trait RpcEnvelope {
    fn contract_version(&self) -> &str;
    fn request_id(&self) -> &str;
}

impl RpcEnvelope for EnclaveRpcExchangeGoogleTokenRequest {
    fn contract_version(&self) -> &str {
        &self.contract_version
    }

    fn request_id(&self) -> &str {
        &self.request_id
    }
}

impl RpcEnvelope for EnclaveRpcRevokeGoogleTokenRequest {
    fn contract_version(&self) -> &str {
        &self.contract_version
    }

    fn request_id(&self) -> &str {
        &self.request_id
    }
}

struct RpcRejection {
    status: StatusCode,
    body: EnclaveRpcErrorEnvelope,
}

impl RpcRejection {
    fn into_response(self) -> Response {
        error_response(self.status, self.body)
    }
}

type RpcResult<T> = Result<T, Box<RpcRejection>>;

fn reject(status: StatusCode, body: EnclaveRpcErrorEnvelope) -> Box<RpcRejection> {
    Box::new(RpcRejection { status, body })
}

fn validate_request<Request>(
    state: &RuntimeState,
    headers: &HeaderMap,
    path: &str,
    body: &[u8],
) -> RpcResult<Request>
where
    Request: serde::de::DeserializeOwned + RpcEnvelope,
{
    authorize_request(
        &state.config.enclave_rpc_auth,
        &state.rpc_replay_guard,
        headers,
        path,
        body,
    )?;

    let request = serde_json::from_slice::<Request>(body).map_err(|_| {
        reject(
            StatusCode::BAD_REQUEST,
            EnclaveRpcErrorEnvelope::new(
                None,
                "invalid_request_payload",
                "Request payload is invalid",
                false,
            ),
        )
    })?;

    if request.contract_version() != ENCLAVE_RPC_CONTRACT_VERSION {
        return Err(reject(
            StatusCode::BAD_REQUEST,
            EnclaveRpcErrorEnvelope::new(
                Some(request.request_id().to_string()),
                "invalid_contract_version",
                "Unsupported enclave RPC contract version",
                false,
            ),
        ));
    }

    if request.request_id().trim().is_empty() {
        return Err(reject(
            StatusCode::BAD_REQUEST,
            EnclaveRpcErrorEnvelope::new(
                None,
                "invalid_request_id",
                "request_id is required",
                false,
            ),
        ));
    }

    Ok(request)
}

fn authorize_request(
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
    if !constant_time_eq(&expected_signature, &signature) {
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

fn map_rpc_service_error(
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

fn error_response(status: StatusCode, body: EnclaveRpcErrorEnvelope) -> Response {
    (status, Json(body)).into_response()
}
