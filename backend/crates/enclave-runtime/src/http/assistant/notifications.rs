use shared::llm::contracts::{MorningBriefOutput, UrgencyLevel, UrgentEmailSummaryOutput};

const MORNING_BRIEF_TITLE_MAX_CHARS: usize = 64;
const MORNING_BRIEF_BODY_MAX_CHARS: usize = 180;
const URGENT_EMAIL_TITLE_MAX_CHARS: usize = 64;
const URGENT_EMAIL_BODY_MAX_CHARS: usize = 180;

#[derive(Debug, Clone)]
pub(super) struct NotificationContent {
    pub(super) title: String,
    pub(super) body: String,
}

pub(super) fn notification_from_morning_brief(output: &MorningBriefOutput) -> NotificationContent {
    let title = if output.headline.trim().is_empty() {
        "Morning brief".to_string()
    } else {
        truncate_for_notification(&output.headline, MORNING_BRIEF_TITLE_MAX_CHARS)
    };

    let body = truncate_for_notification(
        &build_morning_brief_notification_body(output),
        MORNING_BRIEF_BODY_MAX_CHARS,
    );

    NotificationContent { title, body }
}

pub(super) fn notification_from_urgent_email(
    output: &UrgentEmailSummaryOutput,
) -> NotificationContent {
    let title = truncate_for_notification(
        notification_title_for_urgency(&output.urgency),
        URGENT_EMAIL_TITLE_MAX_CHARS,
    );
    let body = truncate_for_notification(
        &build_urgent_email_notification_body(output),
        URGENT_EMAIL_BODY_MAX_CHARS,
    );

    NotificationContent { title, body }
}

pub(super) fn urgency_label(urgency: &UrgencyLevel) -> &'static str {
    match urgency {
        UrgencyLevel::Low => "low",
        UrgencyLevel::Medium => "medium",
        UrgencyLevel::High => "high",
        UrgencyLevel::Critical => "critical",
    }
}

fn build_morning_brief_notification_body(output: &MorningBriefOutput) -> String {
    let mut segments = Vec::new();

    if let Some(summary) = non_empty(&output.summary) {
        segments.push(summary.to_string());
    }
    if let Some(priority) = first_non_empty(&output.priorities) {
        segments.push(format!("Priority: {priority}"));
    }
    if let Some(schedule) = first_non_empty(&output.schedule) {
        segments.push(format!("Schedule: {schedule}"));
    }
    if let Some(alert) = first_non_empty(&output.alerts) {
        segments.push(format!("Alert: {alert}"));
    }

    if segments.is_empty() {
        return "Review your calendar and inbox for today.".to_string();
    }

    segments.join(" • ")
}

fn build_urgent_email_notification_body(output: &UrgentEmailSummaryOutput) -> String {
    let mut segments = Vec::new();

    segments.push(
        non_empty(&output.summary)
            .unwrap_or("Urgent email needs attention.")
            .to_string(),
    );
    if let Some(action) = first_non_empty(&output.suggested_actions) {
        segments.push(format!("Action: {action}"));
    }

    segments.join(" • ")
}

fn notification_title_for_urgency(urgency: &UrgencyLevel) -> &'static str {
    match urgency {
        UrgencyLevel::Critical => "Critical email alert",
        UrgencyLevel::High => "Urgent email alert",
        UrgencyLevel::Medium | UrgencyLevel::Low => "Email alert",
    }
}

fn truncate_for_notification(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }

    let truncated = trimmed
        .chars()
        .take(max_chars)
        .collect::<String>()
        .trim_end()
        .to_string();

    format!("{truncated}...")
}

fn first_non_empty(values: &[String]) -> Option<&str> {
    values.iter().find_map(|value| non_empty(value.as_str()))
}

pub(super) fn non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}
