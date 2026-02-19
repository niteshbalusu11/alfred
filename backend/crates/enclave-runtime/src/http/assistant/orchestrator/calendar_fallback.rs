use std::cmp::Ordering;

use serde_json::{Value, json};
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

pub(super) fn build_calendar_context_payload(
    window: &CalendarQueryWindow,
    meetings: &[shared::llm::GoogleCalendarMeetingSource],
) -> Value {
    let entries = meetings
        .iter()
        .enumerate()
        .map(|(index, meeting)| {
            json!({
                "event_ref": meeting
                    .event_id
                    .clone()
                    .unwrap_or_else(|| format!("meeting-{:03}", index + 1)),
                "title": meeting
                    .title
                    .clone()
                    .unwrap_or_else(|| "Untitled meeting".to_string()),
                "start_at": meeting
                    .start_at
                    .map(|value| value.to_rfc3339())
                    .unwrap_or_default(),
                "end_at": meeting.end_at.map(|value| value.to_rfc3339()),
                "attendee_count": meeting.attendee_emails.len(),
            })
        })
        .collect::<Vec<_>>();

    json!({
        "version": shared::llm::CONTEXT_CONTRACT_VERSION_V1,
        "range_label": window.label,
        "time_min_utc": window.time_min.to_rfc3339(),
        "time_max_utc": window.time_max.to_rfc3339(),
        "meeting_count": meetings.len(),
        "meetings": entries,
    })
}

pub(super) fn deterministic_calendar_fallback_payload(
    window: &CalendarQueryWindow,
    meetings: &[shared::llm::GoogleCalendarMeetingSource],
) -> AssistantStructuredPayload {
    if meetings.is_empty() {
        return AssistantStructuredPayload {
            title: "No meetings in range".to_string(),
            summary: format!("No meetings are currently scheduled for {}.", window.label),
            key_points: Vec::new(),
            follow_ups: Vec::new(),
        };
    }

    let meeting_count = meetings.len();
    let key_points = meetings
        .iter()
        .take(MAX_FALLBACK_KEY_POINTS)
        .map(fallback_meeting_key_point)
        .collect::<Vec<_>>();

    AssistantStructuredPayload {
        title: "Meetings in range".to_string(),
        summary: format!(
            "You have {meeting_count} meeting{} scheduled for {}.",
            if meeting_count == 1 { "" } else { "s" },
            window.label
        ),
        key_points,
        follow_ups: vec!["Open Calendar for full meeting details.".to_string()],
    }
}

pub(super) fn default_display_for_window(
    _capability: &AssistantQueryCapability,
    _window: &CalendarQueryWindow,
) -> &'static str {
    "Here is your calendar summary."
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
    use shared::assistant_semantic_plan::{
        AssistantSemanticTimeWindow, AssistantTimeWindowResolutionSource,
    };
    use shared::llm::GoogleCalendarMeetingSource;

    use super::super::calendar_range::window_from_semantic_time_window;
    use super::deterministic_calendar_fallback_payload;

    fn utc(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("timestamp should parse")
            .with_timezone(&Utc)
    }

    fn window(start: &str, end: &str) -> super::CalendarQueryWindow {
        let semantic = AssistantSemanticTimeWindow {
            start: utc(start),
            end: utc(end),
            timezone: "UTC".to_string(),
            resolution_source: AssistantTimeWindowResolutionSource::RelativeDate,
        };
        window_from_semantic_time_window(&semantic)
    }

    #[test]
    fn deterministic_fallback_uses_window_label_for_no_events() {
        let payload = deterministic_calendar_fallback_payload(
            &window("2026-02-17T00:00:00Z", "2026-02-24T00:00:00Z"),
            &[],
        );

        assert_eq!(payload.title, "No meetings in range");
        assert!(payload.summary.contains("2026-02-17 00:00"));
        assert!(payload.key_points.is_empty());
    }

    #[test]
    fn deterministic_fallback_is_grounded_to_event_times() {
        let meetings = vec![GoogleCalendarMeetingSource {
            event_id: Some("event-1".to_string()),
            title: Some("Team Sync".to_string()),
            start_at: Some(utc("2026-02-17T16:30:00Z")),
            end_at: None,
            attendee_emails: vec![],
        }];

        let payload = deterministic_calendar_fallback_payload(
            &window("2026-02-17T00:00:00Z", "2026-02-18T00:00:00Z"),
            &meetings,
        );
        assert_eq!(payload.title, "Meetings in range");
        assert!(payload.summary.contains("2026-02-17 00:00"));
        assert_eq!(
            payload.key_points,
            vec!["16:30 UTC - Team Sync".to_string()]
        );
    }
}
