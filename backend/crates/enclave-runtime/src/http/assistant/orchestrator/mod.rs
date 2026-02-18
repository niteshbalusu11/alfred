use axum::response::Response;
use shared::enclave::AttestedIdentityPayload;
use shared::models::{AssistantQueryCapability, AssistantResponsePart, AssistantStructuredPayload};
use shared::timezone::DEFAULT_USER_TIME_ZONE;
use tracing::warn;
use uuid::Uuid;

use super::session_state::EnclaveAssistantSessionState;
use crate::RuntimeState;

mod calendar;
mod calendar_fallback;
mod calendar_range;
mod chat;
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
    let user_time_zone = resolve_user_time_zone(state, user_id).await;
    let semantic_plan =
        planner::resolve_semantic_plan(state, user_id, query, user_time_zone.as_str(), prior_state)
            .await;
    let route = policy::resolve_route_policy(&semantic_plan);

    match route {
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
                    user_time_zone.as_str(),
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
                    user_time_zone.as_str(),
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
                    user_time_zone.as_str(),
                    prior_state,
                )
                .await
            }
            AssistantQueryCapability::GeneralChat => {
                Ok(chat::execute_general_chat(state, user_id, query, prior_state).await)
            }
        },
    }
}

fn local_attested_identity(state: &RuntimeState) -> AttestedIdentityPayload {
    AttestedIdentityPayload {
        runtime: state.config.runtime_id.clone(),
        measurement: state.config.measurement.clone(),
    }
}

async fn resolve_user_time_zone(state: &RuntimeState, user_id: Uuid) -> String {
    match state
        .enclave_service
        .get_or_create_preferences(user_id)
        .await
    {
        Ok(preferences) => preferences.time_zone,
        Err(err) => {
            warn!(
                user_id = %user_id,
                "assistant preferences lookup failed; defaulting to UTC timezone: {err}"
            );
            DEFAULT_USER_TIME_ZONE.to_string()
        }
    }
}
