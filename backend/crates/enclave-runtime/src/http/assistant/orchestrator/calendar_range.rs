use chrono::{DateTime, Duration, Utc};
use shared::assistant_semantic_plan::AssistantSemanticTimeWindow;
use shared::timezone::parse_time_zone_or_default;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CalendarQueryWindow {
    pub(super) time_min: DateTime<Utc>,
    pub(super) time_max: DateTime<Utc>,
    pub(super) label: String,
}

pub(super) fn calendar_window_from_semantic_time_window(
    time_window: &AssistantSemanticTimeWindow,
) -> Option<CalendarQueryWindow> {
    if time_window.end <= time_window.start {
        return None;
    }

    let time_zone = parse_time_zone_or_default(time_window.timezone.as_str());
    let start_local = time_window.start.with_timezone(&time_zone).date_naive();
    let end_local_exclusive = time_window.end.with_timezone(&time_zone).date_naive();
    let span_days = (end_local_exclusive - start_local).num_days();
    let label = if span_days <= 1 {
        start_local.to_string()
    } else {
        format!(
            "{} to {}",
            start_local,
            end_local_exclusive - Duration::days(1)
        )
    };

    Some(CalendarQueryWindow {
        time_min: time_window.start,
        time_max: time_window.end,
        label,
    })
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use shared::assistant_semantic_plan::{
        AssistantSemanticTimeWindow, AssistantTimeWindowResolutionSource,
    };

    use super::calendar_window_from_semantic_time_window;

    fn utc(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("timestamp should parse")
            .with_timezone(&Utc)
    }

    #[test]
    fn calendar_window_from_semantic_window_uses_requested_bounds_and_label() {
        let semantic_window = AssistantSemanticTimeWindow {
            start: utc("2026-02-17T08:00:00Z"),
            end: utc("2026-02-24T08:00:00Z"),
            timezone: "America/Los_Angeles".to_string(),
            resolution_source: AssistantTimeWindowResolutionSource::RelativeDate,
        };

        let window = calendar_window_from_semantic_time_window(&semantic_window)
            .expect("semantic time window should convert");
        assert_eq!(window.time_min.to_rfc3339(), "2026-02-17T08:00:00+00:00");
        assert_eq!(window.time_max.to_rfc3339(), "2026-02-24T08:00:00+00:00");
        assert_eq!(window.label, "2026-02-17 to 2026-02-23");
    }

    #[test]
    fn calendar_window_from_semantic_window_rejects_invalid_order() {
        let semantic_window = AssistantSemanticTimeWindow {
            start: utc("2026-02-24T08:00:00Z"),
            end: utc("2026-02-17T08:00:00Z"),
            timezone: "UTC".to_string(),
            resolution_source: AssistantTimeWindowResolutionSource::RelativeDate,
        };

        assert!(calendar_window_from_semantic_time_window(&semantic_window).is_none());
    }
}
