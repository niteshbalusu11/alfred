use std::cmp::Ordering;

use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use shared::timezone::{local_day_bounds_utc, user_local_date};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EmailWindowKind {
    Today,
    ThisWeek,
    PastDays(i64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EmailQueryPlan {
    pub(super) sender_filter: Option<String>,
    pub(super) lookback_days: i64,
    pub(super) window_label: &'static str,
    window_kind: EmailWindowKind,
}

pub(super) fn build_gmail_query(plan: &EmailQueryPlan) -> String {
    let mut parts = vec![format!("newer_than:{}d", plan.lookback_days)];
    if let Some(sender_filter) = &plan.sender_filter {
        parts.push(format!("from:{sender_filter}"));
    }
    parts.join(" ")
}

pub(super) fn plan_email_query(query: &str) -> EmailQueryPlan {
    let normalized = query.to_ascii_lowercase();

    let (lookback_days, window_label, window_kind) = if normalized.contains("today") {
        (1, "today", EmailWindowKind::Today)
    } else if normalized.contains("this week") {
        (7, "this week", EmailWindowKind::ThisWeek)
    } else if normalized.contains("next 7 days")
        || normalized.contains("last 7 days")
        || normalized.contains("week")
    {
        (7, "the past 7 days", EmailWindowKind::PastDays(7))
    } else if normalized.contains("month") || normalized.contains("30 days") {
        (30, "the past 30 days", EmailWindowKind::PastDays(30))
    } else {
        (7, "the past 7 days", EmailWindowKind::PastDays(7))
    };

    EmailQueryPlan {
        sender_filter: parse_sender_filter(query),
        lookback_days,
        window_label,
        window_kind,
    }
}

pub(super) fn apply_email_filters(
    mut candidates: Vec<shared::llm::GoogleEmailCandidateSource>,
    plan: &EmailQueryPlan,
    now: DateTime<Utc>,
    user_time_zone: &str,
) -> Vec<shared::llm::GoogleEmailCandidateSource> {
    let min_received_at = window_start_utc(plan, now, user_time_zone)
        .unwrap_or_else(|| now - Duration::days(plan.lookback_days));

    candidates.retain(|candidate| {
        let sender_match = plan
            .sender_filter
            .as_ref()
            .map(|sender_filter| {
                candidate
                    .from
                    .as_deref()
                    .unwrap_or("")
                    .to_ascii_lowercase()
                    .contains(sender_filter)
            })
            .unwrap_or(true);

        let time_match = candidate
            .received_at
            .map(|received| received >= min_received_at && received <= now)
            .unwrap_or(false);

        sender_match && time_match
    });

    candidates.sort_by(|left, right| match (left.received_at, right.received_at) {
        (Some(left_received), Some(right_received)) => right_received.cmp(&left_received),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    });

    candidates
}

pub(super) fn window_start_utc(
    plan: &EmailQueryPlan,
    now: DateTime<Utc>,
    user_time_zone: &str,
) -> Option<DateTime<Utc>> {
    let local_today = user_local_date(now, user_time_zone);

    let start_date = match plan.window_kind {
        EmailWindowKind::Today => local_today,
        EmailWindowKind::ThisWeek => start_of_week(local_today),
        EmailWindowKind::PastDays(days) => local_today - Duration::days(days.saturating_sub(1)),
    };

    local_day_bounds_utc(start_date, user_time_zone).map(|(start, _)| start)
}

fn start_of_week(date: NaiveDate) -> NaiveDate {
    date - Duration::days(date.weekday().num_days_from_monday() as i64)
}

fn parse_sender_filter(query: &str) -> Option<String> {
    let markers = ["from ", "sender "];
    let normalized = query.to_ascii_lowercase();

    for marker in markers {
        if let Some(start) = normalized.find(marker) {
            let rest = &query[start + marker.len()..];
            let raw = rest.split_whitespace().next().unwrap_or("");
            let cleaned = raw
                .trim_matches(|c: char| {
                    !c.is_ascii_alphanumeric()
                        && c != '@'
                        && c != '.'
                        && c != '_'
                        && c != '-'
                        && c != '+'
                })
                .to_ascii_lowercase();
            if !cleaned.is_empty() {
                return Some(cleaned);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use shared::llm::GoogleEmailCandidateSource;

    use super::{apply_email_filters, plan_email_query, window_start_utc};

    fn utc(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("timestamp should parse")
            .with_timezone(&Utc)
    }

    #[test]
    fn plan_email_query_extracts_sender_and_window() {
        let plan = plan_email_query("Any email from finance@example.com today?");
        assert_eq!(plan.sender_filter.as_deref(), Some("finance@example.com"));
        assert_eq!(plan.lookback_days, 1);
        assert_eq!(plan.window_label, "today");

        let weekly_plan = plan_email_query("summarize my inbox this week");
        assert_eq!(weekly_plan.sender_filter, None);
        assert_eq!(weekly_plan.lookback_days, 7);
        assert_eq!(weekly_plan.window_label, "this week");
    }

    #[test]
    fn apply_email_filters_supports_sender_and_user_local_day_window() {
        let now = utc("2026-02-17T12:00:00Z");
        let plan = plan_email_query("Any email from finance@example.com today?");
        let candidates = vec![
            GoogleEmailCandidateSource {
                message_id: Some("1".to_string()),
                from: Some("finance@example.com".to_string()),
                subject: Some("Invoice".to_string()),
                snippet: None,
                received_at: Some(utc("2026-02-17T10:00:00Z")),
                label_ids: vec![],
                has_attachments: false,
            },
            GoogleEmailCandidateSource {
                message_id: Some("2".to_string()),
                from: Some("finance@example.com".to_string()),
                subject: Some("Before local midnight".to_string()),
                snippet: None,
                received_at: Some(utc("2026-02-17T07:30:00Z")),
                label_ids: vec![],
                has_attachments: false,
            },
            GoogleEmailCandidateSource {
                message_id: Some("3".to_string()),
                from: Some("finance@example.com".to_string()),
                subject: Some("Unknown time".to_string()),
                snippet: None,
                received_at: None,
                label_ids: vec![],
                has_attachments: false,
            },
        ];

        let filtered = apply_email_filters(candidates, &plan, now, "America/Los_Angeles");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].message_id.as_deref(), Some("1"));
    }

    #[test]
    fn window_start_utc_aligns_this_week_to_local_monday() {
        let now = utc("2026-02-19T05:00:00Z");
        let plan = plan_email_query("summarize my inbox this week");

        let start = window_start_utc(&plan, now, "America/New_York").expect("start should resolve");
        assert_eq!(start.to_rfc3339(), "2026-02-16T05:00:00+00:00");
    }
}
