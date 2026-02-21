use std::time::Instant;

use axum::response::Response;
use shared::enclave::AttestedIdentityPayload;
use shared::models::{AssistantQueryCapability, AssistantResponsePart, AssistantStructuredPayload};
use shared::timezone::DEFAULT_USER_TIME_ZONE;
use tracing::{info, warn};
use uuid::Uuid;

use super::session_state::EnclaveAssistantSessionState;
use crate::RuntimeState;

mod calendar;
mod calendar_fallback;
mod calendar_range;
mod chat;
mod chat_fast_path;
mod email;
mod email_fallback;
mod email_plan;
mod mixed;
mod planner;
mod policy;

pub(super) struct AssistantOrchestratorResult {
    pub(super) capability: AssistantQueryCapability,
    pub(super) display_text: String,
    pub(super) payload: AssistantStructuredPayload,
    pub(super) response_parts: Vec<AssistantResponsePart>,
    pub(super) attested_identity: AttestedIdentityPayload,
}

pub(super) async fn execute_query(
    state: &RuntimeState,
    user_id: Uuid,
    request_id: &str,
    query: &str,
    prior_state: Option<&EnclaveAssistantSessionState>,
) -> Result<AssistantOrchestratorResult, Response> {
    let orchestrator_started = Instant::now();

    if chat_fast_path::is_small_talk_fast_path_query(query) {
        let lane_started = Instant::now();
        let execution =
            chat::execute_general_chat(state, user_id, request_id, query, prior_state).await;
        let lane_stage_ms = lane_started.elapsed().as_millis() as u64;
        let total_orchestrator_ms = orchestrator_started.elapsed().as_millis() as u64;
        info!(
            user_id = %user_id,
            request_id,
            route = "general_chat_fast_path",
            final_capability = capability_label(&execution.capability),
            planner_confidence = 1.0_f32,
            planner_needs_clarification = false,
            planner_used_deterministic_fallback = false,
            timezone_lookup_ms = 0_u64,
            planner_stage_ms = 0_u64,
            lane_stage_ms,
            total_orchestrator_ms,
            "assistant orchestrator latency breakdown"
        );
        return Ok(execution);
    }

    let timezone_lookup_started = Instant::now();
    let user_time_zone = resolve_user_time_zone(state, user_id);
    let timezone_lookup_ms = timezone_lookup_started.elapsed().as_millis() as u64;

    let planner_started = Instant::now();
    let semantic_plan = planner::resolve_semantic_plan(
        state,
        user_id,
        request_id,
        query,
        user_time_zone.as_str(),
        prior_state,
    )
    .await;
    let planner_stage_ms = planner_started.elapsed().as_millis() as u64;
    let route = policy::resolve_route_policy(&semantic_plan);
    let route_label = planned_route_label(&route);

    let lane_started = Instant::now();
    let result = match route {
        policy::PlannedRoute::Clarify(question) => Ok(chat::execute_clarification(
            state,
            question.as_str(),
            user_time_zone.as_str(),
        )),
        policy::PlannedRoute::Execute(capability) => match capability {
            AssistantQueryCapability::MeetingsToday | AssistantQueryCapability::CalendarLookup => {
                calendar::execute_calendar_query(
                    state,
                    user_id,
                    request_id,
                    query,
                    capability,
                    &semantic_plan.plan,
                    prior_state,
                )
                .await
            }
            AssistantQueryCapability::EmailLookup => {
                email::execute_email_query(
                    state,
                    user_id,
                    request_id,
                    query,
                    &semantic_plan.plan,
                    prior_state,
                )
                .await
            }
            AssistantQueryCapability::Mixed => {
                mixed::execute_mixed_query(
                    state,
                    user_id,
                    request_id,
                    query,
                    &semantic_plan.plan,
                    prior_state,
                )
                .await
            }
            AssistantQueryCapability::GeneralChat => {
                Ok(
                    chat::execute_general_chat(state, user_id, request_id, query, prior_state)
                        .await,
                )
            }
        },
    };
    let lane_stage_ms = lane_started.elapsed().as_millis() as u64;
    let total_orchestrator_ms = orchestrator_started.elapsed().as_millis() as u64;

    match &result {
        Ok(execution) => {
            info!(
                user_id = %user_id,
                request_id,
                route = route_label,
                final_capability = capability_label(&execution.capability),
                planner_confidence = semantic_plan.plan.confidence,
                planner_needs_clarification = semantic_plan.plan.needs_clarification,
                planner_used_deterministic_fallback = semantic_plan.used_deterministic_fallback,
                timezone_lookup_ms,
                planner_stage_ms,
                lane_stage_ms,
                total_orchestrator_ms,
                "assistant orchestrator latency breakdown"
            );
        }
        Err(response) => {
            warn!(
                user_id = %user_id,
                request_id,
                route = route_label,
                status = response.status().as_u16(),
                planner_confidence = semantic_plan.plan.confidence,
                planner_needs_clarification = semantic_plan.plan.needs_clarification,
                planner_used_deterministic_fallback = semantic_plan.used_deterministic_fallback,
                timezone_lookup_ms,
                planner_stage_ms,
                lane_stage_ms,
                total_orchestrator_ms,
                "assistant orchestrator failed"
            );
        }
    }

    result
}

fn planned_route_label(route: &policy::PlannedRoute) -> &'static str {
    match route {
        policy::PlannedRoute::Clarify(_) => "clarify",
        policy::PlannedRoute::Execute(capability) => capability_label(capability),
    }
}

fn capability_label(capability: &AssistantQueryCapability) -> &'static str {
    match capability {
        AssistantQueryCapability::MeetingsToday => "meetings_today",
        AssistantQueryCapability::CalendarLookup => "calendar_lookup",
        AssistantQueryCapability::EmailLookup => "email_lookup",
        AssistantQueryCapability::GeneralChat => "general_chat",
        AssistantQueryCapability::Mixed => "mixed",
    }
}

fn local_attested_identity(state: &RuntimeState) -> AttestedIdentityPayload {
    AttestedIdentityPayload {
        runtime: state.config.runtime_id.clone(),
        measurement: state.config.measurement.clone(),
    }
}

fn resolve_user_time_zone(_state: &RuntimeState, _user_id: Uuid) -> String {
    DEFAULT_USER_TIME_ZONE.to_string()
}
