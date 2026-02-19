use chrono::{DateTime, Utc};
use shared::assistant_semantic_plan::AssistantSemanticTimeWindow;
use shared::timezone::parse_time_zone_or_default;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CalendarQueryWindow {
    pub(super) time_min: DateTime<Utc>,
    pub(super) time_max: DateTime<Utc>,
    pub(super) label: String,
}

pub(super) fn window_from_semantic_time_window(
    time_window: &AssistantSemanticTimeWindow,
) -> CalendarQueryWindow {
    CalendarQueryWindow {
        time_min: time_window.start,
        time_max: time_window.end,
        label: window_label(
            time_window.start,
            time_window.end,
            time_window.timezone.as_str(),
        ),
    }
}

pub(super) fn window_label(start: DateTime<Utc>, end: DateTime<Utc>, time_zone: &str) -> String {
    let tz = parse_time_zone_or_default(time_zone);
    let start_local = start.with_timezone(&tz);
    let end_local = end.with_timezone(&tz);

    format!(
        "{} to {} ({})",
        start_local.format("%Y-%m-%d %H:%M"),
        end_local.format("%Y-%m-%d %H:%M"),
        tz.name()
    )
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use shared::assistant_semantic_plan::{
        AssistantSemanticTimeWindow, AssistantTimeWindowResolutionSource,
    };

    use super::{window_from_semantic_time_window, window_label};

    fn utc(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("timestamp should parse")
            .with_timezone(&Utc)
    }

    fn semantic_time_window(start: &str, end: &str, timezone: &str) -> AssistantSemanticTimeWindow {
        AssistantSemanticTimeWindow {
            start: utc(start),
            end: utc(end),
            timezone: timezone.to_string(),
            resolution_source: AssistantTimeWindowResolutionSource::RelativeDate,
        }
    }

    #[test]
    fn window_from_semantic_time_window_keeps_exact_bounds() {
        let semantic = semantic_time_window(
            "2026-02-17T08:00:00Z",
            "2026-02-18T08:00:00Z",
            "America/Los_Angeles",
        );

        let window = window_from_semantic_time_window(&semantic);
        assert_eq!(window.time_min.to_rfc3339(), "2026-02-17T08:00:00+00:00");
        assert_eq!(window.time_max.to_rfc3339(), "2026-02-18T08:00:00+00:00");
    }

    #[test]
    fn window_label_uses_localized_absolute_timestamps() {
        let label = window_label(
            utc("2026-02-17T08:00:00Z"),
            utc("2026-02-17T20:00:00Z"),
            "America/Los_Angeles",
        );

        assert_eq!(
            label,
            "2026-02-17 00:00 to 2026-02-17 12:00 (America/Los_Angeles)"
        );
    }
}
