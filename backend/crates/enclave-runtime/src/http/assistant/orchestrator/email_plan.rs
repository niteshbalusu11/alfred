use std::cmp::Ordering;

use chrono::{DateTime, Utc};
use shared::assistant_semantic_plan::{AssistantSemanticEmailFilters, AssistantSemanticTimeWindow};

use super::calendar_range::window_label;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EmailQueryPlan {
    pub(super) sender_filter: Option<String>,
    pub(super) keyword_filters: Vec<String>,
    pub(super) unread_only: bool,
    pub(super) window_start_utc: DateTime<Utc>,
    pub(super) window_end_utc: DateTime<Utc>,
    pub(super) window_label: String,
}

pub(super) fn plan_email_query(
    time_window: &AssistantSemanticTimeWindow,
    email_filters: Option<&AssistantSemanticEmailFilters>,
) -> EmailQueryPlan {
    let (sender_filter, keyword_filters, unread_only) = if let Some(filters) = email_filters {
        (
            sanitize_sender(filters.sender.as_deref()),
            filters
                .keywords
                .iter()
                .filter_map(|keyword| sanitize_keyword(keyword))
                .collect::<Vec<_>>(),
            filters.unread_only,
        )
    } else {
        (None, Vec::new(), false)
    };

    EmailQueryPlan {
        sender_filter,
        keyword_filters,
        unread_only,
        window_start_utc: time_window.start,
        window_end_utc: time_window.end,
        window_label: window_label(
            time_window.start,
            time_window.end,
            time_window.timezone.as_str(),
        ),
    }
}

pub(super) fn build_gmail_query(plan: &EmailQueryPlan) -> String {
    let mut parts = vec![
        format!("after:{}", plan.window_start_utc.timestamp()),
        format!("before:{}", plan.window_end_utc.timestamp()),
    ];

    if let Some(sender_filter) = &plan.sender_filter {
        parts.push(format!("from:{sender_filter}"));
    }

    if plan.unread_only {
        parts.push("is:unread".to_string());
    }

    for keyword in &plan.keyword_filters {
        parts.push(format!("\"{keyword}\""));
    }

    parts.join(" ")
}

pub(super) fn apply_email_filters(
    mut candidates: Vec<shared::llm::GoogleEmailCandidateSource>,
    plan: &EmailQueryPlan,
) -> Vec<shared::llm::GoogleEmailCandidateSource> {
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

        let unread_match = if plan.unread_only {
            candidate
                .label_ids
                .iter()
                .any(|label| label.eq_ignore_ascii_case("UNREAD"))
        } else {
            true
        };

        let text = format!(
            "{}\n{}\n{}",
            candidate.from.as_deref().unwrap_or(""),
            candidate.subject.as_deref().unwrap_or(""),
            candidate.snippet.as_deref().unwrap_or("")
        )
        .to_ascii_lowercase();
        let keyword_match = plan
            .keyword_filters
            .iter()
            .all(|keyword| text.contains(keyword));

        let time_match = candidate
            .received_at
            .map(|received| received >= plan.window_start_utc && received < plan.window_end_utc)
            .unwrap_or(false);

        sender_match && unread_match && keyword_match && time_match
    });

    candidates.sort_by(|left, right| match (left.received_at, right.received_at) {
        (Some(left_received), Some(right_received)) => right_received.cmp(&left_received),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    });

    candidates
}

fn sanitize_sender(raw: Option<&str>) -> Option<String> {
    let value = raw?.trim();
    if value.is_empty() {
        return None;
    }

    let normalized = value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '@' | '.' | '_' | '-' | '+' | '*'))
        .collect::<String>()
        .to_ascii_lowercase();

    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn sanitize_keyword(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.is_empty() {
        return None;
    }

    let normalized = value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '@' | '.' | '_' | '-' | '+'))
        .collect::<String>()
        .trim()
        .to_ascii_lowercase();

    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use shared::assistant_semantic_plan::{
        AssistantSemanticEmailFilters, AssistantSemanticTimeWindow,
        AssistantTimeWindowResolutionSource,
    };
    use shared::llm::GoogleEmailCandidateSource;

    use super::{apply_email_filters, build_gmail_query, plan_email_query};

    fn utc(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("timestamp should parse")
            .with_timezone(&Utc)
    }

    fn semantic_window() -> AssistantSemanticTimeWindow {
        AssistantSemanticTimeWindow {
            start: utc("2026-02-17T08:00:00Z"),
            end: utc("2026-02-18T08:00:00Z"),
            timezone: "America/Los_Angeles".to_string(),
            resolution_source: AssistantTimeWindowResolutionSource::RelativeDate,
        }
    }

    #[test]
    fn build_gmail_query_uses_absolute_bounds_and_filters() {
        let filters = AssistantSemanticEmailFilters {
            sender: Some("Finance@Example.com".to_string()),
            keywords: vec!["Quarterly Update".to_string()],
            lookback_days: 9,
            unread_only: true,
        };

        let plan = plan_email_query(&semantic_window(), Some(&filters));
        let query = build_gmail_query(&plan);

        assert!(query.contains("after:1771315200"));
        assert!(query.contains("before:1771401600"));
        assert!(query.contains("from:finance@example.com"));
        assert!(query.contains("is:unread"));
        assert!(query.contains("\"quarterly update\""));
        assert!(!query.contains("newer_than:"));
    }

    #[test]
    fn apply_email_filters_supports_sender_window_unread_and_keywords() {
        let filters = AssistantSemanticEmailFilters {
            sender: Some("finance@example.com".to_string()),
            keywords: vec!["invoice".to_string()],
            lookback_days: 5,
            unread_only: true,
        };
        let plan = plan_email_query(&semantic_window(), Some(&filters));

        let candidates = vec![
            GoogleEmailCandidateSource {
                message_id: Some("1".to_string()),
                from: Some("finance@example.com".to_string()),
                subject: Some("Invoice due".to_string()),
                snippet: None,
                received_at: Some(utc("2026-02-17T10:00:00Z")),
                label_ids: vec!["INBOX".to_string(), "UNREAD".to_string()],
                has_attachments: false,
            },
            GoogleEmailCandidateSource {
                message_id: Some("2".to_string()),
                from: Some("finance@example.com".to_string()),
                subject: Some("Invoice older".to_string()),
                snippet: None,
                received_at: Some(utc("2026-02-17T07:30:00Z")),
                label_ids: vec!["INBOX".to_string(), "UNREAD".to_string()],
                has_attachments: false,
            },
            GoogleEmailCandidateSource {
                message_id: Some("3".to_string()),
                from: Some("finance@example.com".to_string()),
                subject: Some("Read email".to_string()),
                snippet: None,
                received_at: Some(utc("2026-02-17T11:00:00Z")),
                label_ids: vec!["INBOX".to_string()],
                has_attachments: false,
            },
        ];

        let filtered = apply_email_filters(candidates, &plan);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].message_id.as_deref(), Some("1"));
    }
}
