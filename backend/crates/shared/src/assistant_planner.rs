use crate::models::AssistantQueryCapability;

pub fn detect_query_capability(query: &str) -> Option<AssistantQueryCapability> {
    let normalized = query.to_ascii_lowercase();
    let asks_for_today = contains_any(
        normalized.as_str(),
        &["today", "this morning", "this afternoon", "tonight"],
    );
    let asks_for_calendar = contains_any(
        normalized.as_str(),
        &[
            "meeting",
            "calendar",
            "schedule",
            "event",
            "agenda",
            "appointment",
            "appointments",
        ],
    );
    let asks_for_email = contains_any(
        normalized.as_str(),
        &[
            "email",
            "inbox",
            "mail",
            "gmail",
            "mailbox",
            "messages",
            "message thread",
            "threads",
        ],
    );

    if asks_for_calendar && asks_for_email {
        return Some(AssistantQueryCapability::Mixed);
    }

    if asks_for_email {
        return Some(AssistantQueryCapability::EmailLookup);
    }

    if asks_for_today && asks_for_calendar {
        return Some(AssistantQueryCapability::MeetingsToday);
    }

    if asks_for_calendar {
        return Some(AssistantQueryCapability::CalendarLookup);
    }

    None
}

pub fn resolve_query_capability(
    query: &str,
    detected: Option<AssistantQueryCapability>,
    prior_capability: Option<AssistantQueryCapability>,
) -> Option<AssistantQueryCapability> {
    detected.or_else(|| {
        if looks_like_follow_up_query(query) {
            return prior_capability;
        }

        None
    })
}

fn looks_like_follow_up_query(query: &str) -> bool {
    let normalized = query.trim();
    if normalized.is_empty() {
        return false;
    }

    let normalized_lower = normalized.to_ascii_lowercase();
    let tokens = normalized_lower.split_whitespace().collect::<Vec<_>>();
    let token_count = tokens.len();
    if token_count == 0 || token_count > 8 {
        return false;
    }

    // Treat compact, elliptical prompts as likely follow-ups to prior intent.
    // This is intentionally generic (question form + omitted context), not
    // capability keyword routing.
    if normalized.ends_with('?') || normalized.ends_with("?!") || normalized.ends_with("...") {
        return !contains_standalone_question_verb(&tokens);
    }

    if token_count <= 5
        && starts_with_any(&tokens, &["what", "how"])
        && !contains_standalone_question_verb(&tokens)
    {
        return true;
    }

    token_count <= 4
        && (starts_with_any(&tokens, &["same", "again", "then"])
            || contains_any_token(&tokens, &["afterward", "afterwards"]))
}

fn starts_with_any(tokens: &[&str], values: &[&str]) -> bool {
    tokens.first().is_some_and(|token| values.contains(token))
}

fn contains_any_token(tokens: &[&str], values: &[&str]) -> bool {
    tokens.iter().any(|token| values.contains(token))
}

fn contains_standalone_question_verb(tokens: &[&str]) -> bool {
    const QUESTION_VERBS: &[&str] = &[
        "is", "are", "am", "was", "were", "do", "does", "did", "can", "could", "will", "would",
        "should", "has", "have", "had",
    ];
    contains_any_token(tokens, QUESTION_VERBS)
}

fn contains_any(query: &str, terms: &[&str]) -> bool {
    terms.iter().any(|term| query.contains(term))
}

#[cfg(test)]
mod tests {
    use super::{detect_query_capability, resolve_query_capability};
    use crate::models::AssistantQueryCapability;

    #[test]
    fn detect_capability_classifies_chat_calendar_email_and_mixed() {
        assert_eq!(
            detect_query_capability("What meetings do I have today?"),
            Some(AssistantQueryCapability::MeetingsToday)
        );
        assert_eq!(
            detect_query_capability("Show my schedule next week"),
            Some(AssistantQueryCapability::CalendarLookup)
        );
        assert_eq!(
            detect_query_capability("What is on my agenda this afternoon?"),
            Some(AssistantQueryCapability::MeetingsToday)
        );
        assert_eq!(
            detect_query_capability("What is on my agenda tonight?"),
            Some(AssistantQueryCapability::MeetingsToday)
        );
        assert_eq!(
            detect_query_capability("Do I have any appointments tomorrow?"),
            Some(AssistantQueryCapability::CalendarLookup)
        );
        assert_eq!(
            detect_query_capability("Any emails from finance?"),
            Some(AssistantQueryCapability::EmailLookup)
        );
        assert_eq!(
            detect_query_capability("Any messages from finance in my mailbox?"),
            Some(AssistantQueryCapability::EmailLookup)
        );
        assert_eq!(
            detect_query_capability("Check calendar and inbox for this afternoon"),
            Some(AssistantQueryCapability::Mixed)
        );
        assert_eq!(
            detect_query_capability("Give me agenda and inbox updates for next week"),
            Some(AssistantQueryCapability::Mixed)
        );
        assert_eq!(detect_query_capability("thanks"), None);
    }

    #[test]
    fn resolve_capability_uses_prior_for_follow_up_queries() {
        assert_eq!(
            resolve_query_capability(
                "what about India?",
                None,
                Some(AssistantQueryCapability::EmailLookup),
            ),
            Some(AssistantQueryCapability::EmailLookup)
        );
        assert_eq!(
            resolve_query_capability(
                "tomorrow?",
                None,
                Some(AssistantQueryCapability::CalendarLookup),
            ),
            Some(AssistantQueryCapability::CalendarLookup)
        );
        assert_eq!(
            resolve_query_capability("same window?", None, Some(AssistantQueryCapability::Mixed),),
            Some(AssistantQueryCapability::Mixed)
        );
        assert_eq!(
            resolve_query_capability("thanks", None, Some(AssistantQueryCapability::Mixed)),
            None
        );
        assert_eq!(
            resolve_query_capability(
                "what is the capital of india?",
                None,
                Some(AssistantQueryCapability::EmailLookup),
            ),
            None
        );
    }

    #[test]
    fn resolve_capability_preserves_explicit_tool_detection() {
        assert_eq!(
            resolve_query_capability(
                "show my meetings tomorrow",
                detect_query_capability("show my meetings tomorrow"),
                Some(AssistantQueryCapability::GeneralChat),
            ),
            Some(AssistantQueryCapability::CalendarLookup)
        );
    }
}
