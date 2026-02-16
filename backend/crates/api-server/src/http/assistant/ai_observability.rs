use std::collections::HashMap;

use shared::llm::LlmTelemetryEvent;
use shared::repos::AuditResult;
use tracing::{info, warn};

use super::super::AppState;

pub(super) fn append_llm_telemetry_metadata(
    metadata: &mut HashMap<String, String>,
    telemetry: &LlmTelemetryEvent,
) {
    metadata.insert("llm_provider".to_string(), telemetry.provider.clone());
    metadata.insert(
        "llm_request_outcome".to_string(),
        telemetry.outcome.to_string(),
    );
    metadata.insert(
        "llm_latency_ms".to_string(),
        telemetry.latency_ms.to_string(),
    );

    if let Some(model) = telemetry.model.as_deref() {
        metadata.insert("llm_model".to_string(), model.to_string());
    }
    if let Some(prompt_tokens) = telemetry.prompt_tokens {
        metadata.insert("llm_prompt_tokens".to_string(), prompt_tokens.to_string());
    }
    if let Some(completion_tokens) = telemetry.completion_tokens {
        metadata.insert(
            "llm_completion_tokens".to_string(),
            completion_tokens.to_string(),
        );
    }
    if let Some(total_tokens) = telemetry.total_tokens {
        metadata.insert("llm_total_tokens".to_string(), total_tokens.to_string());
    }
    if let Some(estimated_cost_usd) = telemetry.estimated_cost_usd {
        metadata.insert(
            "llm_estimated_cost_usd".to_string(),
            format!("{estimated_cost_usd:.6}"),
        );
    }
    if let Some(error_type) = telemetry.error_type {
        metadata.insert("llm_error_type".to_string(), error_type.to_string());
    }
    if let Some(alert) = telemetry.provider_degradation_alert.as_ref() {
        metadata.insert(
            "llm_degradation_provider".to_string(),
            telemetry.degradation_provider.to_string(),
        );
        metadata.insert(
            "llm_provider_degradation_failures".to_string(),
            alert.consecutive_failures.to_string(),
        );
        metadata.insert(
            "llm_provider_degradation_seconds".to_string(),
            alert.degraded_for_seconds.to_string(),
        );
    }
    if telemetry.provider_recovered {
        metadata.insert(
            "llm_degradation_provider".to_string(),
            telemetry.degradation_provider.to_string(),
        );
        metadata.insert("llm_provider_recovered".to_string(), "true".to_string());
    }
}

pub(super) fn log_llm_telemetry(
    user_id: uuid::Uuid,
    request_id: &str,
    telemetry: &LlmTelemetryEvent,
) {
    if telemetry.outcome == "failure" {
        warn!(
            metric_name = "llm_request",
            source = telemetry.source,
            capability = telemetry.capability,
            outcome = telemetry.outcome,
            request_id = request_id,
            user_id = %user_id,
            provider = telemetry.provider.as_str(),
            model = ?telemetry.model,
            latency_ms = telemetry.latency_ms,
            prompt_tokens = ?telemetry.prompt_tokens,
            completion_tokens = ?telemetry.completion_tokens,
            total_tokens = ?telemetry.total_tokens,
            estimated_cost_usd = ?telemetry.estimated_cost_usd,
            error_type = ?telemetry.error_type,
            "llm request metrics"
        );
    } else {
        info!(
            metric_name = "llm_request",
            source = telemetry.source,
            capability = telemetry.capability,
            outcome = telemetry.outcome,
            request_id = request_id,
            user_id = %user_id,
            provider = telemetry.provider.as_str(),
            model = ?telemetry.model,
            latency_ms = telemetry.latency_ms,
            prompt_tokens = ?telemetry.prompt_tokens,
            completion_tokens = ?telemetry.completion_tokens,
            total_tokens = ?telemetry.total_tokens,
            estimated_cost_usd = ?telemetry.estimated_cost_usd,
            "llm request metrics"
        );
    }

    if let Some(alert) = telemetry.provider_degradation_alert.as_ref() {
        warn!(
            event = "llm_provider_degradation_alert",
            metric_name = "llm_provider_degradation",
            request_id = request_id,
            user_id = %user_id,
            provider = telemetry.degradation_provider,
            consecutive_failures = alert.consecutive_failures,
            degraded_for_seconds = alert.degraded_for_seconds,
            "llm provider sustained degradation detected"
        );
    }

    if telemetry.provider_recovered {
        info!(
            event = "llm_provider_recovered",
            metric_name = "llm_provider_degradation",
            request_id = request_id,
            user_id = %user_id,
            provider = telemetry.degradation_provider,
            "llm provider recovered after degradation"
        );
    }
}

pub(super) async fn record_ai_audit_event(
    state: &AppState,
    user_id: uuid::Uuid,
    request_id: &str,
    result: AuditResult,
    metadata: &HashMap<String, String>,
) {
    if let Err(err) = state
        .store
        .add_audit_event(user_id, "AI_ASSISTANT_QUERY", None, result, metadata)
        .await
    {
        warn!(
            user_id = %user_id,
            request_id = request_id,
            "failed to persist AI assistant query audit event: {err}"
        );
    }
}
