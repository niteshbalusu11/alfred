use axum::response::Response;
use shared::enclave::AttestedIdentityPayload;
use shared::models::{AssistantQueryCapability, AssistantStructuredPayload};
use uuid::Uuid;

use super::memory::{detect_query_capability, resolve_query_capability};
use super::session_state::EnclaveAssistantSessionState;
use crate::RuntimeState;

mod calendar;
mod calendar_fallback;
mod calendar_range;
mod chat;
mod email;
mod mixed;

pub(super) struct AssistantOrchestratorResult {
    pub(super) capability: AssistantQueryCapability,
    pub(super) display_text: String,
    pub(super) payload: AssistantStructuredPayload,
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
            calendar::execute_calendar_query(
                state,
                user_id,
                request_id,
                query,
                capability,
                prior_state,
            )
            .await
        }
        AssistantQueryCapability::EmailLookup => Ok(email::execute_email_query(state, query)),
        AssistantQueryCapability::Mixed => Ok(mixed::execute_mixed_query(state, query)),
        AssistantQueryCapability::GeneralChat => Ok(chat::execute_general_chat(state, query)),
    }
}

fn local_attested_identity(state: &RuntimeState) -> AttestedIdentityPayload {
    AttestedIdentityPayload {
        runtime: state.config.runtime_id.clone(),
        measurement: state.config.measurement.clone(),
    }
}
