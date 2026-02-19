use std::time::Instant;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use shared::assistant_semantic_plan::AssistantSemanticTimeWindow;
use shared::models::{AssistantQueryCapability, AssistantResponsePart};
use tracing::info;
use uuid::Uuid;

use super::super::mapping::map_calendar_event_to_meeting_source;
use super::AssistantOrchestratorResult;
use super::calendar_fallback::{
    compare_meetings_by_start_time, default_display_for_window,
    deterministic_calendar_fallback_payload,
};
use super::calendar_range::calendar_window_from_semantic_time_window;
use crate::RuntimeState;
use crate::http::rpc;

const CALENDAR_MAX_RESULTS: usize = 20;

pub(super) async fn execute_calendar_query(
    state: &RuntimeState,
    user_id: Uuid,
    request_id: &str,
    capability: AssistantQueryCapability,
    time_window: &AssistantSemanticTimeWindow,
) -> Result<AssistantOrchestratorResult, Response> {
    let lane_started = Instant::now();

    let connector_started = Instant::now();
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
    let connector_resolve_ms = connector_started.elapsed().as_millis() as u64;

    let window_started = Instant::now();
    let window = match calendar_window_from_semantic_time_window(time_window) {
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
    let window_plan_ms = window_started.elapsed().as_millis() as u64;

    let fetch_started = Instant::now();
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
    let calendar_fetch_ms = fetch_started.elapsed().as_millis() as u64;

    let mut meetings = fetch_response
        .events
        .iter()
        .map(map_calendar_event_to_meeting_source)
        .collect::<Vec<_>>();
    meetings.sort_by(compare_meetings_by_start_time);

    let payload = deterministic_calendar_fallback_payload(&window, &meetings);
    let used_deterministic_fallback = true;

    let display_text = super::super::notifications::non_empty(payload.summary.as_str())
        .unwrap_or(default_display_for_window(&capability, &window))
        .to_string();
    let response_parts = vec![
        AssistantResponsePart::chat_text(display_text.clone()),
        AssistantResponsePart::tool_summary(capability.clone(), payload.clone()),
    ];
    info!(
        user_id = %user_id,
        request_id,
        connector_resolve_ms,
        window_plan_ms,
        calendar_fetch_ms,
        calendar_llm_latency_ms = 0_u64,
        calendar_llm_outcome = "single_call_deterministic",
        calendar_llm_model = Option::<String>::None,
        meetings_count = meetings.len(),
        used_deterministic_fallback,
        total_calendar_lane_ms = lane_started.elapsed().as_millis() as u64,
        "assistant calendar lane latency breakdown"
    );

    Ok(AssistantOrchestratorResult {
        capability,
        display_text,
        payload,
        response_parts,
        attested_identity: fetch_response.attested_identity,
    })
}
