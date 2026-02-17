use std::cmp::Ordering;

use chrono::{DateTime, Duration, Utc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EmailQueryPlan {
    pub(super) sender_filter: Option<String>,
    pub(super) lookback_days: i64,
    pub(super) window_label: &'static str,
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

    let (lookback_days, window_label) = if normalized.contains("today") {
        (1, "today")
    } else if normalized.contains("this week")
        || normalized.contains("next 7 days")
        || normalized.contains("last 7 days")
        || normalized.contains("week")
    {
        (7, "the past 7 days")
    } else if normalized.contains("month") || normalized.contains("30 days") {
        (30, "the past 30 days")
    } else {
        (7, "the past 7 days")
    };

    EmailQueryPlan {
        sender_filter: parse_sender_filter(query),
        lookback_days,
        window_label,
    }
}

pub(super) fn apply_email_filters(
    mut candidates: Vec<shared::llm::GoogleEmailCandidateSource>,
    plan: &EmailQueryPlan,
    now: DateTime<Utc>,
) -> Vec<shared::llm::GoogleEmailCandidateSource> {
    let min_received_at = now - Duration::days(plan.lookback_days);

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
            .map(|received| received >= min_received_at)
            .unwrap_or(true);

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

    use super::{apply_email_filters, plan_email_query};

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
    }

    #[test]
    fn apply_email_filters_supports_sender_and_time_window() {
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
                from: Some("alerts@example.com".to_string()),
                subject: Some("Status".to_string()),
                snippet: None,
                received_at: Some(utc("2026-02-17T09:00:00Z")),
                label_ids: vec![],
                has_attachments: false,
            },
            GoogleEmailCandidateSource {
                message_id: Some("3".to_string()),
                from: Some("finance@example.com".to_string()),
                subject: Some("Old thread".to_string()),
                snippet: None,
                received_at: Some(utc("2026-02-15T10:00:00Z")),
                label_ids: vec![],
                has_attachments: false,
            },
        ];

        let filtered = apply_email_filters(candidates, &plan, now);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].message_id.as_deref(), Some("1"));
    }
}
