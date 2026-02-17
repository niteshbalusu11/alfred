use shared::llm::safety::sanitize_untrusted_text;
use shared::models::{AssistantQueryCapability, AssistantStructuredPayload};

use super::{AssistantOrchestratorResult, local_attested_identity};
use crate::RuntimeState;

const QUERY_SNIPPET_MAX_CHARS: usize = 120;

pub(super) fn execute_general_chat(
    state: &RuntimeState,
    query: &str,
) -> AssistantOrchestratorResult {
    let query_snippet = sanitize_untrusted_text(query)
        .chars()
        .take(QUERY_SNIPPET_MAX_CHARS)
        .collect::<String>();
    let summary = if query_snippet.is_empty() {
        "I can help with general conversation and, in upcoming turns, with calendar and email lookups.".to_string()
    } else {
        format!(
            "I heard: \"{query_snippet}\". I can chat generally now and will route to calendar/email tools when requested."
        )
    };

    AssistantOrchestratorResult {
        capability: AssistantQueryCapability::GeneralChat,
        display_text: summary.clone(),
        payload: AssistantStructuredPayload {
            title: "General conversation".to_string(),
            summary,
            key_points: vec![
                "General-chat lane is active in Assistant v2.".to_string(),
                "Tool-backed calendar/email retrieval routes are capability-gated.".to_string(),
            ],
            follow_ups: vec![
                "Ask: What meetings do I have today?".to_string(),
                "Ask: Any important emails this week?".to_string(),
            ],
        },
        attested_identity: local_attested_identity(state),
    }
}
