use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use shared::assistant_memory::{
    ASSISTANT_SESSION_MEMORY_VERSION_V1, AssistantSessionMemory, AssistantSessionTurn,
};
use shared::assistant_planner::{
    detect_query_capability as detect_query_capability_shared,
    resolve_query_capability as resolve_query_capability_shared,
};
use shared::llm::safety::sanitize_untrusted_text;
use shared::models::AssistantQueryCapability;

const SESSION_MEMORY_MAX_TURNS: usize = 25;
const SESSION_MEMORY_QUERY_MAX_CHARS: usize = 180;
const SESSION_MEMORY_SUMMARY_MAX_CHARS: usize = 280;
const SESSION_CONTEXT_QUERY_MAX_CHARS: usize = 280;

pub(super) fn detect_query_capability(query: &str) -> Option<AssistantQueryCapability> {
    detect_query_capability_shared(query)
}

pub(super) fn resolve_query_capability(
    query: &str,
    detected: Option<AssistantQueryCapability>,
    prior_capability: Option<AssistantQueryCapability>,
) -> Option<AssistantQueryCapability> {
    resolve_query_capability_shared(query, detected, prior_capability)
}

pub(super) fn build_updated_memory(
    existing_memory: Option<&AssistantSessionMemory>,
    query: &str,
    assistant_summary: &str,
    capability: AssistantQueryCapability,
    now: DateTime<Utc>,
) -> AssistantSessionMemory {
    let mut turns = existing_memory
        .map(|memory| memory.turns.clone())
        .unwrap_or_default();

    turns.push(AssistantSessionTurn {
        user_query_snippet: redact_and_truncate(query, SESSION_MEMORY_QUERY_MAX_CHARS),
        assistant_summary_snippet: redact_and_truncate(
            assistant_summary,
            SESSION_MEMORY_SUMMARY_MAX_CHARS,
        ),
        capability,
        created_at: now,
    });

    if turns.len() > SESSION_MEMORY_MAX_TURNS {
        turns = turns.split_off(turns.len() - SESSION_MEMORY_MAX_TURNS);
    }

    AssistantSessionMemory {
        version: ASSISTANT_SESSION_MEMORY_VERSION_V1.to_string(),
        turns,
    }
}

pub(super) fn query_context_snippet(query: &str) -> String {
    redact_and_truncate(query, SESSION_CONTEXT_QUERY_MAX_CHARS)
}

pub(super) fn session_memory_context(memory: Option<&AssistantSessionMemory>) -> Option<Value> {
    let memory = memory?;
    if memory.turns.is_empty() {
        return None;
    }

    Some(json!({
        "version": memory.version,
        "turn_count": memory.turns.len(),
        "recent_turns": memory.turns,
    }))
}

fn redact_and_truncate(value: &str, max_chars: usize) -> String {
    let sanitized = sanitize_untrusted_text(value);
    sanitized.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::{detect_query_capability, resolve_query_capability};
    use shared::models::AssistantQueryCapability;

    #[test]
    fn detect_capability_classifies_calendar_and_email_queries() {
        assert_eq!(
            detect_query_capability("What meetings do I have today?"),
            Some(AssistantQueryCapability::MeetingsToday)
        );
        assert_eq!(
            detect_query_capability("Show my schedule next week"),
            Some(AssistantQueryCapability::CalendarLookup)
        );
        assert_eq!(
            detect_query_capability("Any emails from finance?"),
            Some(AssistantQueryCapability::EmailLookup)
        );
        assert_eq!(
            detect_query_capability("Check calendar and inbox for this afternoon"),
            Some(AssistantQueryCapability::Mixed)
        );
    }

    #[test]
    fn resolve_capability_uses_prior_for_follow_up_queries() {
        assert_eq!(
            resolve_query_capability(
                "what about after that?",
                None,
                Some(AssistantQueryCapability::EmailLookup),
            ),
            Some(AssistantQueryCapability::EmailLookup)
        );
    }

    #[test]
    fn resolve_capability_switches_between_chat_and_tool_lanes() {
        assert_eq!(
            resolve_query_capability(
                "show my meetings tomorrow",
                detect_query_capability("show my meetings tomorrow"),
                Some(AssistantQueryCapability::GeneralChat),
            ),
            Some(AssistantQueryCapability::CalendarLookup)
        );

        assert_eq!(
            resolve_query_capability("thanks", detect_query_capability("thanks"), None),
            None
        );
    }
}
