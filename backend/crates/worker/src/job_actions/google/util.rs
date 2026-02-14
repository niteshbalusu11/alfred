use chrono::{DateTime, Utc};

use super::fetch::{GoogleCalendarEvent, GoogleGmailMessageDetail};

const URGENT_KEYWORDS: &[&str] = &[
    "urgent",
    "asap",
    "immediately",
    "action required",
    "deadline",
    "important",
];

pub(super) struct TimedCalendarEvent {
    pub(super) id: String,
    pub(super) summary: Option<String>,
    pub(super) start_at: DateTime<Utc>,
}

pub(super) fn first_timed_event(events: Vec<GoogleCalendarEvent>) -> Option<TimedCalendarEvent> {
    events.into_iter().find_map(|event| {
        let start = event.start?.date_time?;
        let start_at = DateTime::parse_from_rfc3339(&start)
            .ok()?
            .with_timezone(&Utc);

        Some(TimedCalendarEvent {
            id: event.id.unwrap_or_else(|| "unknown".to_string()),
            summary: event.summary,
            start_at,
        })
    })
}

pub(super) fn classify_urgent_message(message: &GoogleGmailMessageDetail) -> Option<&'static str> {
    if message.label_ids.iter().any(|label| label == "IMPORTANT") {
        return Some("important_label");
    }

    let subject = message_header(message, "Subject").unwrap_or_default();
    if contains_urgent_keyword(subject) {
        return Some("subject_keyword");
    }

    let sender = message_header(message, "From").unwrap_or_default();
    if contains_urgent_keyword(sender) {
        return Some("sender_keyword");
    }

    if contains_urgent_keyword(&message.snippet) {
        return Some("snippet_keyword");
    }

    None
}

pub(super) fn message_header<'a>(
    message: &'a GoogleGmailMessageDetail,
    header_name: &str,
) -> Option<&'a str> {
    let payload = message.payload.as_ref()?;

    payload
        .headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case(header_name))
        .map(|header| header.value.trim())
}

pub(super) fn truncate_for_notification(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    let mut out = trimmed.chars().take(max_chars).collect::<String>();
    if trimmed.chars().count() > max_chars {
        out.push_str("...");
    }
    out
}

fn contains_urgent_keyword(value: &str) -> bool {
    let lowered = value.to_ascii_lowercase();
    URGENT_KEYWORDS
        .iter()
        .any(|keyword| lowered.contains(keyword))
}

#[cfg(test)]
mod tests {
    use super::{classify_urgent_message, truncate_for_notification};
    use crate::job_actions::google::fetch::{
        GoogleGmailHeader, GoogleGmailMessageDetail, GoogleGmailPayload,
    };

    fn message(
        subject: &str,
        from: &str,
        snippet: &str,
        labels: &[&str],
    ) -> GoogleGmailMessageDetail {
        GoogleGmailMessageDetail {
            id: "msg-1".to_string(),
            label_ids: labels.iter().map(|item| (*item).to_string()).collect(),
            snippet: snippet.to_string(),
            payload: Some(GoogleGmailPayload {
                headers: vec![
                    GoogleGmailHeader {
                        name: "Subject".to_string(),
                        value: subject.to_string(),
                    },
                    GoogleGmailHeader {
                        name: "From".to_string(),
                        value: from.to_string(),
                    },
                ],
            }),
        }
    }

    #[test]
    fn urgent_rule_matches_important_label() {
        let detail = message(
            "status update",
            "team@example.com",
            "all good",
            &["IMPORTANT"],
        );
        assert_eq!(classify_urgent_message(&detail), Some("important_label"));
    }

    #[test]
    fn urgent_rule_matches_subject_keyword() {
        let detail = message(
            "Action Required: contract",
            "team@example.com",
            "all good",
            &[],
        );
        assert_eq!(classify_urgent_message(&detail), Some("subject_keyword"));
    }

    #[test]
    fn urgent_rule_returns_none_for_non_urgent_message() {
        let detail = message("weekly notes", "team@example.com", "regular summary", &[]);
        assert_eq!(classify_urgent_message(&detail), None);
    }

    #[test]
    fn notification_truncation_appends_ellipsis() {
        let truncated = truncate_for_notification("abcdefghijklmnop", 5);
        assert_eq!(truncated, "abcde...");
    }
}
