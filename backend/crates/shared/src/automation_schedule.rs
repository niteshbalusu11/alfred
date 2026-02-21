use chrono::{
    DateTime, Datelike, Days, Duration, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, TimeZone,
    Utc,
};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

use crate::timezone::normalize_time_zone;

const MAX_DST_FORWARD_SHIFT_MINUTES: i64 = 180;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AutomationScheduleType {
    Daily,
    Weekly,
    Monthly,
    Annually,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutomationScheduleSpec {
    pub schedule_type: AutomationScheduleType,
    pub time_zone: String,
    pub local_time_minutes: u16,
    pub anchor_day_of_week: Option<u8>,
    pub anchor_day_of_month: Option<u8>,
    pub anchor_month: Option<u8>,
}

impl AutomationScheduleSpec {
    pub fn local_time_hhmm(&self) -> String {
        format_local_time_hhmm(self.local_time_minutes)
    }
}

pub fn parse_local_time_hhmm(value: &str) -> Option<u16> {
    let trimmed = value.trim();
    let (hour, minute) = trimmed.split_once(':')?;
    if hour.len() != 2 || minute.len() != 2 {
        return None;
    }

    let hour: u16 = hour.parse().ok()?;
    let minute: u16 = minute.parse().ok()?;
    if hour > 23 || minute > 59 {
        return None;
    }

    Some((hour * 60) + minute)
}

pub fn format_local_time_hhmm(minutes: u16) -> String {
    let hour = minutes / 60;
    let minute = minutes % 60;
    format!("{hour:02}:{minute:02}")
}

pub fn interval_seconds_hint(schedule_type: AutomationScheduleType) -> i32 {
    match schedule_type {
        AutomationScheduleType::Daily => 86_400,
        AutomationScheduleType::Weekly => 604_800,
        AutomationScheduleType::Monthly => 2_629_746,
        AutomationScheduleType::Annually => 31_556_952,
    }
}

pub fn build_schedule_spec(
    schedule_type: AutomationScheduleType,
    time_zone: &str,
    local_time_minutes: u16,
    reference_utc: DateTime<Utc>,
) -> Result<AutomationScheduleSpec, String> {
    let Some(normalized_time_zone) = normalize_time_zone(time_zone) else {
        return Err("time_zone is not a valid IANA timezone".to_string());
    };

    if local_time_minutes > 1_439 {
        return Err("local_time must be between 00:00 and 23:59".to_string());
    }

    let tz = normalized_time_zone
        .parse::<Tz>()
        .map_err(|_| "time_zone is not a valid IANA timezone".to_string())?;
    let local_date = reference_utc.with_timezone(&tz).date_naive();

    let (anchor_day_of_week, anchor_day_of_month, anchor_month) = match schedule_type {
        AutomationScheduleType::Daily => (None, None, None),
        AutomationScheduleType::Weekly => {
            let day = u8::try_from(local_date.weekday().number_from_monday())
                .map_err(|_| "failed to derive weekly anchor day".to_string())?;
            (Some(day), None, None)
        }
        AutomationScheduleType::Monthly => {
            let day = u8::try_from(local_date.day())
                .map_err(|_| "failed to derive monthly anchor day".to_string())?;
            (None, Some(day), None)
        }
        AutomationScheduleType::Annually => {
            let day = u8::try_from(local_date.day())
                .map_err(|_| "failed to derive annual anchor day".to_string())?;
            let month = u8::try_from(local_date.month())
                .map_err(|_| "failed to derive annual anchor month".to_string())?;
            (None, Some(day), Some(month))
        }
    };

    let spec = AutomationScheduleSpec {
        schedule_type,
        time_zone: normalized_time_zone,
        local_time_minutes,
        anchor_day_of_week,
        anchor_day_of_month,
        anchor_month,
    };
    validate_schedule_spec(&spec)?;
    Ok(spec)
}

pub fn validate_schedule_spec(spec: &AutomationScheduleSpec) -> Result<(), String> {
    if normalize_time_zone(spec.time_zone.as_str()).is_none() {
        return Err("time_zone is not a valid IANA timezone".to_string());
    }

    if spec.local_time_minutes > 1_439 {
        return Err("local_time must be between 00:00 and 23:59".to_string());
    }

    match spec.schedule_type {
        AutomationScheduleType::Daily => {
            if spec.anchor_day_of_week.is_some()
                || spec.anchor_day_of_month.is_some()
                || spec.anchor_month.is_some()
            {
                return Err("daily schedules must not include anchor fields".to_string());
            }
        }
        AutomationScheduleType::Weekly => {
            let Some(day_of_week) = spec.anchor_day_of_week else {
                return Err("weekly schedules require anchor_day_of_week".to_string());
            };
            if !(1..=7).contains(&day_of_week) {
                return Err("anchor_day_of_week must be between 1 and 7".to_string());
            }
            if spec.anchor_day_of_month.is_some() || spec.anchor_month.is_some() {
                return Err(
                    "weekly schedules must not include month/day-of-month anchors".to_string(),
                );
            }
        }
        AutomationScheduleType::Monthly => {
            let Some(day_of_month) = spec.anchor_day_of_month else {
                return Err("monthly schedules require anchor_day_of_month".to_string());
            };
            if !(1..=31).contains(&day_of_month) {
                return Err("anchor_day_of_month must be between 1 and 31".to_string());
            }
            if spec.anchor_day_of_week.is_some() || spec.anchor_month.is_some() {
                return Err("monthly schedules must not include weekly/annual anchors".to_string());
            }
        }
        AutomationScheduleType::Annually => {
            let Some(day_of_month) = spec.anchor_day_of_month else {
                return Err("annual schedules require anchor_day_of_month".to_string());
            };
            let Some(month) = spec.anchor_month else {
                return Err("annual schedules require anchor_month".to_string());
            };
            if !(1..=31).contains(&day_of_month) {
                return Err("anchor_day_of_month must be between 1 and 31".to_string());
            }
            if !(1..=12).contains(&month) {
                return Err("anchor_month must be between 1 and 12".to_string());
            }
            if spec.anchor_day_of_week.is_some() {
                return Err("annual schedules must not include weekly anchors".to_string());
            }
        }
    }

    Ok(())
}

pub fn next_run_after(
    reference_utc: DateTime<Utc>,
    spec: &AutomationScheduleSpec,
) -> Option<DateTime<Utc>> {
    validate_schedule_spec(spec).ok()?;
    let tz = spec.time_zone.parse::<Tz>().ok()?;
    let local_time = local_time_from_minutes(spec.local_time_minutes)?;

    let mut cursor_utc = reference_utc;
    for _ in 0..4 {
        let local_reference = cursor_utc.with_timezone(&tz).naive_local();
        let candidate_local = next_local_candidate(local_reference, local_time, spec)?;
        let candidate_utc = resolve_local_datetime_to_utc(&tz, candidate_local)?;
        if candidate_utc > reference_utc {
            return Some(candidate_utc);
        }
        cursor_utc += Duration::minutes(1);
    }

    None
}

fn next_local_candidate(
    local_reference: NaiveDateTime,
    local_time: NaiveTime,
    spec: &AutomationScheduleSpec,
) -> Option<NaiveDateTime> {
    match spec.schedule_type {
        AutomationScheduleType::Daily => {
            let mut candidate_date = local_reference.date();
            let mut candidate = candidate_date.and_time(local_time);
            if candidate <= local_reference {
                candidate_date = candidate_date.checked_add_days(Days::new(1))?;
                candidate = candidate_date.and_time(local_time);
            }
            Some(candidate)
        }
        AutomationScheduleType::Weekly => {
            let target_day = u32::from(spec.anchor_day_of_week?);
            let current_day = local_reference.date().weekday().number_from_monday();
            let mut days_until = i64::from(target_day) - i64::from(current_day);
            if days_until < 0 {
                days_until += 7;
            }

            let mut candidate_date = local_reference
                .date()
                .checked_add_days(Days::new(u64::try_from(days_until).ok()?))?;
            let mut candidate = candidate_date.and_time(local_time);
            if candidate <= local_reference {
                candidate_date = candidate_date.checked_add_days(Days::new(7))?;
                candidate = candidate_date.and_time(local_time);
            }
            Some(candidate)
        }
        AutomationScheduleType::Monthly => {
            let anchor_day = u32::from(spec.anchor_day_of_month?);
            let mut year = local_reference.date().year();
            let mut month = local_reference.date().month();

            let mut candidate_date = date_with_clamped_day(year, month, anchor_day)?;
            let mut candidate = candidate_date.and_time(local_time);
            if candidate <= local_reference {
                let (next_year, next_month) = next_month(year, month);
                year = next_year;
                month = next_month;
                candidate_date = date_with_clamped_day(year, month, anchor_day)?;
                candidate = candidate_date.and_time(local_time);
            }
            Some(candidate)
        }
        AutomationScheduleType::Annually => {
            let anchor_day = u32::from(spec.anchor_day_of_month?);
            let anchor_month = u32::from(spec.anchor_month?);

            let mut year = local_reference.date().year();
            let mut candidate_date = date_with_clamped_day(year, anchor_month, anchor_day)?;
            let mut candidate = candidate_date.and_time(local_time);
            if candidate <= local_reference {
                year += 1;
                candidate_date = date_with_clamped_day(year, anchor_month, anchor_day)?;
                candidate = candidate_date.and_time(local_time);
            }
            Some(candidate)
        }
    }
}

fn local_time_from_minutes(minutes: u16) -> Option<NaiveTime> {
    if minutes > 1_439 {
        return None;
    }

    let hour = u32::from(minutes / 60);
    let minute = u32::from(minutes % 60);
    NaiveTime::from_hms_opt(hour, minute, 0)
}

fn resolve_local_datetime_to_utc(tz: &Tz, local: NaiveDateTime) -> Option<DateTime<Utc>> {
    match tz.from_local_datetime(&local) {
        LocalResult::Single(value) => Some(value.with_timezone(&Utc)),
        LocalResult::Ambiguous(earliest, _) => Some(earliest.with_timezone(&Utc)),
        LocalResult::None => {
            for minute_offset in 1..=MAX_DST_FORWARD_SHIFT_MINUTES {
                let shifted = local.checked_add_signed(Duration::minutes(minute_offset))?;
                match tz.from_local_datetime(&shifted) {
                    LocalResult::Single(value) => return Some(value.with_timezone(&Utc)),
                    LocalResult::Ambiguous(earliest, _) => {
                        return Some(earliest.with_timezone(&Utc));
                    }
                    LocalResult::None => continue,
                }
            }
            None
        }
    }
}

fn date_with_clamped_day(year: i32, month: u32, day: u32) -> Option<NaiveDate> {
    let days_in_month = days_in_month(year, month)?;
    let clamped_day = day.min(days_in_month);
    NaiveDate::from_ymd_opt(year, month, clamped_day)
}

fn days_in_month(year: i32, month: u32) -> Option<u32> {
    if !(1..=12).contains(&month) {
        return None;
    }

    let first_of_month = NaiveDate::from_ymd_opt(year, month, 1)?;
    let (next_year, next_month) = next_month(year, month);
    let first_of_next_month = NaiveDate::from_ymd_opt(next_year, next_month, 1)?;
    let days = (first_of_next_month - first_of_month).num_days();
    u32::try_from(days).ok()
}

fn next_month(year: i32, month: u32) -> (i32, u32) {
    if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::{
        AutomationScheduleSpec, AutomationScheduleType, build_schedule_spec, next_run_after,
        parse_local_time_hhmm,
    };

    #[test]
    fn parse_local_time_hhmm_rejects_invalid_values() {
        assert_eq!(parse_local_time_hhmm("09:45"), Some(585));
        assert_eq!(parse_local_time_hhmm("9:45"), None);
        assert_eq!(parse_local_time_hhmm("24:00"), None);
        assert_eq!(parse_local_time_hhmm("12:60"), None);
    }

    #[test]
    fn daily_schedule_uses_next_day_when_time_has_passed() {
        let reference = Utc
            .with_ymd_and_hms(2026, 2, 20, 18, 0, 0)
            .single()
            .expect("valid datetime");
        let spec = build_schedule_spec(AutomationScheduleType::Daily, "UTC", 9 * 60, reference)
            .expect("valid schedule");

        let next = next_run_after(reference, &spec).expect("next run should exist");
        assert_eq!(next.to_rfc3339(), "2026-02-21T09:00:00+00:00");
    }

    #[test]
    fn monthly_schedule_preserves_anchor_day_when_months_are_shorter() {
        let spec = AutomationScheduleSpec {
            schedule_type: AutomationScheduleType::Monthly,
            time_zone: "UTC".to_string(),
            local_time_minutes: 10 * 60,
            anchor_day_of_week: None,
            anchor_day_of_month: Some(31),
            anchor_month: None,
        };

        let jan_31 = Utc
            .with_ymd_and_hms(2026, 1, 31, 10, 0, 0)
            .single()
            .expect("valid datetime");
        let feb_run = next_run_after(jan_31, &spec).expect("next run should exist");
        assert_eq!(feb_run.to_rfc3339(), "2026-02-28T10:00:00+00:00");

        let mar_run = next_run_after(feb_run, &spec).expect("next run should exist");
        assert_eq!(mar_run.to_rfc3339(), "2026-03-31T10:00:00+00:00");
    }
}
