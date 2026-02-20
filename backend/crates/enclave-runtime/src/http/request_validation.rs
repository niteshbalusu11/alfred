use axum::http::{HeaderMap, StatusCode};
use shared::enclave::{
    ENCLAVE_RPC_CONTRACT_VERSION, EnclaveRpcCompleteGoogleConnectRequest,
    EnclaveRpcExchangeGoogleTokenRequest, EnclaveRpcExecuteAutomationRequest,
    EnclaveRpcFetchAssistantAttestedKeyRequest, EnclaveRpcFetchGoogleCalendarEventsRequest,
    EnclaveRpcFetchGoogleUrgentEmailCandidatesRequest, EnclaveRpcGenerateMorningBriefRequest,
    EnclaveRpcGenerateUrgentEmailSummaryRequest, EnclaveRpcProcessAssistantQueryRequest,
    EnclaveRpcRevokeGoogleTokenRequest,
};

use super::rpc;
use crate::RuntimeState;

pub(super) trait RpcEnvelope {
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

impl RpcEnvelope for EnclaveRpcCompleteGoogleConnectRequest {
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

impl RpcEnvelope for EnclaveRpcFetchAssistantAttestedKeyRequest {
    fn contract_version(&self) -> &str {
        &self.contract_version
    }

    fn request_id(&self) -> &str {
        &self.request_id
    }
}

impl RpcEnvelope for EnclaveRpcProcessAssistantQueryRequest {
    fn contract_version(&self) -> &str {
        &self.contract_version
    }

    fn request_id(&self) -> &str {
        &self.request_id
    }
}

impl RpcEnvelope for EnclaveRpcGenerateMorningBriefRequest {
    fn contract_version(&self) -> &str {
        &self.contract_version
    }

    fn request_id(&self) -> &str {
        &self.request_id
    }
}

impl RpcEnvelope for EnclaveRpcGenerateUrgentEmailSummaryRequest {
    fn contract_version(&self) -> &str {
        &self.contract_version
    }

    fn request_id(&self) -> &str {
        &self.request_id
    }
}

impl RpcEnvelope for EnclaveRpcExecuteAutomationRequest {
    fn contract_version(&self) -> &str {
        &self.contract_version
    }

    fn request_id(&self) -> &str {
        &self.request_id
    }
}

pub(super) fn validate_request<Request>(
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
