use chrono::{Days, Utc};
use serde_json::{Value, json};
use shared::assistant_semantic_plan::{
    AssistantSemanticCapability, AssistantSemanticPlan, AssistantSemanticPlanOutput,
    AssistantSemanticTimeWindow, AssistantTimeWindowResolutionSource,
    normalize_semantic_plan_output,
};
use shared::llm::{
    AssistantCapability, AssistantOutputContract, LlmExecutionSource, LlmGatewayError,
    LlmGatewayRequest, generate_with_telemetry, sanitize_context_payload, template_for_capability,
    validate_output_value,
};
use shared::models::AssistantQueryCapability;
use tracing::{info, warn};
use uuid::Uuid;

use super::super::memory::{
    detect_query_capability, query_context_snippet, resolve_query_capability,
    session_memory_context,
};
use super::super::session_state::EnclaveAssistantSessionState;
use crate::RuntimeState;
use shared::timezone::{local_day_bounds_utc, parse_time_zone_or_default};

pub(super) struct SemanticPlanResolution {
    pub(super) plan: AssistantSemanticPlan,
    pub(super) used_deterministic_fallback: bool,
}

pub(super) async fn resolve_semantic_plan(
    state: &RuntimeState,
    user_id: Uuid,
    request_id: &str,
    query: &str,
    user_time_zone: &str,
    prior_state: Option<&EnclaveAssistantSessionState>,
) -> SemanticPlanResolution {
    let now_utc = Utc::now();
    let now_local = now_utc
        .with_timezone(&parse_time_zone_or_default(user_time_zone))
        .to_rfc3339();
    let mut context_payload = json!({
        "query_context": query_context_snippet(query),
        "user_time_zone": user_time_zone,
        "current_time_utc": now_utc.to_rfc3339(),
        "current_time_local": now_local,
    });
    if let Value::Object(entries) = &mut context_payload {
        if let Some(memory_context) = session_memory_context(prior_state.map(|state| &state.memory))
        {
            entries.insert("session_memory".to_string(), memory_context);
        }
        if let Some(prior_capability) = prior_state.map(|state| state.last_capability.clone()) {
            entries.insert(
                "prior_capability".to_string(),
                json!(capability_label(prior_capability)),
            );
        }
    }

    let context_payload = sanitize_context_payload(&context_payload);
    let llm_request = LlmGatewayRequest::from_template(
        template_for_capability(AssistantCapability::AssistantSemanticPlan),
        context_payload,
    )
    .with_requester_id(user_id.to_string());

    let (llm_result, telemetry) = generate_with_telemetry(
        state.assistant_planner_gateway(),
        LlmExecutionSource::ApiAssistantQuery,
        llm_request,
    )
    .await;
    super::super::mapping::log_telemetry(user_id, &telemetry, "assistant_semantic_planner");
    info!(
        user_id = %user_id,
        request_id,
        planner_llm_latency_ms = telemetry.latency_ms,
        planner_llm_outcome = telemetry.outcome,
        planner_llm_model = ?telemetry.model,
        "assistant semantic planner llm stage"
    );

    match llm_result {
        Ok(response) => match parse_semantic_plan_output(&response.output, user_time_zone) {
            Ok(plan) => {
                let plan = with_default_time_window_if_missing(query, user_time_zone, plan);
                info!(
                    user_id = %user_id,
                    request_id,
                    confidence = plan.confidence,
                    needs_clarification = plan.needs_clarification,
                    "assistant semantic planner resolved model output"
                );
                SemanticPlanResolution {
                    plan,
                    used_deterministic_fallback: false,
                }
            }
            Err(err) => {
                warn!(
                    user_id = %user_id,
                    request_id,
                    "assistant semantic planner output was invalid, falling back deterministically: {err}"
                );
                SemanticPlanResolution {
                    plan: deterministic_fallback_plan(query, user_time_zone, prior_state),
                    used_deterministic_fallback: true,
                }
            }
        },
        Err(err) => {
            warn!(
                user_id = %user_id,
                request_id,
                "assistant semantic planner request failed, falling back deterministically: {err}"
            );
            SemanticPlanResolution {
                plan: deterministic_fallback_plan(query, user_time_zone, prior_state),
                used_deterministic_fallback: true,
            }
        }
    }
}

fn parse_semantic_plan_output(
    payload: &Value,
    user_time_zone: &str,
) -> Result<AssistantSemanticPlan, LlmGatewayError> {
    let contract = validate_output_value(AssistantCapability::AssistantSemanticPlan, payload)
        .map_err(|err| LlmGatewayError::InvalidProviderPayload(err.to_string()))?;
    let AssistantOutputContract::AssistantSemanticPlan(contract) = contract else {
        return Err(LlmGatewayError::InvalidProviderPayload(
            "semantic planner contract type mismatch".to_string(),
        ));
    };

    normalize_semantic_plan_output(contract.output, user_time_zone, Utc::now())
        .map_err(|err| LlmGatewayError::InvalidProviderPayload(err.to_string()))
}

fn deterministic_fallback_plan(
    query: &str,
    user_time_zone: &str,
    prior_state: Option<&EnclaveAssistantSessionState>,
) -> AssistantSemanticPlan {
    let detected_capability = detect_query_capability(query);
    let resolved = resolve_query_capability(
        query,
        detected_capability,
        prior_state.map(|state| state.last_capability.clone()),
    )
    .unwrap_or(AssistantQueryCapability::GeneralChat);

    let output = AssistantSemanticPlanOutput {
        capabilities: vec![map_to_semantic_capability(resolved)],
        confidence: 0.25,
        needs_clarification: false,
        clarifying_question: None,
        time_window: None,
        email_filters: None,
        language: None,
    };

    let plan = normalize_semantic_plan_output(output, user_time_zone, Utc::now())
        .expect("deterministic semantic planner fallback must normalize");
    with_default_time_window_if_missing(query, user_time_zone, plan)
}

fn with_default_time_window_if_missing(
    query: &str,
    user_time_zone: &str,
    mut plan: AssistantSemanticPlan,
) -> AssistantSemanticPlan {
    if plan.time_window.is_some() {
        return plan;
    }

    let capability = plan
        .capabilities
        .first()
        .cloned()
        .unwrap_or(AssistantQueryCapability::GeneralChat);
    if !requires_time_window(&capability) {
        return plan;
    }

    plan.time_window = derive_semantic_time_window(query, user_time_zone, capability, Utc::now());
    plan
}

fn requires_time_window(capability: &AssistantQueryCapability) -> bool {
    matches!(
        capability,
        &AssistantQueryCapability::MeetingsToday
            | &AssistantQueryCapability::CalendarLookup
            | &AssistantQueryCapability::EmailLookup
            | &AssistantQueryCapability::Mixed
    )
}

fn derive_semantic_time_window(
    query: &str,
    user_time_zone: &str,
    capability: AssistantQueryCapability,
    now_utc: chrono::DateTime<Utc>,
) -> Option<AssistantSemanticTimeWindow> {
    let timezone = parse_time_zone_or_default(user_time_zone);
    let timezone_name = timezone.name().to_string();
    let local_today = now_utc.with_timezone(&timezone).date_naive();
    let lower_query = query.to_ascii_lowercase();

    if lower_query.contains("today") {
        return day_window(local_today, user_time_zone, timezone_name.as_str());
    }

    if lower_query.contains("yesterday") {
        let target = local_today.checked_sub_days(Days::new(1))?;
        return day_window(target, user_time_zone, timezone_name.as_str());
    }

    if lower_query.contains("tomorrow") {
        let target = local_today.checked_add_days(Days::new(1))?;
        return day_window(target, user_time_zone, timezone_name.as_str());
    }

    default_window_for_capability(
        capability,
        now_utc,
        local_today,
        user_time_zone,
        timezone_name,
    )
}

fn day_window(
    local_date: chrono::NaiveDate,
    user_time_zone: &str,
    timezone_name: &str,
) -> Option<AssistantSemanticTimeWindow> {
    let (start, end) = local_day_bounds_utc(local_date, user_time_zone)?;
    Some(AssistantSemanticTimeWindow {
        start,
        end,
        timezone: timezone_name.to_string(),
        resolution_source: AssistantTimeWindowResolutionSource::RelativeDate,
    })
}

fn default_window_for_capability(
    capability: AssistantQueryCapability,
    now_utc: chrono::DateTime<Utc>,
    local_today: chrono::NaiveDate,
    user_time_zone: &str,
    timezone_name: String,
) -> Option<AssistantSemanticTimeWindow> {
    match capability {
        AssistantQueryCapability::EmailLookup => {
            let start_date = local_today.checked_sub_days(Days::new(7))?;
            let (start_utc, _) = local_day_bounds_utc(start_date, user_time_zone)?;
            Some(AssistantSemanticTimeWindow {
                start: start_utc,
                end: now_utc,
                timezone: timezone_name,
                resolution_source: AssistantTimeWindowResolutionSource::DefaultWindow,
            })
        }
        AssistantQueryCapability::MeetingsToday
        | AssistantQueryCapability::CalendarLookup
        | AssistantQueryCapability::Mixed => {
            day_window(local_today, user_time_zone, &timezone_name).map(|mut window| {
                window.resolution_source = AssistantTimeWindowResolutionSource::DefaultWindow;
                window
            })
        }
        AssistantQueryCapability::GeneralChat => None,
    }
}

fn map_to_semantic_capability(capability: AssistantQueryCapability) -> AssistantSemanticCapability {
    match capability {
        AssistantQueryCapability::MeetingsToday | AssistantQueryCapability::CalendarLookup => {
            AssistantSemanticCapability::CalendarLookup
        }
        AssistantQueryCapability::EmailLookup => AssistantSemanticCapability::EmailLookup,
        AssistantQueryCapability::GeneralChat => AssistantSemanticCapability::GeneralChat,
        AssistantQueryCapability::Mixed => AssistantSemanticCapability::Mixed,
    }
}

fn capability_label(capability: AssistantQueryCapability) -> &'static str {
    match capability {
        AssistantQueryCapability::MeetingsToday => "meetings_today",
        AssistantQueryCapability::CalendarLookup => "calendar_lookup",
        AssistantQueryCapability::EmailLookup => "email_lookup",
        AssistantQueryCapability::GeneralChat => "general_chat",
        AssistantQueryCapability::Mixed => "mixed",
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use serde_json::json;
    use shared::assistant_memory::{
        ASSISTANT_SESSION_MEMORY_VERSION_V1, AssistantSessionMemory, AssistantSessionTurn,
    };
    use shared::models::AssistantQueryCapability;

    use super::{
        derive_semantic_time_window, parse_semantic_plan_output,
        with_default_time_window_if_missing,
    };
    use crate::http::assistant::orchestrator::planner::deterministic_fallback_plan;
    use crate::http::assistant::session_state::EnclaveAssistantSessionState;

    #[test]
    fn parse_semantic_plan_output_accepts_valid_contract() {
        let payload = json!({
            "version": "2026-02-18",
            "output": {
                "capabilities": ["email_lookup"],
                "confidence": 0.82,
                "needs_clarification": false,
                "clarifying_question": null,
                "time_window": null,
                "email_filters": {
                    "sender": "finance@example.com",
                    "keywords": ["invoice"],
                    "lookback_days": 5,
                    "unread_only": true
                },
                "language": "en"
            }
        });

        let plan = parse_semantic_plan_output(&payload, "UTC")
            .expect("valid semantic planner payload should parse");
        assert_eq!(plan.capabilities.len(), 1);
        assert!(plan.email_filters.is_some());
    }

    #[test]
    fn parse_semantic_plan_output_rejects_invalid_payload() {
        let payload = json!({
            "version": "2026-02-18",
            "output": {
                "capabilities": [],
                "confidence": 2.0,
                "needs_clarification": false,
                "clarifying_question": null
            }
        });

        let err = parse_semantic_plan_output(&payload, "UTC")
            .expect_err("invalid confidence should fail");
        assert!(format!("{err}").contains("semantic planner"));
    }

    #[test]
    fn deterministic_fallback_plan_uses_chat_when_query_is_ambiguous() {
        let plan = deterministic_fallback_plan("thanks", "UTC", None);
        assert_eq!(plan.capabilities.len(), 1);
    }

    #[test]
    fn deterministic_fallback_plan_uses_prior_capability_for_follow_up_queries() {
        let prior_state = EnclaveAssistantSessionState {
            version: ASSISTANT_SESSION_MEMORY_VERSION_V1.to_string(),
            last_capability: AssistantQueryCapability::EmailLookup,
            memory: AssistantSessionMemory {
                version: ASSISTANT_SESSION_MEMORY_VERSION_V1.to_string(),
                turns: vec![AssistantSessionTurn {
                    user_query_snippet: "anything from finance today?".to_string(),
                    assistant_summary_snippet: "Two messages matched.".to_string(),
                    capability: AssistantQueryCapability::EmailLookup,
                    created_at: Utc::now(),
                }],
            },
        };

        let plan = deterministic_fallback_plan("India?", "UTC", Some(&prior_state));
        assert_eq!(
            plan.capabilities,
            vec![AssistantQueryCapability::EmailLookup]
        );
        assert!(
            plan.time_window.is_some(),
            "fallback planner should synthesize tool time windows"
        );
    }

    #[test]
    fn derive_semantic_time_window_defaults_email_range_for_unparsed_relative_queries() {
        let now_utc = Utc
            .with_ymd_and_hms(2026, 2, 20, 12, 0, 0)
            .single()
            .expect("valid test timestamp");
        let window = derive_semantic_time_window(
            "do i have any important emails since last week?",
            "America/Los_Angeles",
            AssistantQueryCapability::EmailLookup,
            now_utc,
        )
        .expect("window should derive");

        assert_eq!(window.start.to_rfc3339(), "2026-02-13T08:00:00+00:00");
        assert_eq!(window.end.to_rfc3339(), "2026-02-20T12:00:00+00:00");
        assert_eq!(
            window.resolution_source,
            shared::assistant_semantic_plan::AssistantTimeWindowResolutionSource::DefaultWindow
        );
    }

    #[test]
    fn derive_semantic_time_window_supports_today_for_email_queries() {
        let now_utc = Utc
            .with_ymd_and_hms(2026, 2, 20, 12, 0, 0)
            .single()
            .expect("valid test timestamp");
        let window = derive_semantic_time_window(
            "do i have any emails today?",
            "America/Los_Angeles",
            AssistantQueryCapability::EmailLookup,
            now_utc,
        )
        .expect("window should derive");

        assert_eq!(window.start.to_rfc3339(), "2026-02-20T08:00:00+00:00");
        assert_eq!(window.end.to_rfc3339(), "2026-02-21T08:00:00+00:00");
    }

    #[test]
    fn enriches_missing_time_window_for_non_chat_plan() {
        let plan = shared::assistant_semantic_plan::AssistantSemanticPlan {
            capabilities: vec![AssistantQueryCapability::EmailLookup],
            confidence: 0.9,
            needs_clarification: false,
            clarifying_question: None,
            time_window: None,
            email_filters: None,
            language: Some("en".to_string()),
            planned_at: Utc
                .with_ymd_and_hms(2026, 2, 20, 12, 0, 0)
                .single()
                .expect("valid test timestamp"),
        };
        let enriched =
            with_default_time_window_if_missing("do i have any emails today?", "UTC", plan);

        assert!(enriched.time_window.is_some());
    }
}
