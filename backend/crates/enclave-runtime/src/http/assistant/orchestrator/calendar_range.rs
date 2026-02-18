use chrono::{DateTime, Datelike, Duration, LocalResult, NaiveDate, TimeZone, Utc};
use shared::timezone::{parse_time_zone_or_default, user_local_date};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CalendarQueryWindow {
    pub(super) time_min: DateTime<Utc>,
    pub(super) time_max: DateTime<Utc>,
    pub(super) label: String,
}

pub(super) fn plan_calendar_query_window(
    query: &str,
    now: DateTime<Utc>,
    user_time_zone: &str,
) -> Option<CalendarQueryWindow> {
    let normalized = query.to_ascii_lowercase();
    let today = user_local_date(now, user_time_zone);

    if let Some(explicit_date) = parse_explicit_date(&normalized) {
        return window_for_single_day(explicit_date, explicit_date.to_string(), user_time_zone);
    }

    if normalized.contains("tomorrow") {
        return window_for_single_day(
            today + Duration::days(1),
            "tomorrow".to_string(),
            user_time_zone,
        );
    }

    if normalized.contains("next 7 days") || normalized.contains("next seven days") {
        return window_for_day_span(today, 7, "next 7 days".to_string(), user_time_zone);
    }

    if normalized.contains("next week") {
        let this_week_start = start_of_week(today);
        return window_for_day_span(
            this_week_start + Duration::days(7),
            7,
            "next week".to_string(),
            user_time_zone,
        );
    }

    if normalized.contains("this week") {
        return window_for_day_span(
            start_of_week(today),
            7,
            "this week".to_string(),
            user_time_zone,
        );
    }

    if normalized.contains("today") {
        return window_for_single_day(today, "today".to_string(), user_time_zone);
    }

    window_for_single_day(today, "today".to_string(), user_time_zone)
}

fn start_of_week(date: NaiveDate) -> NaiveDate {
    date - Duration::days(date.weekday().num_days_from_monday() as i64)
}

fn window_for_single_day(
    date: NaiveDate,
    label: String,
    user_time_zone: &str,
) -> Option<CalendarQueryWindow> {
    window_for_day_span(date, 1, label, user_time_zone)
}

fn window_for_day_span(
    start_date: NaiveDate,
    day_count: i64,
    label: String,
    user_time_zone: &str,
) -> Option<CalendarQueryWindow> {
    if day_count <= 0 {
        return None;
    }

    let time_min = at_local_midnight_utc(start_date, user_time_zone)?;
    let time_max = at_local_midnight_utc(start_date + Duration::days(day_count), user_time_zone)?;

    Some(CalendarQueryWindow {
        time_min,
        time_max,
        label,
    })
}

fn at_local_midnight_utc(date: NaiveDate, user_time_zone: &str) -> Option<DateTime<Utc>> {
    let local_midnight = date.and_hms_opt(0, 0, 0)?;
    let time_zone = parse_time_zone_or_default(user_time_zone);

    match time_zone.from_local_datetime(&local_midnight) {
        LocalResult::Single(value) => Some(value.with_timezone(&Utc)),
        LocalResult::Ambiguous(earliest, _) => Some(earliest.with_timezone(&Utc)),
        LocalResult::None => None,
    }
}

fn parse_explicit_date(query: &str) -> Option<NaiveDate> {
    query
        .split_whitespace()
        .map(|token| {
            token.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '/')
        })
        .find_map(|candidate| {
            if candidate.is_empty() {
                return None;
            }

            NaiveDate::parse_from_str(candidate, "%Y-%m-%d")
                .ok()
                .or_else(|| NaiveDate::parse_from_str(candidate, "%m/%d/%Y").ok())
        })
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};

    use super::plan_calendar_query_window;

    fn utc(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("timestamp should parse")
            .with_timezone(&Utc)
    }

    #[test]
    fn plan_calendar_window_handles_today_and_tomorrow() {
        let now = utc("2026-02-17T10:15:00Z");

        let today = plan_calendar_query_window("what meetings are today", now, "UTC")
            .expect("today window should resolve");
        assert_eq!(today.label, "today");
        assert_eq!(today.time_min.to_rfc3339(), "2026-02-17T00:00:00+00:00");
        assert_eq!(today.time_max.to_rfc3339(), "2026-02-18T00:00:00+00:00");

        let tomorrow = plan_calendar_query_window("what is on my calendar tomorrow?", now, "UTC")
            .expect("tomorrow window should resolve");
        assert_eq!(tomorrow.label, "tomorrow");
        assert_eq!(tomorrow.time_min.to_rfc3339(), "2026-02-18T00:00:00+00:00");
        assert_eq!(tomorrow.time_max.to_rfc3339(), "2026-02-19T00:00:00+00:00");
    }

    #[test]
    fn plan_calendar_window_handles_next_7_days_and_week_ranges() {
        let now = utc("2026-02-17T10:15:00Z");

        let next_7_days = plan_calendar_query_window("show calendar for next 7 days", now, "UTC")
            .expect("next 7 days window should resolve");
        assert_eq!(next_7_days.label, "next 7 days");
        assert_eq!(
            next_7_days.time_min.to_rfc3339(),
            "2026-02-17T00:00:00+00:00"
        );
        assert_eq!(
            next_7_days.time_max.to_rfc3339(),
            "2026-02-24T00:00:00+00:00"
        );

        let this_week = plan_calendar_query_window("what is on my calendar this week", now, "UTC")
            .expect("this week window should resolve");
        assert_eq!(this_week.label, "this week");
        assert_eq!(this_week.time_min.to_rfc3339(), "2026-02-16T00:00:00+00:00");
        assert_eq!(this_week.time_max.to_rfc3339(), "2026-02-23T00:00:00+00:00");

        let next_week = plan_calendar_query_window("anything next week?", now, "UTC")
            .expect("next week window should resolve");
        assert_eq!(next_week.label, "next week");
        assert_eq!(next_week.time_min.to_rfc3339(), "2026-02-23T00:00:00+00:00");
        assert_eq!(next_week.time_max.to_rfc3339(), "2026-03-02T00:00:00+00:00");
    }

    #[test]
    fn plan_calendar_window_supports_explicit_date_formats() {
        let now = utc("2026-02-17T10:15:00Z");

        let iso_date = plan_calendar_query_window("meetings on 2026-02-21", now, "UTC")
            .expect("explicit iso date should resolve");
        assert_eq!(iso_date.label, "2026-02-21");
        assert_eq!(iso_date.time_min.to_rfc3339(), "2026-02-21T00:00:00+00:00");
        assert_eq!(iso_date.time_max.to_rfc3339(), "2026-02-22T00:00:00+00:00");

        let us_date = plan_calendar_query_window("calendar for 02/22/2026", now, "UTC")
            .expect("explicit us date should resolve");
        assert_eq!(us_date.label, "2026-02-22");
        assert_eq!(us_date.time_min.to_rfc3339(), "2026-02-22T00:00:00+00:00");
        assert_eq!(us_date.time_max.to_rfc3339(), "2026-02-23T00:00:00+00:00");
    }

    #[test]
    fn plan_calendar_window_uses_user_local_timezone_for_today() {
        let now = utc("2026-02-17T04:15:00Z");
        let today =
            plan_calendar_query_window("what meetings are today", now, "America/Los_Angeles")
                .expect("today window should resolve");

        assert_eq!(today.label, "today");
        assert_eq!(today.time_min.to_rfc3339(), "2026-02-16T08:00:00+00:00");
        assert_eq!(today.time_max.to_rfc3339(), "2026-02-17T08:00:00+00:00");
    }

    #[test]
    fn plan_calendar_window_treats_afternoon_and_tonight_as_today() {
        let now = utc("2026-02-17T10:15:00Z");

        let afternoon =
            plan_calendar_query_window("what is on my agenda this afternoon?", now, "UTC")
                .expect("afternoon window should resolve");
        assert_eq!(afternoon.label, "today");
        assert_eq!(afternoon.time_min.to_rfc3339(), "2026-02-17T00:00:00+00:00");
        assert_eq!(afternoon.time_max.to_rfc3339(), "2026-02-18T00:00:00+00:00");

        let tonight = plan_calendar_query_window("do I have events tonight?", now, "UTC")
            .expect("tonight window should resolve");
        assert_eq!(tonight.label, "today");
        assert_eq!(tonight.time_min.to_rfc3339(), "2026-02-17T00:00:00+00:00");
        assert_eq!(tonight.time_max.to_rfc3339(), "2026-02-18T00:00:00+00:00");
    }
}
