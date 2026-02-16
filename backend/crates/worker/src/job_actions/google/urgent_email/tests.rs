use shared::llm::contracts::{UrgencyLevel, UrgentEmailSummaryOutput};

use super::{
    URGENT_EMAIL_BODY_MAX_CHARS, URGENT_EMAIL_TITLE_MAX_CHARS, build_notification_body,
    notification_from_output,
};

#[test]
fn notification_uses_summary_and_first_action() {
    let output = UrgentEmailSummaryOutput {
        should_notify: true,
        urgency: UrgencyLevel::High,
        summary: "Invoice approval is blocked by missing sign-off.".to_string(),
        reason: "finance escalation".to_string(),
        suggested_actions: vec![
            "Review the request from Finance now.".to_string(),
            "Confirm payment terms with Procurement.".to_string(),
        ],
    };

    let notification = notification_from_output(&output);
    assert_eq!(notification.title, "Urgent email alert");
    assert!(
        notification
            .body
            .contains("Invoice approval is blocked by missing sign-off.")
    );
    assert!(
        notification
            .body
            .contains("Action: Review the request from Finance now.")
    );
}

#[test]
fn notification_falls_back_when_fields_are_empty() {
    let output = UrgentEmailSummaryOutput {
        should_notify: true,
        urgency: UrgencyLevel::Critical,
        summary: "   ".to_string(),
        reason: "   ".to_string(),
        suggested_actions: vec![" ".to_string()],
    };

    let notification = notification_from_output(&output);
    assert_eq!(notification.title, "Critical email alert");
    assert_eq!(notification.body, "Urgent email needs attention.");
}

#[test]
fn notification_title_and_body_are_bounded() {
    let output = UrgentEmailSummaryOutput {
        should_notify: true,
        urgency: UrgencyLevel::Critical,
        summary: "S".repeat(URGENT_EMAIL_BODY_MAX_CHARS + 100),
        reason: "r".to_string(),
        suggested_actions: vec!["A".repeat(URGENT_EMAIL_BODY_MAX_CHARS + 80)],
    };

    let notification = notification_from_output(&output);
    assert!(notification.body.ends_with("..."));
    assert!(notification.title.chars().count() <= URGENT_EMAIL_TITLE_MAX_CHARS + 3);
    assert!(notification.body.chars().count() <= URGENT_EMAIL_BODY_MAX_CHARS + 3);
    assert!(notification.body.chars().count() < build_notification_body(&output).chars().count());
}
