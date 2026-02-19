use std::cmp::Ordering;

use chrono::{DateTime, Duration, Utc};
use shared::assistant_semantic_plan::AssistantSemanticEmailFilters;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EmailQueryPlan {
    pub(super) sender_filter: Option<String>,
    pub(super) keywords: Vec<String>,
    pub(super) lookback_days: i64,
    pub(super) unread_only: bool,
    pub(super) window_label: String,
    window_start: DateTime<Utc>,
}

pub(super) fn plan_email_query(
    filters: &AssistantSemanticEmailFilters,
    now: DateTime<Utc>,
) -> EmailQueryPlan {
    let lookback_days = i64::from(filters.lookback_days.clamp(1, 30));
    let window_label = if lookback_days == 1 {
        "the past day".to_string()
    } else {
        format!("the past {lookback_days} days")
    };

    EmailQueryPlan {
        sender_filter: filters.sender.as_deref().map(str::to_ascii_lowercase),
        keywords: filters
            .keywords
            .iter()
            .map(|keyword| keyword.to_ascii_lowercase())
            .collect(),
        lookback_days,
        unread_only: filters.unread_only,
        window_label,
        window_start: now - Duration::days(lookback_days),
    }
}

pub(super) fn build_gmail_query(plan: &EmailQueryPlan) -> String {
    let mut parts = vec![format!("newer_than:{}d", plan.lookback_days)];
    if let Some(sender_filter) = &plan.sender_filter {
        parts.push(format!("from:{sender_filter}"));
    }
    if plan.unread_only {
        parts.push("is:unread".to_string());
    }
    if !plan.keywords.is_empty() {
        parts.extend(plan.keywords.iter().cloned());
    }

    parts.join(" ")
}

pub(super) fn apply_email_filters(
    mut candidates: Vec<shared::llm::GoogleEmailCandidateSource>,
    plan: &EmailQueryPlan,
    now: DateTime<Utc>,
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

        let keyword_match = if plan.keywords.is_empty() {
            true
        } else {
            let subject = candidate
                .subject
                .as_deref()
                .unwrap_or("")
                .to_ascii_lowercase();
            let snippet = candidate
                .snippet
                .as_deref()
                .unwrap_or("")
                .to_ascii_lowercase();
            plan.keywords
                .iter()
                .any(|keyword| subject.contains(keyword) || snippet.contains(keyword))
        };

        let unread_match = if plan.unread_only {
            candidate
                .label_ids
                .iter()
                .any(|label| label.eq_ignore_ascii_case("UNREAD"))
        } else {
            true
        };

        let time_match = candidate
            .received_at
            .map(|received| received >= plan.window_start && received <= now)
            .unwrap_or(false);

        sender_match && keyword_match && unread_match && time_match
    });

    candidates.sort_by(|left, right| match (left.received_at, right.received_at) {
        (Some(left_received), Some(right_received)) => right_received.cmp(&left_received),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    });

    candidates
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use shared::assistant_semantic_plan::AssistantSemanticEmailFilters;
    use shared::llm::GoogleEmailCandidateSource;

    use super::{apply_email_filters, build_gmail_query, plan_email_query};

    fn utc(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("timestamp should parse")
            .with_timezone(&Utc)
    }

    #[test]
    fn plan_email_query_is_built_from_structured_filters() {
        let now = utc("2026-02-17T12:00:00Z");
        let plan = plan_email_query(
            &AssistantSemanticEmailFilters {
                sender: Some("Finance@Example.com".to_string()),
                keywords: vec!["invoice".to_string(), "q1".to_string()],
                lookback_days: 7,
                unread_only: true,
            },
            now,
        );

        assert_eq!(plan.sender_filter.as_deref(), Some("finance@example.com"));
        assert_eq!(plan.lookback_days, 7);
        assert_eq!(plan.window_label, "the past 7 days");
        assert!(plan.unread_only);

        let gmail_query = build_gmail_query(&plan);
        assert!(gmail_query.contains("newer_than:7d"));
        assert!(gmail_query.contains("from:finance@example.com"));
        assert!(gmail_query.contains("is:unread"));
        assert!(gmail_query.contains("invoice"));
    }

    #[test]
    fn apply_email_filters_uses_structured_sender_keyword_and_unread_constraints() {
        let now = utc("2026-02-17T12:00:00Z");
        let plan = plan_email_query(
            &AssistantSemanticEmailFilters {
                sender: Some("finance@example.com".to_string()),
                keywords: vec!["invoice".to_string()],
                lookback_days: 7,
                unread_only: true,
            },
            now,
        );

        let candidates = vec![
            GoogleEmailCandidateSource {
                message_id: Some("1".to_string()),
                from: Some("finance@example.com".to_string()),
                subject: Some("Invoice for Q1".to_string()),
                snippet: Some("Please review attached invoice".to_string()),
                received_at: Some(utc("2026-02-16T10:00:00Z")),
                label_ids: vec!["UNREAD".to_string()],
                has_attachments: false,
            },
            GoogleEmailCandidateSource {
                message_id: Some("2".to_string()),
                from: Some("finance@example.com".to_string()),
                subject: Some("Budget update".to_string()),
                snippet: Some("No invoice mentioned".to_string()),
                received_at: Some(utc("2026-02-16T09:00:00Z")),
                label_ids: vec!["INBOX".to_string()],
                has_attachments: false,
            },
        ];

        let filtered = apply_email_filters(candidates, &plan, now);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].message_id.as_deref(), Some("1"));
    }
}
