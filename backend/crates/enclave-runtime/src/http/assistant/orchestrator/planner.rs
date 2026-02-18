use chrono::Utc;
use serde_json::{Value, json};
use shared::assistant_semantic_plan::{
    AssistantSemanticCapability, AssistantSemanticPlan, AssistantSemanticPlanOutput,
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

pub(super) struct SemanticPlanResolution {
    pub(super) plan: AssistantSemanticPlan,
    pub(super) used_deterministic_fallback: bool,
}

pub(super) async fn resolve_semantic_plan(
    state: &RuntimeState,
    user_id: Uuid,
    query: &str,
    user_time_zone: &str,
    prior_state: Option<&EnclaveAssistantSessionState>,
) -> SemanticPlanResolution {
    let mut context_payload = json!({
        "query_context": query_context_snippet(query),
        "user_time_zone": user_time_zone,
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
        state.llm_gateway.as_ref(),
        LlmExecutionSource::ApiAssistantQuery,
        llm_request,
    )
    .await;
    super::super::mapping::log_telemetry(user_id, &telemetry, "assistant_semantic_planner");

    match llm_result {
        Ok(response) => match parse_semantic_plan_output(&response.output, user_time_zone) {
            Ok(plan) => {
                info!(
                    user_id = %user_id,
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

    normalize_semantic_plan_output(output, user_time_zone, Utc::now())
        .expect("deterministic semantic planner fallback must normalize")
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
    use serde_json::json;

    use super::parse_semantic_plan_output;
    use crate::http::assistant::orchestrator::planner::deterministic_fallback_plan;

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
}
