use shared::llm::safety::sanitize_untrusted_text;
use shared::models::{AssistantQueryCapability, AssistantResponsePart, AssistantStructuredPayload};

use super::super::session_state::EnclaveAssistantSessionState;
use super::{AssistantOrchestratorResult, local_attested_identity};
use crate::RuntimeState;

const QUERY_SNIPPET_MAX_CHARS: usize = 120;

pub(super) fn execute_general_chat(
    state: &RuntimeState,
    query: &str,
    prior_state: Option<&EnclaveAssistantSessionState>,
) -> AssistantOrchestratorResult {
    let summary = general_chat_summary(query, prior_state);

    AssistantOrchestratorResult {
        capability: AssistantQueryCapability::GeneralChat,
        display_text: summary.clone(),
        payload: AssistantStructuredPayload {
            title: "General conversation".to_string(),
            summary: summary.clone(),
            key_points: vec![
                "General-chat lane is active in Assistant v2.".to_string(),
                "Tool-backed calendar/email retrieval routes are capability-gated.".to_string(),
            ],
            follow_ups: vec![
                "Ask: What meetings do I have today?".to_string(),
                "Ask: Any important emails this week?".to_string(),
            ],
        },
        response_parts: vec![AssistantResponsePart::chat_text(summary)],
        attested_identity: local_attested_identity(state),
    }
}

fn general_chat_summary(query: &str, prior_state: Option<&EnclaveAssistantSessionState>) -> String {
    let query_snippet = sanitize_untrusted_text(query)
        .chars()
        .take(QUERY_SNIPPET_MAX_CHARS)
        .collect::<String>();
    let follow_up_context = prior_state
        .and_then(|state| state.memory.turns.last())
        .map(|turn| {
            format!(
                "Following up on your previous {} request: ",
                capability_label(&turn.capability)
            )
        })
        .unwrap_or_default();

    if query_snippet.is_empty() {
        "I can help with general conversation and, in upcoming turns, with calendar and email lookups.".to_string()
    } else {
        format!(
            "{follow_up_context}I heard: \"{query_snippet}\". I can chat generally now and will route to calendar/email tools when requested."
        )
    }
}

fn capability_label(capability: &AssistantQueryCapability) -> &'static str {
    match capability {
        AssistantQueryCapability::MeetingsToday => "meetings",
        AssistantQueryCapability::CalendarLookup => "calendar",
        AssistantQueryCapability::EmailLookup => "email",
        AssistantQueryCapability::GeneralChat => "chat",
        AssistantQueryCapability::Mixed => "calendar and email",
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use shared::assistant_memory::{
        ASSISTANT_SESSION_MEMORY_VERSION_V1, AssistantSessionMemory, AssistantSessionTurn,
    };
    use shared::models::AssistantQueryCapability;

    use super::general_chat_summary;
    use crate::http::assistant::session_state::EnclaveAssistantSessionState;

    #[test]
    fn general_chat_summary_includes_follow_up_context_when_memory_exists() {
        let prior_state = EnclaveAssistantSessionState {
            version: ASSISTANT_SESSION_MEMORY_VERSION_V1.to_string(),
            last_capability: AssistantQueryCapability::EmailLookup,
            memory: AssistantSessionMemory {
                version: ASSISTANT_SESSION_MEMORY_VERSION_V1.to_string(),
                turns: vec![AssistantSessionTurn {
                    user_query_snippet: "anything from finance?".to_string(),
                    assistant_summary_snippet: "One urgent email matched.".to_string(),
                    capability: AssistantQueryCapability::EmailLookup,
                    created_at: Utc::now(),
                }],
            },
        };

        let summary = general_chat_summary("thanks", Some(&prior_state));
        assert!(summary.starts_with("Following up on your previous email request:"));
    }
}
