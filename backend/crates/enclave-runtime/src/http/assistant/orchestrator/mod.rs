use axum::response::Response;
use shared::enclave::AttestedIdentityPayload;
use shared::models::{AssistantQueryCapability, AssistantResponsePart, AssistantStructuredPayload};
use shared::timezone::DEFAULT_USER_TIME_ZONE;
use tracing::warn;
use uuid::Uuid;

use super::memory::{detect_query_capability, resolve_query_capability};
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
    let detected_capability = detect_query_capability(query);
    let capability = resolve_query_capability(
        query,
        detected_capability,
        prior_state
            .as_ref()
            .map(|state| state.last_capability.clone()),
    )
    .unwrap_or(AssistantQueryCapability::GeneralChat);

    match capability {
        AssistantQueryCapability::MeetingsToday | AssistantQueryCapability::CalendarLookup => {
            let user_time_zone = resolve_user_time_zone(state, user_id).await;
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
            let user_time_zone = resolve_user_time_zone(state, user_id).await;
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
            let user_time_zone = resolve_user_time_zone(state, user_id).await;
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
            Ok(chat::execute_general_chat(state, query, prior_state))
        }
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
