use crate::models::AssistantQueryCapability;

pub fn detect_query_capability(query: &str) -> Option<AssistantQueryCapability> {
    let normalized = query.to_ascii_lowercase();
    let asks_for_today = normalized.contains("today");
    let asks_for_calendar = normalized.contains("meeting")
        || normalized.contains("calendar")
        || normalized.contains("schedule")
        || normalized.contains("event");
    let asks_for_email = normalized.contains("email")
        || normalized.contains("inbox")
        || normalized.contains("mail")
        || normalized.contains("gmail");

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
    let normalized = query.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return false;
    }

    let token_count = normalized.split_whitespace().count();
    if token_count > 10 {
        return false;
    }

    let follow_up_markers = [
        "what about",
        "how about",
        "and then",
        "then",
        "next",
        "after that",
        "same",
        "again",
        "also",
        "those",
        "them",
    ];

    follow_up_markers
        .iter()
        .any(|marker| normalized.contains(marker))
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
            detect_query_capability("Any emails from finance?"),
            Some(AssistantQueryCapability::EmailLookup)
        );
        assert_eq!(
            detect_query_capability("Check calendar and inbox for this afternoon"),
            Some(AssistantQueryCapability::Mixed)
        );
        assert_eq!(detect_query_capability("thanks"), None);
    }

    #[test]
    fn resolve_capability_uses_prior_for_follow_up_queries() {
        assert_eq!(
            resolve_query_capability(
                "what about after that",
                None,
                Some(AssistantQueryCapability::EmailLookup),
            ),
            Some(AssistantQueryCapability::EmailLookup)
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
