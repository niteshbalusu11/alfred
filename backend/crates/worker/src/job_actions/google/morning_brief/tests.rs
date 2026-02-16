use shared::llm::contracts::MorningBriefOutput;

use super::{
    MORNING_BRIEF_BODY_MAX_CHARS, MORNING_BRIEF_TITLE_MAX_CHARS, build_notification_body,
    notification_from_output,
};

#[test]
fn notification_uses_structured_morning_brief_content() {
    let output = MorningBriefOutput {
        headline: "Tuesday plan".to_string(),
        summary: "You have 2 meetings and 1 urgent email candidate.".to_string(),
        priorities: vec!["Confirm demo scope before 10 AM".to_string()],
        schedule: vec!["09:00 - Team standup".to_string()],
        alerts: vec!["Invoice reminder from Finance".to_string()],
    };

    let notification = notification_from_output(&output);
    assert_eq!(notification.title, "Tuesday plan");
    assert!(
        notification
            .body
            .contains("Priority: Confirm demo scope before 10 AM")
    );
    assert!(notification.body.contains("Schedule: 09:00 - Team standup"));
    assert!(
        notification
            .body
            .contains("Alert: Invoice reminder from Finance")
    );
}

#[test]
fn notification_falls_back_when_output_fields_are_empty() {
    let output = MorningBriefOutput {
        headline: "   ".to_string(),
        summary: "   ".to_string(),
        priorities: vec!["".to_string()],
        schedule: vec!["".to_string()],
        alerts: vec![],
    };

    let notification = notification_from_output(&output);
    assert_eq!(notification.title, "Morning brief");
    assert_eq!(
        notification.body,
        "Review your calendar and inbox for today."
    );
}

#[test]
fn notification_body_and_title_are_bounded() {
    let output = MorningBriefOutput {
        headline: "X".repeat(MORNING_BRIEF_TITLE_MAX_CHARS + 20),
        summary: "Y".repeat(MORNING_BRIEF_BODY_MAX_CHARS + 80),
        priorities: Vec::new(),
        schedule: Vec::new(),
        alerts: Vec::new(),
    };

    let notification = notification_from_output(&output);
    assert!(notification.title.ends_with("..."));
    assert!(notification.body.ends_with("..."));
    assert!(notification.title.chars().count() <= MORNING_BRIEF_TITLE_MAX_CHARS + 3);
    assert!(notification.body.chars().count() <= MORNING_BRIEF_BODY_MAX_CHARS + 3);
    assert!(notification.body.chars().count() < build_notification_body(&output).chars().count());
}
