use std::cmp::Ordering;

use shared::models::{AssistantQueryCapability, AssistantStructuredPayload};

use super::super::notifications::non_empty;
use super::calendar_range::CalendarQueryWindow;

const MAX_FALLBACK_KEY_POINTS: usize = 3;

pub(super) fn compare_meetings_by_start_time(
    left: &shared::llm::GoogleCalendarMeetingSource,
    right: &shared::llm::GoogleCalendarMeetingSource,
) -> Ordering {
    match (left.start_at, right.start_at) {
        (Some(left_start), Some(right_start)) => left_start.cmp(&right_start),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
    .then_with(|| left.title.cmp(&right.title))
    .then_with(|| left.event_id.cmp(&right.event_id))
}

pub(super) fn deterministic_calendar_fallback_payload(
    window: &CalendarQueryWindow,
    meetings: &[shared::llm::GoogleCalendarMeetingSource],
) -> AssistantStructuredPayload {
    if meetings.is_empty() {
        let (title, summary) = if window.label == "today" {
            (
                "No meetings today".to_string(),
                "No meetings are currently scheduled for today.".to_string(),
            )
        } else if window.label == "tomorrow" {
            (
                "No meetings tomorrow".to_string(),
                "No meetings are currently scheduled for tomorrow.".to_string(),
            )
        } else {
            (
                format!("No meetings for {}", window.label),
                format!("No meetings are currently scheduled for {}.", window.label),
            )
        };

        return AssistantStructuredPayload {
            title,
            summary,
            key_points: Vec::new(),
            follow_ups: Vec::new(),
        };
    }

    let meeting_count = meetings.len();
    let title = if window.label == "today" {
        "Today's meetings".to_string()
    } else if window.label == "tomorrow" {
        "Tomorrow's meetings".to_string()
    } else {
        format!("Meetings for {}", window.label)
    };

    let summary = format!(
        "You have {meeting_count} meeting{} scheduled for {}.",
        if meeting_count == 1 { "" } else { "s" },
        window.label
    );

    let key_points = meetings
        .iter()
        .take(MAX_FALLBACK_KEY_POINTS)
        .map(fallback_meeting_key_point)
        .collect::<Vec<_>>();

    AssistantStructuredPayload {
        title,
        summary,
        key_points,
        follow_ups: vec!["Open Calendar for full meeting details.".to_string()],
    }
}

pub(super) fn default_display_for_window(
    capability: &AssistantQueryCapability,
    window: &CalendarQueryWindow,
) -> &'static str {
    match capability {
        AssistantQueryCapability::CalendarLookup => {
            if window.label == "today" {
                "Here is your calendar summary for today."
            } else {
                "Here is your calendar summary."
            }
        }
        _ => "Here are your meetings.",
    }
}

fn fallback_meeting_key_point(meeting: &shared::llm::GoogleCalendarMeetingSource) -> String {
    let title = non_empty(meeting.title.as_deref().unwrap_or("")).unwrap_or("Untitled meeting");
    let start_at = meeting
        .start_at
        .map(|value| value.format("%H:%M UTC").to_string())
        .unwrap_or_else(|| "time TBD".to_string());

    format!("{start_at} - {title}")
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use shared::llm::GoogleCalendarMeetingSource;

    use super::super::calendar_range::CalendarQueryWindow;
    use super::deterministic_calendar_fallback_payload;

    fn utc(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("timestamp should parse")
            .with_timezone(&Utc)
    }

    #[test]
    fn deterministic_fallback_uses_window_label_for_no_events() {
        let window = CalendarQueryWindow {
            time_min: utc("2026-02-17T00:00:00Z"),
            time_max: utc("2026-02-24T00:00:00Z"),
            label: "2026-02-17 to 2026-02-23".to_string(),
        };

        let payload = deterministic_calendar_fallback_payload(&window, &[]);
        assert_eq!(payload.title, "No meetings for 2026-02-17 to 2026-02-23");
        assert_eq!(
            payload.summary,
            "No meetings are currently scheduled for 2026-02-17 to 2026-02-23."
        );
        assert!(payload.key_points.is_empty());
    }

    #[test]
    fn deterministic_fallback_is_grounded_to_event_times() {
        let window = CalendarQueryWindow {
            time_min: utc("2026-02-17T00:00:00Z"),
            time_max: utc("2026-02-18T00:00:00Z"),
            label: "today".to_string(),
        };

        let meetings = vec![GoogleCalendarMeetingSource {
            event_id: Some("event-1".to_string()),
            title: Some("Team Sync".to_string()),
            start_at: Some(utc("2026-02-17T16:30:00Z")),
            end_at: None,
            attendee_emails: vec![],
        }];

        let payload = deterministic_calendar_fallback_payload(&window, &meetings);
        assert_eq!(payload.title, "Today's meetings");
        assert_eq!(payload.summary, "You have 1 meeting scheduled for today.");
        assert_eq!(
            payload.key_points,
            vec!["16:30 UTC - Team Sync".to_string()]
        );
    }
}
