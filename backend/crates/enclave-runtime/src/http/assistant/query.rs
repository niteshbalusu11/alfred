use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::{Duration, Utc};
use serde_json::Value;
use shared::assistant_crypto::{decrypt_assistant_request, encrypt_assistant_response};
use shared::assistant_memory::ASSISTANT_SESSION_MEMORY_VERSION_V1;
use shared::enclave::{
    ENCLAVE_RPC_CONTRACT_VERSION, EnclaveRpcProcessAssistantQueryRequest,
    EnclaveRpcProcessAssistantQueryResponse,
};
use shared::llm::{
    AssistantCapability, AssistantOutputContract, LlmExecutionSource, LlmGatewayRequest,
    assemble_meetings_today_context, generate_with_telemetry, resolve_safe_output,
    sanitize_context_payload, template_for_capability,
};
use shared::models::{AssistantMeetingsTodayPayload, AssistantPlaintextQueryResponse};
use tracing::warn;
use uuid::Uuid;

use super::mapping::{log_telemetry, map_calendar_event_to_meeting_source};
use super::memory::{
    build_updated_memory, detect_query_capability, query_context_snippet, resolve_query_capability,
    session_memory_context,
};
use super::notifications::non_empty;
use super::session_state::{
    EnclaveAssistantSessionState, decrypt_session_state, encrypt_session_state,
};
use crate::RuntimeState;
use crate::http::rpc;

const CALENDAR_MAX_RESULTS: usize = 20;

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

    let detected_capability = detect_query_capability(query);
    let capability = resolve_query_capability(
        query,
        detected_capability,
        prior_state
            .as_ref()
            .map(|state| state.last_capability.clone()),
    )
    .unwrap_or(shared::models::AssistantQueryCapability::MeetingsToday);

    let connector = match state
        .enclave_service
        .resolve_active_google_connector_request(request.user_id)
        .await
    {
        Ok(connector) => connector,
        Err(err) => {
            return rpc::map_rpc_service_error(err, Some(request.request_id)).into_response();
        }
    };

    let calendar_day = Utc::now().date_naive();
    let time_min = match calendar_day.and_hms_opt(0, 0, 0) {
        Some(start) => start.and_utc(),
        None => {
            return rpc::reject(
                StatusCode::INTERNAL_SERVER_ERROR,
                shared::enclave::EnclaveRpcErrorEnvelope::new(
                    Some(request.request_id),
                    "rpc_internal_error",
                    "failed to resolve calendar day window",
                    true,
                ),
            )
            .into_response();
        }
    };
    let time_max = time_min + Duration::days(1);

    let fetch_response = match state
        .enclave_service
        .fetch_google_calendar_events(
            connector,
            time_min.to_rfc3339(),
            time_max.to_rfc3339(),
            CALENDAR_MAX_RESULTS,
        )
        .await
    {
        Ok(response) => response,
        Err(err) => {
            return rpc::map_rpc_service_error(err, Some(request.request_id)).into_response();
        }
    };

    let meetings = fetch_response
        .events
        .iter()
        .map(map_calendar_event_to_meeting_source)
        .collect::<Vec<_>>();
    let meetings_context = assemble_meetings_today_context(calendar_day, &meetings);

    let mut context_payload = match serde_json::to_value(&meetings_context) {
        Ok(value) => value,
        Err(_) => {
            return rpc::reject(
                StatusCode::INTERNAL_SERVER_ERROR,
                shared::enclave::EnclaveRpcErrorEnvelope::new(
                    Some(request.request_id),
                    "rpc_internal_error",
                    "failed to serialize meetings context",
                    true,
                ),
            )
            .into_response();
        }
    };

    if let Value::Object(entries) = &mut context_payload {
        entries.insert(
            "query_context".to_string(),
            Value::String(query_context_snippet(query)),
        );
        if let Some(memory_context) =
            session_memory_context(prior_state.as_ref().map(|state| &state.memory))
        {
            entries.insert("session_memory".to_string(), memory_context);
        }
    }

    let context_payload = sanitize_context_payload(&context_payload);
    let llm_request = LlmGatewayRequest::from_template(
        template_for_capability(AssistantCapability::MeetingsSummary),
        context_payload.clone(),
    )
    .with_requester_id(request.user_id.to_string());

    let (llm_result, telemetry) = generate_with_telemetry(
        state.llm_gateway.as_ref(),
        LlmExecutionSource::ApiAssistantQuery,
        llm_request,
    )
    .await;
    log_telemetry(request.user_id, &telemetry, "assistant_query");

    let model_output = match llm_result {
        Ok(response) => response.output,
        Err(err) => {
            warn!(user_id = %request.user_id, "assistant provider request failed: {err}");
            Value::Null
        }
    };

    let resolved = resolve_safe_output(
        AssistantCapability::MeetingsSummary,
        if model_output.is_null() {
            None
        } else {
            Some(&model_output)
        },
        &context_payload,
    );

    let AssistantOutputContract::MeetingsSummary(summary_contract) = resolved.contract else {
        return rpc::reject(
            StatusCode::INTERNAL_SERVER_ERROR,
            shared::enclave::EnclaveRpcErrorEnvelope::new(
                Some(request.request_id),
                "rpc_internal_error",
                "assistant contract resolution failed",
                true,
            ),
        )
        .into_response();
    };

    let display_text = non_empty(summary_contract.output.summary.as_str())
        .unwrap_or("Here are your meetings for today.")
        .to_string();

    let response_contract = AssistantPlaintextQueryResponse {
        session_id,
        capability: capability.clone(),
        display_text: display_text.clone(),
        payload: AssistantMeetingsTodayPayload {
            title: summary_contract.output.title,
            summary: summary_contract.output.summary,
            key_points: summary_contract.output.key_points,
            follow_ups: summary_contract.output.follow_ups,
        },
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
        capability.clone(),
        now,
    );
    let encrypted_session_state = match encrypt_session_state(
        &state,
        &EnclaveAssistantSessionState {
            version: ASSISTANT_SESSION_MEMORY_VERSION_V1.to_string(),
            last_capability: capability,
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
        attested_identity: fetch_response.attested_identity,
    })
    .into_response()
}
