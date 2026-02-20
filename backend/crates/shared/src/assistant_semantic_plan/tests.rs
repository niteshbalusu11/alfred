use chrono::{DateTime, Utc};

use super::{
    ASSISTANT_SEMANTIC_PLAN_VERSION_V1, AssistantSemanticCapability,
    AssistantSemanticEmailFiltersOutput, AssistantSemanticPlanContract,
    AssistantSemanticPlanNormalizationError, AssistantSemanticPlanOutput,
    AssistantSemanticTimeWindowOutput, AssistantTimeWindowResolutionSource,
    normalize_semantic_plan_contract,
};
use crate::models::AssistantQueryCapability;

fn utc(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .expect("timestamp should parse")
        .with_timezone(&Utc)
}

#[test]
fn normalize_promotes_calendar_and_email_to_mixed() {
    let plan = normalize_semantic_plan_contract(
        AssistantSemanticPlanContract {
            version: ASSISTANT_SEMANTIC_PLAN_VERSION_V1.to_string(),
            output: AssistantSemanticPlanOutput {
                capabilities: vec![
                    AssistantSemanticCapability::CalendarLookup,
                    AssistantSemanticCapability::EmailLookup,
                ],
                confidence: 0.9,
                needs_clarification: false,
                clarifying_question: None,
                time_window: None,
                email_filters: None,
                language: Some("EN-us".to_string()),
            },
        },
        "America/Los_Angeles",
        utc("2026-02-18T00:00:00Z"),
    )
    .expect("plan should normalize");

    assert_eq!(plan.capabilities, vec![AssistantQueryCapability::Mixed]);
    assert_eq!(plan.language.as_deref(), Some("en-us"));
}

#[test]
fn normalize_clamps_email_filters() {
    let plan = normalize_semantic_plan_contract(
        AssistantSemanticPlanContract {
            version: ASSISTANT_SEMANTIC_PLAN_VERSION_V1.to_string(),
            output: AssistantSemanticPlanOutput {
                capabilities: vec![AssistantSemanticCapability::EmailLookup],
                confidence: 0.8,
                needs_clarification: false,
                clarifying_question: None,
                time_window: None,
                email_filters: Some(AssistantSemanticEmailFiltersOutput {
                    sender: Some(" finance@example.com ".to_string()),
                    keywords: vec![
                        "q1".to_string(),
                        "budget".to_string(),
                        "escalation".to_string(),
                        "risk".to_string(),
                        "follow-up".to_string(),
                        "priority".to_string(),
                        "overflow".to_string(),
                    ],
                    lookback_days: Some(400),
                    unread_only: None,
                }),
                language: None,
            },
        },
        "UTC",
        utc("2026-02-18T00:00:00Z"),
    )
    .expect("plan should normalize");

    let filters = plan.email_filters.expect("email filters should exist");
    assert_eq!(filters.sender.as_deref(), Some("finance@example.com"));
    assert_eq!(filters.lookback_days, 30);
    assert_eq!(filters.keywords.len(), 6);
    assert!(!filters.unread_only);
}

#[test]
fn normalize_rejects_invalid_time_window() {
    let err = normalize_semantic_plan_contract(
        AssistantSemanticPlanContract {
            version: ASSISTANT_SEMANTIC_PLAN_VERSION_V1.to_string(),
            output: AssistantSemanticPlanOutput {
                capabilities: vec![AssistantSemanticCapability::CalendarLookup],
                confidence: 0.6,
                needs_clarification: false,
                clarifying_question: None,
                time_window: Some(AssistantSemanticTimeWindowOutput {
                    start: "2026-02-19T00:00:00Z".to_string(),
                    end: "2026-02-18T00:00:00Z".to_string(),
                    timezone: "UTC".to_string(),
                    resolution_source: AssistantTimeWindowResolutionSource::RelativeDate,
                }),
                email_filters: None,
                language: None,
            },
        },
        "UTC",
        utc("2026-02-18T00:00:00Z"),
    )
    .expect_err("invalid range must fail");

    assert!(matches!(
        err,
        AssistantSemanticPlanNormalizationError::InvalidTimeWindowOrder
    ));
}

#[test]
fn normalize_allows_missing_clarifying_question() {
    let plan = normalize_semantic_plan_contract(
        AssistantSemanticPlanContract {
            version: ASSISTANT_SEMANTIC_PLAN_VERSION_V1.to_string(),
            output: AssistantSemanticPlanOutput {
                capabilities: vec![AssistantSemanticCapability::GeneralChat],
                confidence: 0.2,
                needs_clarification: true,
                clarifying_question: None,
                time_window: None,
                email_filters: None,
                language: None,
            },
        },
        "UTC",
        utc("2026-02-18T00:00:00Z"),
    )
    .expect("clarifying question should be optional");
    assert!(plan.needs_clarification);
    assert!(plan.clarifying_question.is_none());
}
