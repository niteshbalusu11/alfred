use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::{Duration, Utc};
use serde_json::Value;
use shared::llm::{
    AssistantCapability, AssistantOutputContract, LlmExecutionSource, LlmGatewayRequest,
    assemble_meetings_today_context, generate_with_telemetry, resolve_safe_output,
    sanitize_context_payload, template_for_capability,
};
use shared::models::{AssistantQueryCapability, AssistantStructuredPayload};
use tracing::warn;
use uuid::Uuid;

use super::super::mapping::{log_telemetry, map_calendar_event_to_meeting_source};
use super::super::memory::{query_context_snippet, session_memory_context};
use super::super::notifications::non_empty;
use super::super::session_state::EnclaveAssistantSessionState;
use super::AssistantOrchestratorResult;
use crate::RuntimeState;
use crate::http::rpc;

const CALENDAR_MAX_RESULTS: usize = 20;

pub(super) async fn execute_calendar_query(
    state: &RuntimeState,
    user_id: Uuid,
    request_id: &str,
    query: &str,
    capability: AssistantQueryCapability,
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

    let calendar_day = Utc::now().date_naive();
    let time_min = match calendar_day.and_hms_opt(0, 0, 0) {
        Some(start) => start.and_utc(),
        None => {
            return Err(rpc::reject(
                StatusCode::INTERNAL_SERVER_ERROR,
                shared::enclave::EnclaveRpcErrorEnvelope::new(
                    Some(request_id.to_string()),
                    "rpc_internal_error",
                    "failed to resolve calendar day window",
                    true,
                ),
            )
            .into_response());
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
            return Err(
                rpc::map_rpc_service_error(err, Some(request_id.to_string())).into_response(),
            );
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
            return Err(rpc::reject(
                StatusCode::INTERNAL_SERVER_ERROR,
                shared::enclave::EnclaveRpcErrorEnvelope::new(
                    Some(request_id.to_string()),
                    "rpc_internal_error",
                    "failed to serialize meetings context",
                    true,
                ),
            )
            .into_response());
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

    let display_text = non_empty(summary_contract.output.summary.as_str())
        .unwrap_or(default_display_for_capability(&capability))
        .to_string();

    Ok(AssistantOrchestratorResult {
        capability,
        display_text,
        payload: AssistantStructuredPayload {
            title: summary_contract.output.title,
            summary: summary_contract.output.summary,
            key_points: summary_contract.output.key_points,
            follow_ups: summary_contract.output.follow_ups,
        },
        attested_identity: fetch_response.attested_identity,
    })
}

fn default_display_for_capability(capability: &AssistantQueryCapability) -> &'static str {
    match capability {
        AssistantQueryCapability::CalendarLookup => "Here is your calendar summary.",
        _ => "Here are your meetings for today.",
    }
}
