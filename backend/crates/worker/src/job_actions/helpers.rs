use chrono::NaiveTime;
use serde::Deserialize;

use crate::{JobExecutionError, NotificationContent};

#[derive(Debug, Deserialize)]
struct NotificationJobPayload {
    notification: Option<NotificationPayloadBody>,
}

#[derive(Debug, Deserialize)]
struct NotificationPayloadBody {
    title: String,
    body: String,
}

pub(super) fn parse_notification_payload(payload: Option<&[u8]>) -> Option<NotificationContent> {
    let payload = payload?;
    let parsed: NotificationJobPayload = serde_json::from_slice(payload).ok()?;
    let notification = parsed.notification?;

    let title = notification.title.trim();
    let body = notification.body.trim();

    if title.is_empty() || body.is_empty() {
        return None;
    }

    Some(NotificationContent {
        title: title.to_string(),
        body: body.to_string(),
    })
}

pub(super) fn parse_simulated_failure(payload: Option<&[u8]>) -> Option<JobExecutionError> {
    let payload = payload?;
    let text = std::str::from_utf8(payload).ok()?;

    let mut parts = text.splitn(4, ':');
    if parts.next()? != "simulate-failure" {
        return None;
    }

    let class = parts.next()?;
    let code = parts.next()?.trim();
    let message = parts.next()?.trim();

    match class {
        "transient" => Some(JobExecutionError::transient(code, message)),
        "permanent" => Some(JobExecutionError::permanent(code, message)),
        _ => None,
    }
}

pub(super) fn is_within_quiet_hours(
    now: NaiveTime,
    start: &str,
    end: &str,
) -> Result<bool, String> {
    let start = parse_hhmm(start)?;
    let end = parse_hhmm(end)?;

    if start == end {
        return Ok(true);
    }

    if start < end {
        Ok(now >= start && now < end)
    } else {
        Ok(now >= start || now < end)
    }
}

fn parse_hhmm(value: &str) -> Result<NaiveTime, String> {
    NaiveTime::parse_from_str(value, "%H:%M")
        .map_err(|_| format!("time must be in HH:MM format: {value}"))
}

#[cfg(test)]
mod tests {
    use chrono::NaiveTime;

    use super::{is_within_quiet_hours, parse_simulated_failure};

    #[test]
    fn quiet_hours_supports_wrapped_ranges() {
        let before_midnight = NaiveTime::from_hms_opt(23, 15, 0).expect("valid time");
        let after_midnight = NaiveTime::from_hms_opt(6, 45, 0).expect("valid time");
        let outside = NaiveTime::from_hms_opt(14, 0, 0).expect("valid time");

        assert!(is_within_quiet_hours(before_midnight, "22:00", "07:00").expect("valid range"));
        assert!(is_within_quiet_hours(after_midnight, "22:00", "07:00").expect("valid range"));
        assert!(!is_within_quiet_hours(outside, "22:00", "07:00").expect("valid range"));
    }

    #[test]
    fn quiet_hours_supports_non_wrapped_ranges() {
        let in_range = NaiveTime::from_hms_opt(13, 0, 0).expect("valid time");
        let out_of_range = NaiveTime::from_hms_opt(17, 0, 0).expect("valid time");

        assert!(is_within_quiet_hours(in_range, "12:00", "14:00").expect("valid range"));
        assert!(!is_within_quiet_hours(out_of_range, "12:00", "14:00").expect("valid range"));
    }

    #[test]
    fn quiet_hours_with_equal_bounds_suppresses_all_day() {
        let now = NaiveTime::from_hms_opt(9, 30, 0).expect("valid time");
        assert!(is_within_quiet_hours(now, "08:00", "08:00").expect("valid range"));
    }

    #[test]
    fn simulated_failures_are_parsed() {
        let transient = parse_simulated_failure(Some(b"simulate-failure:transient:TEMP:retry"))
            .expect("transient error");
        assert_eq!(transient.code, "TEMP");

        let permanent = parse_simulated_failure(Some(b"simulate-failure:permanent:FATAL:stop"))
            .expect("permanent error");
        assert_eq!(permanent.code, "FATAL");
    }
}
