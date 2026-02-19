use shared::assistant_semantic_plan::AssistantSemanticPlan;
use shared::models::AssistantQueryCapability;

pub(super) const MIN_CONFIDENCE_FOR_DIRECT_EXECUTION: f32 = 0.45;
const DEFAULT_CLARIFICATION_QUESTION: &str =
    "Could you restate your request with the exact scope you want me to use?";
const DEFAULT_ENGLISH_ONLY_QUESTION: &str =
    "English support is currently available. Could you rephrase your request in English?";
const MISSING_TIME_WINDOW_QUESTION: &str =
    "Could you clarify the exact calendar timeframe you want me to use?";
const MISSING_EMAIL_FILTERS_QUESTION: &str =
    "Could you clarify the email scope (time window, sender, or keywords) you want me to use?";
const MISSING_MIXED_SCOPE_QUESTION: &str =
    "Could you clarify both the calendar timeframe and email scope you want me to use?";

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

    if let Some(question) =
        unsupported_language_clarification(&resolution.plan, resolution.used_deterministic_fallback)
    {
        return PlannedRoute::Clarify(question);
    }

    if let Some(question) = missing_structured_scope_clarification(&resolution.plan, &capability) {
        return PlannedRoute::Clarify(question);
    }

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

fn unsupported_language_clarification(
    plan: &AssistantSemanticPlan,
    used_deterministic_fallback: bool,
) -> Option<String> {
    if used_deterministic_fallback {
        return None;
    }

    let language = plan.language.as_deref()?;
    if language_is_english(language) {
        return None;
    }

    Some(DEFAULT_ENGLISH_ONLY_QUESTION.to_string())
}

fn missing_structured_scope_clarification(
    plan: &AssistantSemanticPlan,
    capability: &AssistantQueryCapability,
) -> Option<String> {
    match capability {
        AssistantQueryCapability::MeetingsToday | AssistantQueryCapability::CalendarLookup => {
            if plan.time_window.is_none() {
                return Some(MISSING_TIME_WINDOW_QUESTION.to_string());
            }
        }
        AssistantQueryCapability::EmailLookup => {
            if plan.email_filters.is_none() {
                return Some(MISSING_EMAIL_FILTERS_QUESTION.to_string());
            }
        }
        AssistantQueryCapability::Mixed => {
            if plan.time_window.is_none() || plan.email_filters.is_none() {
                return Some(MISSING_MIXED_SCOPE_QUESTION.to_string());
            }
        }
        AssistantQueryCapability::GeneralChat => {}
    }

    None
}

fn language_is_english(language: &str) -> bool {
    let normalized = language.trim().to_ascii_lowercase();
    normalized == "en" || normalized.starts_with("en-")
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
    use shared::assistant_semantic_plan::AssistantSemanticEmailFilters;
    use shared::assistant_semantic_plan::AssistantSemanticPlan;
    use shared::assistant_semantic_plan::AssistantSemanticTimeWindow;
    use shared::assistant_semantic_plan::AssistantTimeWindowResolutionSource;

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
        let default_time_window = AssistantSemanticTimeWindow {
            start: utc("2026-02-18T00:00:00Z"),
            end: utc("2026-02-25T00:00:00Z"),
            timezone: "UTC".to_string(),
            resolution_source: AssistantTimeWindowResolutionSource::RelativeDate,
        };
        let default_email_filters = AssistantSemanticEmailFilters {
            sender: Some("finance@example.com".to_string()),
            keywords: vec!["invoice".to_string()],
            lookback_days: 7,
            unread_only: false,
        };
        let (time_window, email_filters) = match capability {
            AssistantQueryCapability::MeetingsToday | AssistantQueryCapability::CalendarLookup => {
                (Some(default_time_window), None)
            }
            AssistantQueryCapability::EmailLookup => (None, Some(default_email_filters)),
            AssistantQueryCapability::Mixed => {
                (Some(default_time_window), Some(default_email_filters))
            }
            AssistantQueryCapability::GeneralChat => (None, None),
        };

        SemanticPlanResolution {
            plan: AssistantSemanticPlan {
                capabilities: vec![capability],
                confidence,
                needs_clarification,
                clarifying_question: Some("can you clarify?".to_string()),
                time_window,
                email_filters,
                language: Some("en".to_string()),
                planned_at: utc("2026-02-18T00:00:00Z"),
            },
            model_response_payload: None,
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
    fn missing_calendar_time_window_routes_to_clarification() {
        let mut resolution =
            resolution(AssistantQueryCapability::CalendarLookup, 0.95, false, false);
        resolution.plan.time_window = None;
        let planned = resolve_route_policy(&resolution);
        assert!(matches!(
            planned,
            PlannedRoute::Clarify(question) if question.contains("calendar timeframe")
        ));
    }

    #[test]
    fn missing_email_filters_routes_to_clarification() {
        let mut resolution = resolution(AssistantQueryCapability::EmailLookup, 0.95, false, false);
        resolution.plan.email_filters = None;
        let planned = resolve_route_policy(&resolution);
        assert!(matches!(
            planned,
            PlannedRoute::Clarify(question) if question.contains("email scope")
        ));
    }

    #[test]
    fn structured_inputs_allow_high_confidence_tool_execution() {
        let mut enriched = resolution(AssistantQueryCapability::Mixed, 0.95, false, false);
        enriched.plan.time_window = Some(AssistantSemanticTimeWindow {
            start: utc("2026-02-18T00:00:00Z"),
            end: utc("2026-02-25T00:00:00Z"),
            timezone: "UTC".to_string(),
            resolution_source: AssistantTimeWindowResolutionSource::RelativeDate,
        });
        enriched.plan.email_filters = Some(AssistantSemanticEmailFilters {
            sender: Some("finance@example.com".to_string()),
            keywords: vec!["invoice".to_string()],
            lookback_days: 7,
            unread_only: false,
        });

        let planned = resolve_route_policy(&enriched);
        assert!(matches!(
            planned,
            PlannedRoute::Execute(AssistantQueryCapability::Mixed)
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
            matches!(planned, PlannedRoute::Clarify(question) if question.contains("exact scope"))
        );
    }

    #[test]
    fn non_english_language_hint_routes_to_clarification() {
        let mut resolution =
            resolution(AssistantQueryCapability::CalendarLookup, 0.95, false, false);
        resolution.plan.language = Some("es".to_string());
        let planned = resolve_route_policy(&resolution);
        assert!(
            matches!(planned, PlannedRoute::Clarify(question) if question.contains("rephrase your request in English"))
        );
    }

    #[test]
    fn english_language_variants_do_not_force_clarification() {
        let mut resolution = resolution(AssistantQueryCapability::EmailLookup, 0.95, false, false);
        resolution.plan.language = Some("en-US".to_string());
        let planned = resolve_route_policy(&resolution);
        assert!(matches!(
            planned,
            PlannedRoute::Execute(AssistantQueryCapability::EmailLookup)
        ));
    }

    #[test]
    fn deterministic_fallback_does_not_force_non_english_clarification() {
        let mut resolution = resolution(AssistantQueryCapability::CalendarLookup, 0.2, false, true);
        resolution.plan.language = Some("es".to_string());
        let planned = resolve_route_policy(&resolution);
        assert!(matches!(
            planned,
            PlannedRoute::Execute(AssistantQueryCapability::CalendarLookup)
        ));
    }
}
