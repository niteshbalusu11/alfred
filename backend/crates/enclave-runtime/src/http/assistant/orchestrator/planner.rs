use chrono::Utc;
use serde_json::{Value, json};
use shared::assistant_semantic_plan::{
    AssistantSemanticCapability, AssistantSemanticPlan, AssistantSemanticPlanOutput,
    AssistantSemanticResponseOutput, normalize_semantic_plan_output,
};
use shared::llm::safety::sanitize_untrusted_text;
use shared::llm::{
    AssistantCapability, AssistantOutputContract, LlmExecutionSource, LlmGatewayError,
    LlmGatewayRequest, generate_with_telemetry, sanitize_context_payload, template_for_capability,
    validate_output_value,
};
use shared::models::{AssistantQueryCapability, AssistantStructuredPayload};
use tracing::{info, warn};
use uuid::Uuid;

use super::super::memory::{
    query_context_snippet, session_memory_context, should_include_follow_up_context,
};
use super::super::session_state::EnclaveAssistantSessionState;
use crate::RuntimeState;

pub(super) struct SemanticPlanResolution {
    pub(super) plan: AssistantSemanticPlan,
    pub(super) model_response_payload: Option<AssistantStructuredPayload>,
    pub(super) used_deterministic_fallback: bool,
}

const MAX_RESPONSE_TITLE_CHARS: usize = 120;
const MAX_RESPONSE_SUMMARY_CHARS: usize = 500;
const MAX_RESPONSE_LIST_ITEMS: usize = 8;
const MAX_RESPONSE_LIST_ITEM_CHARS: usize = 320;

pub(super) async fn resolve_semantic_plan(
    state: &RuntimeState,
    user_id: Uuid,
    request_id: &str,
    query: &str,
    user_time_zone: &str,
    prior_state: Option<&EnclaveAssistantSessionState>,
) -> SemanticPlanResolution {
    let mut context_payload = json!({
        "query_context": query_context_snippet(query),
        "user_time_zone": user_time_zone,
    });
    if let Value::Object(entries) = &mut context_payload
        && let Some(prior_state) = prior_state
        && should_include_follow_up_context(query, &prior_state.last_capability)
    {
        if let Some(memory_context) = session_memory_context(Some(&prior_state.memory)) {
            entries.insert("session_memory".to_string(), memory_context);
        }
        entries.insert(
            "prior_capability".to_string(),
            json!(capability_label(prior_state.last_capability.clone())),
        );
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
            Ok((plan, model_response_payload)) => {
                info!(
                    user_id = %user_id,
                    request_id,
                    confidence = plan.confidence,
                    needs_clarification = plan.needs_clarification,
                    "assistant semantic planner resolved model output"
                );
                SemanticPlanResolution {
                    plan,
                    model_response_payload,
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
                    model_response_payload: None,
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
                model_response_payload: None,
                used_deterministic_fallback: true,
            }
        }
    }
}

fn parse_semantic_plan_output(
    payload: &Value,
    user_time_zone: &str,
) -> Result<(AssistantSemanticPlan, Option<AssistantStructuredPayload>), LlmGatewayError> {
    let contract = validate_output_value(AssistantCapability::AssistantSemanticPlan, payload)
        .map_err(|err| LlmGatewayError::InvalidProviderPayload(err.to_string()))?;
    let AssistantOutputContract::AssistantSemanticPlan(contract) = contract else {
        return Err(LlmGatewayError::InvalidProviderPayload(
            "semantic planner contract type mismatch".to_string(),
        ));
    };
    let output = contract.output;
    let model_response_payload = normalize_semantic_response(output.response.clone());
    let plan = normalize_semantic_plan_output(output, user_time_zone, Utc::now())
        .map_err(|err| LlmGatewayError::InvalidProviderPayload(err.to_string()))?;

    Ok((plan, model_response_payload))
}

fn deterministic_fallback_plan(
    _query: &str,
    user_time_zone: &str,
    _prior_state: Option<&EnclaveAssistantSessionState>,
) -> AssistantSemanticPlan {
    let output = AssistantSemanticPlanOutput {
        capabilities: vec![AssistantSemanticCapability::GeneralChat],
        confidence: 0.0,
        needs_clarification: false,
        clarifying_question: None,
        time_window: None,
        email_filters: None,
        language: None,
        response: None,
    };

    normalize_semantic_plan_output(output, user_time_zone, Utc::now())
        .expect("deterministic semantic planner fallback must normalize")
}

fn normalize_semantic_response(
    response: Option<AssistantSemanticResponseOutput>,
) -> Option<AssistantStructuredPayload> {
    let response = response?;
    let summary = truncate_and_sanitize(response.summary.as_str(), MAX_RESPONSE_SUMMARY_CHARS);
    if summary.is_empty() {
        return None;
    }

    let title = truncate_and_sanitize(response.title.as_str(), MAX_RESPONSE_TITLE_CHARS);
    Some(AssistantStructuredPayload {
        title: if title.is_empty() {
            "General conversation".to_string()
        } else {
            title
        },
        summary,
        key_points: sanitize_list(response.key_points),
        follow_ups: sanitize_list(response.follow_ups),
    })
}

fn sanitize_list(items: Vec<String>) -> Vec<String> {
    items
        .into_iter()
        .map(|item| truncate_and_sanitize(item.as_str(), MAX_RESPONSE_LIST_ITEM_CHARS))
        .filter(|item| !item.is_empty())
        .take(MAX_RESPONSE_LIST_ITEMS)
        .collect()
}

fn truncate_and_sanitize(value: &str, limit: usize) -> String {
    sanitize_untrusted_text(value)
        .chars()
        .take(limit)
        .collect::<String>()
        .trim()
        .to_string()
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
    use shared::models::AssistantQueryCapability;

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

        let (plan, model_response_payload) = parse_semantic_plan_output(&payload, "UTC")
            .expect("valid semantic planner payload should parse");
        assert_eq!(plan.capabilities.len(), 1);
        assert!(plan.email_filters.is_some());
        assert!(model_response_payload.is_none());
    }

    #[test]
    fn parse_semantic_plan_output_extracts_response_payload() {
        let payload = json!({
            "version": "2026-02-18",
            "output": {
                "capabilities": ["general_chat"],
                "confidence": 0.88,
                "needs_clarification": false,
                "clarifying_question": null,
                "time_window": null,
                "email_filters": null,
                "language": "en",
                "response": {
                    "title": "Trip planning",
                    "summary": "I can help you plan a practical Alaska trip.",
                    "key_points": ["Pick dates first", "Estimate total budget"],
                    "follow_ups": ["Ask me for a 7-day itinerary"]
                }
            }
        });

        let (plan, model_response_payload) = parse_semantic_plan_output(&payload, "UTC")
            .expect("valid semantic planner payload with response should parse");
        assert_eq!(
            plan.capabilities,
            vec![AssistantQueryCapability::GeneralChat]
        );
        let payload = model_response_payload.expect("response payload should be extracted");
        assert_eq!(payload.title, "Trip planning");
        assert!(payload.summary.contains("Alaska"));
        assert_eq!(payload.key_points.len(), 2);
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
    fn deterministic_fallback_plan_defaults_to_general_chat_execution() {
        let plan = deterministic_fallback_plan("thanks", "UTC", None);
        assert_eq!(
            plan.capabilities,
            vec![AssistantQueryCapability::GeneralChat]
        );
        assert!(!plan.needs_clarification);
        assert!(plan.clarifying_question.is_none());
    }
}
