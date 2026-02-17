use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use serde_json::{Value, json};
use shared::llm::{
    AssistantCapability, AssistantOutputContract, LlmExecutionSource, LlmGatewayRequest,
    SafeOutputSource, assemble_urgent_email_candidates_context, generate_with_telemetry,
    resolve_safe_output, sanitize_context_payload, template_for_capability,
};
use shared::models::{AssistantQueryCapability, AssistantStructuredPayload};
use tracing::warn;
use uuid::Uuid;

use super::super::mapping::{log_telemetry, map_email_candidate_source};
use super::super::memory::{query_context_snippet, session_memory_context};
use super::super::notifications::non_empty;
use super::super::session_state::EnclaveAssistantSessionState;
use super::AssistantOrchestratorResult;
use super::email_fallback::{
    deterministic_email_fallback_payload, format_email_key_point, title_for_email_results,
};
use super::email_plan::{apply_email_filters, build_gmail_query, plan_email_query};
use crate::RuntimeState;
use crate::http::rpc;

const EMAIL_MAX_RESULTS: usize = 20;
const MAX_MODEL_KEY_POINTS: usize = 3;

pub(super) async fn execute_email_query(
    state: &RuntimeState,
    user_id: Uuid,
    request_id: &str,
    query: &str,
    prior_state: Option<&EnclaveAssistantSessionState>,
) -> Result<AssistantOrchestratorResult, Response> {
    let connector = match state
        .enclave_service
        .resolve_active_google_connector_request(user_id)
        .await
    {
        Ok(connector) => connector,
        Err(err) => {
            return Err(
                rpc::map_rpc_service_error(err, Some(request_id.to_string())).into_response(),
            );
        }
    };

    let plan = plan_email_query(query);
    let now = Utc::now();

    let fetch_response = match state
        .enclave_service
        .fetch_google_email_candidates(connector, Some(build_gmail_query(&plan)), EMAIL_MAX_RESULTS)
        .await
    {
        Ok(response) => response,
        Err(err) => {
            return Err(
                rpc::map_rpc_service_error(err, Some(request_id.to_string())).into_response(),
            );
        }
    };

    let raw_candidates = fetch_response
        .candidates
        .iter()
        .map(map_email_candidate_source)
        .collect::<Vec<_>>();
    let candidates = apply_email_filters(raw_candidates, &plan, now);

    let context = assemble_urgent_email_candidates_context(&candidates);
    let mut context_payload = match serde_json::to_value(&context) {
        Ok(value) => value,
        Err(_) => {
            return Err(rpc::reject(
                StatusCode::INTERNAL_SERVER_ERROR,
                shared::enclave::EnclaveRpcErrorEnvelope::new(
                    Some(request_id.to_string()),
                    "rpc_internal_error",
                    "failed to serialize email context",
                    true,
                ),
            )
            .into_response());
        }
    };

    if let Value::Object(entries) = &mut context_payload {
        entries.insert(
            "query_context".to_string(),
            Value::String(query_context_snippet(query)),
        );
        entries.insert(
            "query_plan".to_string(),
            json!({
                "window_label": plan.window_label,
                "lookback_days": plan.lookback_days,
                "sender_filter": plan.sender_filter,
            }),
        );
        if let Some(memory_context) =
            session_memory_context(prior_state.as_ref().map(|state| &state.memory))
        {
            entries.insert("session_memory".to_string(), memory_context);
        }
    }

    let context_payload = sanitize_context_payload(&context_payload);
    let llm_request = LlmGatewayRequest::from_template(
        template_for_capability(AssistantCapability::UrgentEmailSummary),
        context_payload.clone(),
    )
    .with_requester_id(user_id.to_string());

    let (llm_result, telemetry) = generate_with_telemetry(
        state.llm_gateway.as_ref(),
        LlmExecutionSource::ApiAssistantQuery,
        llm_request,
    )
    .await;
    log_telemetry(user_id, &telemetry, "assistant_query");

    let model_output = match llm_result {
        Ok(response) => response.output,
        Err(err) => {
            warn!(user_id = %user_id, "assistant email provider request failed: {err}");
            Value::Null
        }
    };

    let resolved = resolve_safe_output(
        AssistantCapability::UrgentEmailSummary,
        if model_output.is_null() {
            None
        } else {
            Some(&model_output)
        },
        &context_payload,
    );

    let payload = if resolved.source == SafeOutputSource::DeterministicFallback {
        deterministic_email_fallback_payload(&plan, &candidates)
    } else {
        let AssistantOutputContract::UrgentEmailSummary(contract) = resolved.contract else {
            return Err(rpc::reject(
                StatusCode::INTERNAL_SERVER_ERROR,
                shared::enclave::EnclaveRpcErrorEnvelope::new(
                    Some(request_id.to_string()),
                    "rpc_internal_error",
                    "assistant email contract resolution failed",
                    true,
                ),
            )
            .into_response());
        };

        let mut key_points = Vec::new();
        if let Some(reason) = non_empty(contract.output.reason.as_str()) {
            key_points.push(format!("Reason: {reason}"));
        }
        key_points.extend(
            candidates
                .iter()
                .take(MAX_MODEL_KEY_POINTS)
                .map(format_email_key_point),
        );

        AssistantStructuredPayload {
            title: title_for_email_results(&plan),
            summary: non_empty(contract.output.summary.as_str())
                .unwrap_or("Here is your inbox summary.")
                .to_string(),
            key_points,
            follow_ups: if contract.output.suggested_actions.is_empty() {
                vec!["Ask for a narrower sender or timeframe.".to_string()]
            } else {
                contract.output.suggested_actions
            },
        }
    };

    let display_text = non_empty(payload.summary.as_str())
        .unwrap_or("Here is your inbox summary.")
        .to_string();

    Ok(AssistantOrchestratorResult {
        capability: AssistantQueryCapability::EmailLookup,
        display_text,
        payload,
        attested_identity: fetch_response.attested_identity,
    })
}
