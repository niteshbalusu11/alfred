use shared::llm::AssistantOutputContract;

use crate::case::QualityExpectations;

pub fn evaluate_quality(
    contract: &AssistantOutputContract,
    expectations: &QualityExpectations,
) -> Vec<String> {
    let mut issues = Vec::new();

    match contract {
        AssistantOutputContract::MeetingsSummary(summary) => {
            require_non_empty_text("output.title", &summary.output.title, &mut issues);
            require_non_empty_text("output.summary", &summary.output.summary, &mut issues);
            require_all_non_empty("output.key_points", &summary.output.key_points, &mut issues);
            require_all_non_empty("output.follow_ups", &summary.output.follow_ups, &mut issues);
            require_min_len(
                "output.key_points",
                summary.output.key_points.len(),
                expectations.min_key_points,
                &mut issues,
            );
            require_min_len(
                "output.follow_ups",
                summary.output.follow_ups.len(),
                expectations.min_follow_ups,
                &mut issues,
            );
        }
        AssistantOutputContract::MorningBrief(brief) => {
            require_non_empty_text("output.headline", &brief.output.headline, &mut issues);
            require_non_empty_text("output.summary", &brief.output.summary, &mut issues);
            require_all_non_empty("output.priorities", &brief.output.priorities, &mut issues);
            require_all_non_empty("output.schedule", &brief.output.schedule, &mut issues);
            require_all_non_empty("output.alerts", &brief.output.alerts, &mut issues);
            require_min_len(
                "output.priorities",
                brief.output.priorities.len(),
                expectations.min_priorities,
                &mut issues,
            );
            require_min_len(
                "output.schedule",
                brief.output.schedule.len(),
                expectations.min_schedule,
                &mut issues,
            );
            require_min_len(
                "output.alerts",
                brief.output.alerts.len(),
                expectations.min_alerts,
                &mut issues,
            );
        }
        AssistantOutputContract::UrgentEmailSummary(urgent) => {
            require_non_empty_text("output.summary", &urgent.output.summary, &mut issues);
            require_non_empty_text("output.reason", &urgent.output.reason, &mut issues);
            require_all_non_empty(
                "output.suggested_actions",
                &urgent.output.suggested_actions,
                &mut issues,
            );
            require_min_len(
                "output.suggested_actions",
                urgent.output.suggested_actions.len(),
                expectations.min_suggested_actions,
                &mut issues,
            );

            if urgent.output.should_notify
                && !matches!(
                    urgent.output.urgency,
                    shared::llm::contracts::UrgencyLevel::High
                        | shared::llm::contracts::UrgencyLevel::Critical
                )
            {
                issues.push(
                    "output.urgency: should_notify=true requires high or critical urgency"
                        .to_string(),
                );
            }
        }
    }

    issues
}

fn require_non_empty_text(field: &str, value: &str, issues: &mut Vec<String>) {
    if value.trim().is_empty() {
        issues.push(format!("{field}: must be non-empty"));
    }
}

fn require_all_non_empty(field: &str, values: &[String], issues: &mut Vec<String>) {
    for (index, value) in values.iter().enumerate() {
        if value.trim().is_empty() {
            issues.push(format!("{field}[{index}]: must be non-empty"));
        }
    }
}

fn require_min_len(
    field: &str,
    actual: usize,
    expected_min: Option<usize>,
    issues: &mut Vec<String>,
) {
    if let Some(expected_min) = expected_min
        && actual < expected_min
    {
        issues.push(format!(
            "{field}: expected at least {expected_min} items, got {actual}"
        ));
    }
}
