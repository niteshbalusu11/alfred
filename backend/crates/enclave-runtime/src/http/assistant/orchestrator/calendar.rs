use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::Value;
use shared::llm::{
    AssistantCapability, AssistantOutputContract, LlmExecutionSource, LlmGatewayRequest,
    SafeOutputSource, generate_with_telemetry, resolve_safe_output, sanitize_context_payload,
    template_for_capability,
};
use shared::models::{AssistantQueryCapability, AssistantResponsePart};
use tracing::warn;
use uuid::Uuid;

use super::super::mapping::{log_telemetry, map_calendar_event_to_meeting_source};
use super::super::memory::{query_context_snippet, session_memory_context};
use super::super::session_state::EnclaveAssistantSessionState;
use super::AssistantOrchestratorResult;
use super::calendar_fallback::{
    build_calendar_context_payload, compare_meetings_by_start_time, default_display_for_window,
    deterministic_calendar_fallback_payload,
};
use super::calendar_range::plan_calendar_query_window;
use crate::RuntimeState;
use crate::http::rpc;

const CALENDAR_MAX_RESULTS: usize = 20;

pub(super) async fn execute_calendar_query(
    state: &RuntimeState,
    user_id: Uuid,
    request_id: &str,
    query: &str,
    capability: AssistantQueryCapability,
    user_time_zone: &str,
    prior_state: Option<&EnclaveAssistantSessionState>,
) -> Result<AssistantOrchestratorResult, Response> {
    let connector = match state
        .enclave_service
        .resolve_active_google_connector_request(user_id)
        .await
    {
        Ok(connector) => connector,
        Err(err) => {
            return Err(
                rpc::map_rpc_service_error(err, Some(request_id.to_string())).into_response(),
            );
        }
    };

    let window = match plan_calendar_query_window(query, chrono::Utc::now(), user_time_zone) {
        Some(window) => window,
        None => {
            return Err(rpc::reject(
                StatusCode::INTERNAL_SERVER_ERROR,
                shared::enclave::EnclaveRpcErrorEnvelope::new(
                    Some(request_id.to_string()),
                    "rpc_internal_error",
                    "failed to resolve calendar query window",
                    true,
                ),
            )
            .into_response());
        }
    };

    let fetch_response = match state
        .enclave_service
        .fetch_google_calendar_events(
            connector,
            window.time_min.to_rfc3339(),
            window.time_max.to_rfc3339(),
            CALENDAR_MAX_RESULTS,
        )
        .await
    {
        Ok(response) => response,
        Err(err) => {
            return Err(
                rpc::map_rpc_service_error(err, Some(request_id.to_string())).into_response(),
            );
        }
    };

    let mut meetings = fetch_response
        .events
        .iter()
        .map(map_calendar_event_to_meeting_source)
        .collect::<Vec<_>>();
    meetings.sort_by(compare_meetings_by_start_time);

    let mut context_payload = build_calendar_context_payload(&window, &meetings);
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
    .with_requester_id(user_id.to_string());

    let (llm_result, telemetry) = generate_with_telemetry(
        state.llm_gateway.as_ref(),
        LlmExecutionSource::ApiAssistantQuery,
        llm_request,
    )
    .await;
    log_telemetry(user_id, &telemetry, "assistant_query");

    let model_output = match llm_result {
        Ok(response) => response.output,
        Err(err) => {
            warn!(user_id = %user_id, "assistant provider request failed: {err}");
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

    let payload = if resolved.source == SafeOutputSource::DeterministicFallback {
        deterministic_calendar_fallback_payload(&window, &meetings)
    } else {
        let AssistantOutputContract::MeetingsSummary(summary_contract) = resolved.contract else {
            return Err(rpc::reject(
                StatusCode::INTERNAL_SERVER_ERROR,
                shared::enclave::EnclaveRpcErrorEnvelope::new(
                    Some(request_id.to_string()),
                    "rpc_internal_error",
                    "assistant contract resolution failed",
                    true,
                ),
            )
            .into_response());
        };

        shared::models::AssistantStructuredPayload {
            title: summary_contract.output.title,
            summary: summary_contract.output.summary,
            key_points: summary_contract.output.key_points,
            follow_ups: summary_contract.output.follow_ups,
        }
    };

    let display_text = super::super::notifications::non_empty(payload.summary.as_str())
        .unwrap_or(default_display_for_window(&capability, &window))
        .to_string();
    let response_parts = vec![
        AssistantResponsePart::chat_text(display_text.clone()),
        AssistantResponsePart::tool_summary(capability.clone(), payload.clone()),
    ];

    Ok(AssistantOrchestratorResult {
        capability,
        display_text,
        payload,
        response_parts,
        attested_identity: fetch_response.attested_identity,
    })
}
