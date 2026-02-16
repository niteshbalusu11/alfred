use axum::Json;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde_json::{Value, json};
use shared::enclave::{
    ENCLAVE_RPC_CONTRACT_VERSION, ENCLAVE_RPC_PATH_EXCHANGE_GOOGLE_TOKEN,
    ENCLAVE_RPC_PATH_FETCH_GOOGLE_CALENDAR_EVENTS,
    ENCLAVE_RPC_PATH_FETCH_GOOGLE_URGENT_EMAIL_CANDIDATES, ENCLAVE_RPC_PATH_REVOKE_GOOGLE_TOKEN,
    EnclaveRpcExchangeGoogleTokenRequest, EnclaveRpcExchangeGoogleTokenResponse,
    EnclaveRpcFetchGoogleCalendarEventsRequest, EnclaveRpcFetchGoogleCalendarEventsResponse,
    EnclaveRpcFetchGoogleUrgentEmailCandidatesRequest,
    EnclaveRpcFetchGoogleUrgentEmailCandidatesResponse, EnclaveRpcRevokeGoogleTokenRequest,
    EnclaveRpcRevokeGoogleTokenResponse,
};
use shared::enclave_runtime::{AttestationChallengeRequest, AttestationChallengeResponse};

use crate::RuntimeState;

mod rpc;

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
        Err(err) => rpc::map_rpc_service_error(err, Some(request.request_id)).into_response(),
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
        Err(err) => rpc::map_rpc_service_error(err, Some(request.request_id)).into_response(),
    }
}

pub(crate) async fn fetch_google_calendar_events(
    State(state): State<RuntimeState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let request = match validate_request::<EnclaveRpcFetchGoogleCalendarEventsRequest>(
        &state,
        &headers,
        ENCLAVE_RPC_PATH_FETCH_GOOGLE_CALENDAR_EVENTS,
        &body,
    ) {
        Ok(request) => request,
        Err(rejection) => return rejection.into_response(),
    };

    let result = state
        .enclave_service
        .fetch_google_calendar_events(
            request.connector,
            request.time_min,
            request.time_max,
            request.max_results,
        )
        .await;

    match result {
        Ok(fetch_response) => Json(EnclaveRpcFetchGoogleCalendarEventsResponse {
            contract_version: ENCLAVE_RPC_CONTRACT_VERSION.to_string(),
            request_id: request.request_id,
            events: fetch_response.events,
            attested_identity: fetch_response.attested_identity,
        })
        .into_response(),
        Err(err) => rpc::map_rpc_service_error(err, Some(request.request_id)).into_response(),
    }
}

pub(crate) async fn fetch_google_urgent_email_candidates(
    State(state): State<RuntimeState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let request = match validate_request::<EnclaveRpcFetchGoogleUrgentEmailCandidatesRequest>(
        &state,
        &headers,
        ENCLAVE_RPC_PATH_FETCH_GOOGLE_URGENT_EMAIL_CANDIDATES,
        &body,
    ) {
        Ok(request) => request,
        Err(rejection) => return rejection.into_response(),
    };

    let result = state
        .enclave_service
        .fetch_google_urgent_email_candidates(request.connector, request.max_results)
        .await;

    match result {
        Ok(fetch_response) => Json(EnclaveRpcFetchGoogleUrgentEmailCandidatesResponse {
            contract_version: ENCLAVE_RPC_CONTRACT_VERSION.to_string(),
            request_id: request.request_id,
            candidates: fetch_response.candidates,
            attested_identity: fetch_response.attested_identity,
        })
        .into_response(),
        Err(err) => rpc::map_rpc_service_error(err, Some(request.request_id)).into_response(),
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

impl RpcEnvelope for EnclaveRpcFetchGoogleCalendarEventsRequest {
    fn contract_version(&self) -> &str {
        &self.contract_version
    }

    fn request_id(&self) -> &str {
        &self.request_id
    }
}

impl RpcEnvelope for EnclaveRpcFetchGoogleUrgentEmailCandidatesRequest {
    fn contract_version(&self) -> &str {
        &self.contract_version
    }

    fn request_id(&self) -> &str {
        &self.request_id
    }
}

fn validate_request<Request>(
    state: &RuntimeState,
    headers: &HeaderMap,
    path: &str,
    body: &[u8],
) -> rpc::RpcResult<Request>
where
    Request: serde::de::DeserializeOwned + RpcEnvelope,
{
    rpc::authorize_request(
        &state.config.enclave_rpc_auth,
        &state.rpc_replay_guard,
        headers,
        path,
        body,
    )?;

    let request = serde_json::from_slice::<Request>(body).map_err(|_| {
        rpc::reject(
            StatusCode::BAD_REQUEST,
            shared::enclave::EnclaveRpcErrorEnvelope::new(
                None,
                "invalid_request_payload",
                "Request payload is invalid",
                false,
            ),
        )
    })?;

    if request.contract_version() != ENCLAVE_RPC_CONTRACT_VERSION {
        return Err(rpc::reject(
            StatusCode::BAD_REQUEST,
            shared::enclave::EnclaveRpcErrorEnvelope::new(
                Some(request.request_id().to_string()),
                "invalid_contract_version",
                "Unsupported enclave RPC contract version",
                false,
            ),
        ));
    }

    if request.request_id().trim().is_empty() {
        return Err(rpc::reject(
            StatusCode::BAD_REQUEST,
            shared::enclave::EnclaveRpcErrorEnvelope::new(
                None,
                "invalid_request_id",
                "request_id is required",
                false,
            ),
        ));
    }

    Ok(request)
}
