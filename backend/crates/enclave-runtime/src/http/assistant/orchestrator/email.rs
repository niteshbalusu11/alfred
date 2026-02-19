use std::time::Instant;

use axum::response::{IntoResponse, Response};
use chrono::Utc;
use shared::assistant_semantic_plan::AssistantSemanticEmailFilters;
use shared::models::{AssistantQueryCapability, AssistantResponsePart};
use tracing::info;
use uuid::Uuid;

use super::super::mapping::map_email_candidate_source;
use super::super::notifications::non_empty;
use super::AssistantOrchestratorResult;
use super::email_fallback::deterministic_email_fallback_payload;
use super::email_plan::{apply_email_filters, build_gmail_query, plan_email_query};
use crate::RuntimeState;
use crate::http::rpc;

const EMAIL_MAX_RESULTS: usize = 20;

pub(super) async fn execute_email_query(
    state: &RuntimeState,
    user_id: Uuid,
    request_id: &str,
    email_filters: &AssistantSemanticEmailFilters,
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
    let now = Utc::now();

    let plan_started = Instant::now();
    let plan = plan_email_query(email_filters, now);
    let email_plan_ms = plan_started.elapsed().as_millis() as u64;

    let fetch_started = Instant::now();
    let fetch_response = match state
        .enclave_service
        .fetch_google_email_candidates(connector, Some(build_gmail_query(&plan)), EMAIL_MAX_RESULTS)
        .await
    {
        Ok(response) => response,
        Err(err) => {
            return Err(
                rpc::map_rpc_service_error(err, Some(request_id.to_string())).into_response(),
            );
        }
    };
    let email_fetch_ms = fetch_started.elapsed().as_millis() as u64;

    let filter_started = Instant::now();
    let raw_candidates = fetch_response
        .candidates
        .iter()
        .map(map_email_candidate_source)
        .collect::<Vec<_>>();
    let candidates = apply_email_filters(raw_candidates, &plan, now);
    let email_filter_ms = filter_started.elapsed().as_millis() as u64;

    let payload = deterministic_email_fallback_payload(&plan, &candidates);
    let used_deterministic_fallback = true;

    let display_text = non_empty(payload.summary.as_str())
        .unwrap_or("Here is your inbox summary.")
        .to_string();
    let response_parts = vec![
        AssistantResponsePart::chat_text(display_text.clone()),
        AssistantResponsePart::tool_summary(AssistantQueryCapability::EmailLookup, payload.clone()),
    ];
    info!(
        user_id = %user_id,
        request_id,
        connector_resolve_ms,
        email_plan_ms,
        email_fetch_ms,
        email_filter_ms,
        email_llm_latency_ms = 0_u64,
        email_llm_outcome = "single_call_deterministic",
        email_llm_model = Option::<String>::None,
        candidates_count = candidates.len(),
        used_deterministic_fallback,
        total_email_lane_ms = lane_started.elapsed().as_millis() as u64,
        "assistant email lane latency breakdown"
    );

    Ok(AssistantOrchestratorResult {
        capability: AssistantQueryCapability::EmailLookup,
        display_text,
        payload,
        response_parts,
        attested_identity: fetch_response.attested_identity,
    })
}
