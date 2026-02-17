use shared::models::{
    AssistantQueryCapability, AssistantResponsePartType, AssistantStructuredPayload,
};

use super::{
    combine_follow_ups, compose_full_mixed_payload, compose_full_response_parts,
    compose_partial_response_parts,
};

#[test]
fn compose_full_mixed_payload_prefixes_calendar_and_email_points() {
    let calendar = AssistantStructuredPayload {
        title: "Calendar".to_string(),
        summary: "Calendar summary".to_string(),
        key_points: vec!["10:00 Team sync".to_string()],
        follow_ups: vec!["Ask for tomorrow.".to_string()],
    };
    let email = AssistantStructuredPayload {
        title: "Email".to_string(),
        summary: "Email summary".to_string(),
        key_points: vec!["finance@example.com - Invoice".to_string()],
        follow_ups: vec!["Filter by sender.".to_string()],
    };

    let payload = compose_full_mixed_payload(
        "what do i need from calendar and email today?",
        &calendar,
        &email,
    );
    assert_eq!(payload.title, "Calendar and inbox summary");
    assert!(
        payload
            .summary
            .contains("combined summary from your calendar and inbox"),
        "mixed summary should include combined explanation"
    );
    assert_eq!(
        payload.key_points,
        vec![
            "Calendar: 10:00 Team sync".to_string(),
            "Email: finance@example.com - Invoice".to_string(),
        ]
    );
    assert_eq!(
        payload.follow_ups,
        vec![
            "Ask for tomorrow.".to_string(),
            "Filter by sender.".to_string()
        ]
    );
}

#[test]
fn combine_follow_ups_deduplicates_and_limits_results() {
    let follow_ups = combine_follow_ups(
        &[
            "Ask for tomorrow.".to_string(),
            "Filter by sender.".to_string(),
            "Filter by sender.".to_string(),
        ],
        &[
            "Ask for tomorrow.".to_string(),
            "Show this week.".to_string(),
            "Show next week.".to_string(),
            "Extra item".to_string(),
        ],
    );

    assert_eq!(
        follow_ups,
        vec![
            "Ask for tomorrow.".to_string(),
            "Filter by sender.".to_string(),
            "Show this week.".to_string(),
            "Show next week.".to_string(),
        ]
    );
}

#[test]
fn compose_full_response_parts_emits_chat_and_two_tool_summaries() {
    let calendar = AssistantStructuredPayload {
        title: "Calendar".to_string(),
        summary: "Calendar summary".to_string(),
        key_points: vec![],
        follow_ups: vec![],
    };
    let email = AssistantStructuredPayload {
        title: "Email".to_string(),
        summary: "Email summary".to_string(),
        key_points: vec![],
        follow_ups: vec![],
    };

    let parts = compose_full_response_parts(
        "Combined summary".to_string(),
        &AssistantQueryCapability::CalendarLookup,
        &calendar,
        &AssistantQueryCapability::EmailLookup,
        &email,
    );

    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0].part_type, AssistantResponsePartType::ChatText);
    assert_eq!(
        parts[1].capability,
        Some(AssistantQueryCapability::CalendarLookup)
    );
    assert_eq!(
        parts[2].capability,
        Some(AssistantQueryCapability::EmailLookup)
    );
}

#[test]
fn compose_partial_response_parts_emits_single_tool_summary() {
    let payload = AssistantStructuredPayload {
        title: "Calendar".to_string(),
        summary: "Calendar summary".to_string(),
        key_points: vec![],
        follow_ups: vec![],
    };

    let parts = compose_partial_response_parts(
        "Partial summary".to_string(),
        &AssistantQueryCapability::CalendarLookup,
        &payload,
    );

    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].part_type, AssistantResponsePartType::ChatText);
    assert_eq!(
        parts[1].capability,
        Some(AssistantQueryCapability::CalendarLookup)
    );
}
