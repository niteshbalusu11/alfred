use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use shared::assistant_crypto::{decrypt_assistant_request, encrypt_assistant_response};
use shared::assistant_memory::ASSISTANT_SESSION_MEMORY_VERSION_V1;
use shared::enclave::{
    ENCLAVE_RPC_CONTRACT_VERSION, EnclaveRpcProcessAssistantQueryRequest,
    EnclaveRpcProcessAssistantQueryResponse,
};
use shared::models::AssistantPlaintextQueryResponse;
use uuid::Uuid;

use super::memory::build_updated_memory;
use super::orchestrator;
use super::session_state::{
    EnclaveAssistantSessionState, decrypt_session_state, encrypt_session_state,
};
use crate::RuntimeState;
use crate::http::rpc;

pub(super) async fn process_assistant_query(
    state: RuntimeState,
    request: EnclaveRpcProcessAssistantQueryRequest,
) -> Response {
    let request_id = request.request_id.clone();

    let (plaintext, selected_key) =
        match decrypt_assistant_request(&state.config.assistant_ingress_keys, &request.envelope) {
            Ok(result) => result,
            Err(err) => {
                return rpc::reject(
                    StatusCode::BAD_REQUEST,
                    shared::enclave::EnclaveRpcErrorEnvelope::new(
                        Some(request_id),
                        "invalid_request_payload",
                        format!("assistant envelope decrypt failed: {err}"),
                        false,
                    ),
                )
                .into_response();
            }
        };

    let query = plaintext.query.trim();
    if query.is_empty() {
        return rpc::reject(
            StatusCode::BAD_REQUEST,
            shared::enclave::EnclaveRpcErrorEnvelope::new(
                Some(request.request_id),
                "invalid_request_payload",
                "assistant query must not be empty",
                false,
            ),
        )
        .into_response();
    }

    if let (Some(request_session_id), Some(plaintext_session_id)) =
        (request.session_id, plaintext.session_id)
        && request_session_id != plaintext_session_id
    {
        return rpc::reject(
            StatusCode::BAD_REQUEST,
            shared::enclave::EnclaveRpcErrorEnvelope::new(
                Some(request.request_id),
                "invalid_request_payload",
                "session_id mismatch between envelope metadata and plaintext payload",
                false,
            ),
        )
        .into_response();
    }

    let now = Utc::now();
    let prior_state = match request.prior_session_state.as_ref() {
        Some(prior_state) => {
            let session_id = match request.session_id {
                Some(value) => value,
                None => {
                    return rpc::reject(
                        StatusCode::BAD_REQUEST,
                        shared::enclave::EnclaveRpcErrorEnvelope::new(
                            Some(request.request_id),
                            "invalid_request_payload",
                            "prior_session_state requires session_id",
                            false,
                        ),
                    )
                    .into_response();
                }
            };

            match decrypt_session_state(&state, prior_state, request.user_id, session_id, now) {
                Ok(prior) => Some(prior),
                Err(err) => {
                    return rpc::reject(
                        StatusCode::BAD_REQUEST,
                        shared::enclave::EnclaveRpcErrorEnvelope::new(
                            Some(request.request_id),
                            "invalid_request_payload",
                            err,
                            false,
                        ),
                    )
                    .into_response();
                }
            }
        }
        None => None,
    };

    let session_id = request
        .session_id
        .or(plaintext.session_id)
        .unwrap_or_else(Uuid::new_v4);

    let execution = match orchestrator::execute_query(
        &state,
        request.user_id,
        request.request_id.as_str(),
        query,
        prior_state.as_ref(),
    )
    .await
    {
        Ok(execution) => execution,
        Err(response) => return response,
    };

    let response_contract = AssistantPlaintextQueryResponse {
        session_id,
        capability: execution.capability.clone(),
        display_text: execution.display_text.clone(),
        payload: execution.payload,
    };

    let encrypted_response = match encrypt_assistant_response(
        &selected_key,
        request.envelope.request_id.as_str(),
        request.envelope.client_ephemeral_public_key.as_str(),
        &response_contract,
    ) {
        Ok(envelope) => envelope,
        Err(err) => {
            return rpc::reject(
                StatusCode::BAD_REQUEST,
                shared::enclave::EnclaveRpcErrorEnvelope::new(
                    Some(request.request_id),
                    "invalid_request_payload",
                    format!("assistant response encryption failed: {err}"),
                    false,
                ),
            )
            .into_response();
        }
    };

    let updated_memory = build_updated_memory(
        prior_state.as_ref().map(|state| &state.memory),
        query,
        response_contract.display_text.as_str(),
        execution.capability.clone(),
        now,
    );
    let encrypted_session_state = match encrypt_session_state(
        &state,
        &EnclaveAssistantSessionState {
            version: ASSISTANT_SESSION_MEMORY_VERSION_V1.to_string(),
            last_capability: execution.capability,
            memory: updated_memory,
        },
        request.user_id,
        session_id,
        now,
    ) {
        Ok(session_state) => session_state,
        Err(err) => {
            return rpc::reject(
                StatusCode::INTERNAL_SERVER_ERROR,
                shared::enclave::EnclaveRpcErrorEnvelope::new(
                    Some(request.request_id),
                    "rpc_internal_error",
                    err,
                    true,
                ),
            )
            .into_response();
        }
    };

    Json(EnclaveRpcProcessAssistantQueryResponse {
        contract_version: ENCLAVE_RPC_CONTRACT_VERSION.to_string(),
        request_id: request.request_id,
        session_id,
        envelope: encrypted_response,
        session_state: Some(encrypted_session_state),
        attested_identity: execution.attested_identity,
    })
    .into_response()
}
