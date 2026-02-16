use chrono::{DateTime, Days, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;

pub const DEFAULT_USER_TIME_ZONE: &str = "UTC";

pub fn normalize_time_zone(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    trimmed.parse::<Tz>().ok().map(|tz| tz.name().to_string())
}

pub fn parse_time_zone_or_default(value: &str) -> Tz {
    normalize_time_zone(value)
        .and_then(|normalized| normalized.parse::<Tz>().ok())
        .unwrap_or(chrono_tz::UTC)
}

pub fn user_local_date(now_utc: DateTime<Utc>, time_zone: &str) -> NaiveDate {
    let tz = parse_time_zone_or_default(time_zone);
    now_utc.with_timezone(&tz).date_naive()
}

pub fn user_local_time(now_utc: DateTime<Utc>, time_zone: &str) -> NaiveTime {
    let tz = parse_time_zone_or_default(time_zone);
    now_utc.with_timezone(&tz).time()
}

pub fn local_day_bounds_utc(
    local_date: NaiveDate,
    time_zone: &str,
) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let start_of_day = local_date.and_hms_opt(0, 0, 0)?;
    let next_day = local_date.checked_add_days(Days::new(1))?;
    let start_of_next_day = next_day.and_hms_opt(0, 0, 0)?;

    let tz = parse_time_zone_or_default(time_zone);
    let local_start = resolve_local_datetime(&tz, start_of_day)?;
    let local_end = resolve_local_datetime(&tz, start_of_next_day)?;

    Some((
        local_start.with_timezone(&Utc),
        local_end.with_timezone(&Utc),
    ))
}

fn resolve_local_datetime(tz: &Tz, local: NaiveDateTime) -> Option<DateTime<Tz>> {
    match tz.from_local_datetime(&local) {
        LocalResult::Single(value) => Some(value),
        LocalResult::Ambiguous(earliest, _) => Some(earliest),
        LocalResult::None => None,
    }
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, TimeZone, Timelike, Utc};

    use super::{
        DEFAULT_USER_TIME_ZONE, local_day_bounds_utc, normalize_time_zone, user_local_date,
        user_local_time,
    };

    #[test]
    fn normalize_time_zone_accepts_valid_iana_name() {
        assert_eq!(
            normalize_time_zone("America/Los_Angeles"),
            Some("America/Los_Angeles".to_string())
        );
    }

    #[test]
    fn normalize_time_zone_rejects_invalid_values() {
        assert_eq!(normalize_time_zone(""), None);
        assert_eq!(normalize_time_zone("Mars/Olympus"), None);
    }

    #[test]
    fn user_local_date_uses_default_when_time_zone_is_invalid() {
        let now = Utc
            .with_ymd_and_hms(2026, 2, 17, 1, 15, 0)
            .single()
            .expect("valid utc datetime");
        let local_date = user_local_date(now, "not-a-time-zone");
        assert_eq!(local_date.to_string(), "2026-02-17");
        assert_eq!(DEFAULT_USER_TIME_ZONE, "UTC");
    }

    #[test]
    fn local_day_bounds_convert_local_midnight_to_utc() {
        let local_date = NaiveDate::from_ymd_opt(2026, 2, 17).expect("valid local date");
        let (start_utc, end_utc) =
            local_day_bounds_utc(local_date, "America/Los_Angeles").expect("time bounds");

        assert_eq!(start_utc.date_naive().to_string(), "2026-02-17");
        assert_eq!(start_utc.hour(), 8);
        assert_eq!(end_utc.hour(), 8);
    }

    #[test]
    fn user_local_time_converts_from_utc() {
        let now = Utc
            .with_ymd_and_hms(2026, 2, 17, 9, 30, 0)
            .single()
            .expect("valid utc datetime");
        let local_time = user_local_time(now, "America/New_York");
        assert_eq!(local_time.format("%H:%M").to_string(), "04:30");
    }
}
