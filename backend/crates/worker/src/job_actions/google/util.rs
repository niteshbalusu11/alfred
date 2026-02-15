use chrono::{DateTime, Utc};

use super::fetch::GoogleCalendarEvent;

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

pub(super) fn truncate_for_notification(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    let mut out = trimmed.chars().take(max_chars).collect::<String>();
    if trimmed.chars().count() > max_chars {
        out.push_str("...");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::truncate_for_notification;

    #[test]
    fn notification_truncation_appends_ellipsis() {
        let truncated = truncate_for_notification("abcdefghijklmnop", 5);
        assert_eq!(truncated, "abcde...");
    }
}
