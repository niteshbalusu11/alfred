use shared::assistant_planner::{detect_query_capability, resolve_query_capability};
use shared::llm::safety::sanitize_untrusted_text;
use shared::models::{AssistantQueryCapability, AssistantResponsePart, AssistantStructuredPayload};

use super::super::session_state::EnclaveAssistantSessionState;
use super::{AssistantOrchestratorResult, local_attested_identity};
use crate::RuntimeState;

const QUERY_SNIPPET_MAX_CHARS: usize = 120;
const CLARIFICATION_SUMMARY_MAX_CHARS: usize = 220;

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

pub(super) fn execute_clarification(
    state: &RuntimeState,
    question: &str,
    user_time_zone: &str,
) -> AssistantOrchestratorResult {
    let text = clarification_text(question);

    AssistantOrchestratorResult {
        capability: AssistantQueryCapability::GeneralChat,
        display_text: text.clone(),
        payload: AssistantStructuredPayload {
            title: "Clarification needed".to_string(),
            summary: text.clone(),
            key_points: vec![
                "Planner requested clarification before running tool-backed retrieval.".to_string(),
                format!("Current timezone context: {user_time_zone}"),
            ],
            follow_ups: vec![
                "Example: Show my meetings tomorrow.".to_string(),
                "Example: Any urgent emails from finance this week?".to_string(),
            ],
        },
        response_parts: vec![AssistantResponsePart::chat_text(text)],
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
        .filter(|turn| should_include_follow_up_context(query, &turn.capability))
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

fn should_include_follow_up_context(
    query: &str,
    prior_capability: &AssistantQueryCapability,
) -> bool {
    let detected = detect_query_capability(query);
    if detected.is_some() {
        return false;
    }

    resolve_query_capability(query, detected, Some(prior_capability.clone()))
        .is_some_and(|resolved| resolved == *prior_capability)
}

fn clarification_text(question: &str) -> String {
    let sanitized = sanitize_untrusted_text(question)
        .chars()
        .take(CLARIFICATION_SUMMARY_MAX_CHARS)
        .collect::<String>();
    if sanitized.trim().is_empty() {
        "Could you clarify whether you want calendar details, email details, or both?".to_string()
    } else {
        sanitized
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

    use super::{clarification_text, general_chat_summary};
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

        let summary = general_chat_summary("what about after that", Some(&prior_state));
        assert!(summary.starts_with("Following up on your previous email request:"));
    }

    #[test]
    fn general_chat_summary_skips_follow_up_context_for_normal_chat_queries() {
        let prior_state = EnclaveAssistantSessionState {
            version: ASSISTANT_SESSION_MEMORY_VERSION_V1.to_string(),
            last_capability: AssistantQueryCapability::CalendarLookup,
            memory: AssistantSessionMemory {
                version: ASSISTANT_SESSION_MEMORY_VERSION_V1.to_string(),
                turns: vec![AssistantSessionTurn {
                    user_query_snippet: "meetings tomorrow".to_string(),
                    assistant_summary_snippet: "Two meetings tomorrow.".to_string(),
                    capability: AssistantQueryCapability::CalendarLookup,
                    created_at: Utc::now(),
                }],
            },
        };

        let summary = general_chat_summary("how are you doing alfred", Some(&prior_state));
        assert!(!summary.starts_with("Following up on your previous"));
    }

    #[test]
    fn clarification_text_falls_back_when_prompt_is_empty() {
        let text = clarification_text("   ");
        assert!(text.contains("calendar details"));
    }
}
