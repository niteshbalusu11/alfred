use shared::assistant_semantic_plan::AssistantSemanticPlan;
use shared::models::AssistantQueryCapability;

pub(super) const MIN_CONFIDENCE_FOR_DIRECT_EXECUTION: f32 = 0.45;
const DEFAULT_CLARIFICATION_QUESTION: &str =
    "Could you clarify whether you want calendar details, email details, or both?";

pub(super) enum PlannedRoute {
    Execute(AssistantQueryCapability),
    Clarify(String),
}

pub(super) fn resolve_route_policy(
    resolution: &super::planner::SemanticPlanResolution,
) -> PlannedRoute {
    let capability = resolution
        .plan
        .capabilities
        .first()
        .cloned()
        .unwrap_or(AssistantQueryCapability::GeneralChat);

    if should_clarify(
        &resolution.plan,
        resolution.used_deterministic_fallback,
        &capability,
    ) {
        return PlannedRoute::Clarify(clarification_question(&resolution.plan));
    }

    PlannedRoute::Execute(capability)
}

fn should_clarify(
    plan: &AssistantSemanticPlan,
    used_deterministic_fallback: bool,
    capability: &AssistantQueryCapability,
) -> bool {
    if plan.needs_clarification {
        return true;
    }

    if used_deterministic_fallback {
        return false;
    }

    if *capability == AssistantQueryCapability::GeneralChat {
        return false;
    }

    plan.confidence < MIN_CONFIDENCE_FOR_DIRECT_EXECUTION
}

fn clarification_question(plan: &AssistantSemanticPlan) -> String {
    plan.clarifying_question
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_CLARIFICATION_QUESTION)
        .to_string()
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use shared::assistant_semantic_plan::AssistantSemanticPlan;

    use super::{MIN_CONFIDENCE_FOR_DIRECT_EXECUTION, PlannedRoute, resolve_route_policy};
    use crate::http::assistant::orchestrator::planner::SemanticPlanResolution;
    use shared::models::AssistantQueryCapability;

    fn utc(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("timestamp should parse")
            .with_timezone(&Utc)
    }

    fn resolution(
        capability: AssistantQueryCapability,
        confidence: f32,
        needs_clarification: bool,
        used_fallback: bool,
    ) -> SemanticPlanResolution {
        SemanticPlanResolution {
            plan: AssistantSemanticPlan {
                capabilities: vec![capability],
                confidence,
                needs_clarification,
                clarifying_question: Some("can you clarify?".to_string()),
                time_window: None,
                email_filters: None,
                language: Some("en".to_string()),
                planned_at: utc("2026-02-18T00:00:00Z"),
            },
            used_deterministic_fallback: used_fallback,
        }
    }

    #[test]
    fn high_confidence_calendar_executes_calendar_lane() {
        let planned = resolve_route_policy(&resolution(
            AssistantQueryCapability::CalendarLookup,
            0.9,
            false,
            false,
        ));
        assert!(matches!(
            planned,
            PlannedRoute::Execute(AssistantQueryCapability::CalendarLookup)
        ));
    }

    #[test]
    fn high_confidence_mixed_executes_mixed_lane() {
        let planned = resolve_route_policy(&resolution(
            AssistantQueryCapability::Mixed,
            0.9,
            false,
            false,
        ));
        assert!(matches!(
            planned,
            PlannedRoute::Execute(AssistantQueryCapability::Mixed)
        ));
    }

    #[test]
    fn resolves_to_clarification_when_plan_requests_it() {
        let planned = resolve_route_policy(&resolution(
            AssistantQueryCapability::CalendarLookup,
            0.9,
            true,
            false,
        ));
        assert!(matches!(planned, PlannedRoute::Clarify(_)));
    }

    #[test]
    fn low_confidence_non_chat_routes_to_clarification() {
        let planned = resolve_route_policy(&resolution(
            AssistantQueryCapability::EmailLookup,
            MIN_CONFIDENCE_FOR_DIRECT_EXECUTION - 0.01,
            false,
            false,
        ));
        assert!(matches!(planned, PlannedRoute::Clarify(_)));
    }

    #[test]
    fn low_confidence_chat_stays_in_chat_lane() {
        let planned = resolve_route_policy(&resolution(
            AssistantQueryCapability::GeneralChat,
            0.1,
            false,
            false,
        ));
        assert!(matches!(
            planned,
            PlannedRoute::Execute(AssistantQueryCapability::GeneralChat)
        ));
    }

    #[test]
    fn deterministic_fallback_executes_without_forcing_clarification() {
        let planned = resolve_route_policy(&resolution(
            AssistantQueryCapability::CalendarLookup,
            0.1,
            false,
            true,
        ));
        assert!(matches!(
            planned,
            PlannedRoute::Execute(AssistantQueryCapability::CalendarLookup)
        ));
    }

    #[test]
    fn clarification_uses_default_question_when_missing() {
        let mut resolution = resolution(AssistantQueryCapability::EmailLookup, 0.9, true, false);
        resolution.plan.clarifying_question = None;
        let planned = resolve_route_policy(&resolution);
        assert!(
            matches!(planned, PlannedRoute::Clarify(question) if question.contains("calendar details"))
        );
    }
}
