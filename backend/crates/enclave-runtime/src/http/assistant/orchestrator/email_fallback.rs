use shared::models::AssistantStructuredPayload;

use super::super::notifications::non_empty;
use super::email_plan::EmailQueryPlan;

const MAX_FALLBACK_KEY_POINTS: usize = 3;

pub(super) fn deterministic_email_fallback_payload(
    plan: &EmailQueryPlan,
    candidates: &[shared::llm::GoogleEmailCandidateSource],
) -> AssistantStructuredPayload {
    if candidates.is_empty() {
        let summary = if let Some(sender_filter) = &plan.sender_filter {
            format!(
                "No emails from {sender_filter} were found for {}.",
                plan.window_label
            )
        } else {
            format!("No emails were found for {}.", plan.window_label)
        };

        return AssistantStructuredPayload {
            title: "No matching emails".to_string(),
            summary,
            key_points: Vec::new(),
            follow_ups: vec!["Try a broader timeframe or remove sender filters.".to_string()],
        };
    }

    let count = candidates.len();
    AssistantStructuredPayload {
        title: title_for_email_results(plan),
        summary: format!(
            "Found {count} email{} in {}.",
            if count == 1 { "" } else { "s" },
            plan.window_label
        ),
        key_points: candidates
            .iter()
            .take(MAX_FALLBACK_KEY_POINTS)
            .map(format_email_key_point)
            .collect(),
        follow_ups: vec!["Ask for details from a specific sender or subject.".to_string()],
    }
}

pub(super) fn title_for_email_results(plan: &EmailQueryPlan) -> String {
    if let Some(sender_filter) = &plan.sender_filter {
        return format!("Emails from {sender_filter}");
    }

    "Inbox summary".to_string()
}

pub(super) fn format_email_key_point(
    candidate: &shared::llm::GoogleEmailCandidateSource,
) -> String {
    let from = non_empty(candidate.from.as_deref().unwrap_or("")).unwrap_or("unknown sender");
    let subject = non_empty(candidate.subject.as_deref().unwrap_or("")).unwrap_or("(no subject)");
    let received = candidate
        .received_at
        .map(|value| value.format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_else(|| "time unknown".to_string());

    format!("{received}: {from} - {subject}")
}

#[cfg(test)]
mod tests {
    use super::super::email_plan::plan_email_query;
    use super::deterministic_email_fallback_payload;

    #[test]
    fn deterministic_fallback_reports_no_match_queries() {
        let plan = plan_email_query("Any email from legal@example.com today?");
        let payload = deterministic_email_fallback_payload(&plan, &[]);

        assert_eq!(payload.title, "No matching emails");
        assert!(payload.summary.contains("legal@example.com"));
        assert!(payload.summary.contains("today"));
    }
}
