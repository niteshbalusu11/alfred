use std::collections::HashMap;

use shared::config::WorkerConfig;
use shared::llm::contracts::{UrgencyLevel, UrgentEmailSummaryOutput};
use shared::llm::{
    AssistantCapability, AssistantOutputContract, LlmExecutionSource, LlmGateway,
    LlmGatewayRequest, SafeOutputSource, assemble_urgent_email_candidates_context,
    generate_with_telemetry, resolve_safe_output, sanitize_context_payload,
    template_for_capability,
};
use shared::repos::{AuditResult, Store};
use shared::security::SecretRuntime;
use tracing::warn;

use super::super::JobActionResult;
use super::ai_observability::{
    append_llm_telemetry_metadata, log_llm_telemetry, record_ai_audit_event,
};
use super::fetch::fetch_urgent_email_candidates;
use super::session::{build_enclave_client, build_google_session};
use super::util::truncate_for_notification;
use crate::{JobExecutionError, NotificationContent};

const URGENT_EMAIL_CANDIDATE_MAX_RESULTS: usize = 10;
const URGENT_EMAIL_TITLE_MAX_CHARS: usize = 64;
const URGENT_EMAIL_BODY_MAX_CHARS: usize = 180;

pub(super) async fn build_urgent_email_alert(
    store: &Store,
    config: &WorkerConfig,
    secret_runtime: &SecretRuntime,
    oauth_client: &reqwest::Client,
    llm_gateway: &dyn LlmGateway,
    user_id: uuid::Uuid,
) -> Result<JobActionResult, JobExecutionError> {
    let session =
        build_google_session(store, config, secret_runtime, oauth_client, user_id).await?;
    let enclave_client = build_enclave_client(config, oauth_client);
    let fetch_outcome = fetch_urgent_email_candidates(
        &enclave_client,
        session.connector_request,
        URGENT_EMAIL_CANDIDATE_MAX_RESULTS,
    )
    .await?;
    let candidates = fetch_outcome.candidates;
    let candidates_fetched = candidates.len();
    let context = assemble_urgent_email_candidates_context(&candidates);

    let raw_context_payload = serde_json::to_value(&context).map_err(|err| {
        JobExecutionError::permanent(
            "URGENT_EMAIL_CONTEXT_SERIALIZATION_FAILED",
            format!("failed to serialize urgent email context: {err}"),
        )
    })?;
    let context_payload = sanitize_context_payload(&raw_context_payload);
    if context_payload != raw_context_payload {
        warn!(
            user_id = %user_id,
            "urgent email context payload sanitized by safety policy"
        );
    }

    let request = LlmGatewayRequest::from_template(
        template_for_capability(AssistantCapability::UrgentEmailSummary),
        context_payload.clone(),
    )
    .with_requester_id(user_id.to_string());

    let mut metadata = HashMap::new();
    metadata.insert(
        "action_source".to_string(),
        "urgent_email_llm_orchestrator".to_string(),
    );
    metadata.insert(
        "email_candidates_in_context".to_string(),
        context.candidate_count.to_string(),
    );
    metadata.insert(
        "gmail_candidates_fetched".to_string(),
        candidates_fetched.to_string(),
    );
    metadata.insert(
        "attested_measurement".to_string(),
        fetch_outcome.attested_measurement,
    );

    let (llm_result, telemetry) =
        generate_with_telemetry(llm_gateway, LlmExecutionSource::WorkerUrgentEmail, request).await;
    log_llm_telemetry(user_id, &telemetry);
    append_llm_telemetry_metadata(&mut metadata, &telemetry);

    let mut llm_request_succeeded = false;
    let model_output = match llm_result {
        Ok(response) => {
            llm_request_succeeded = true;
            if let Some(provider_request_id) = response.provider_request_id {
                metadata.insert("llm_provider_request_id".to_string(), provider_request_id);
            }
            Some(response.output)
        }
        Err(err) => {
            warn!(user_id = %user_id, "urgent email provider request failed: {err}");
            metadata.insert("llm_error".to_string(), err.to_string());
            None
        }
    };

    let resolved = resolve_safe_output(
        AssistantCapability::UrgentEmailSummary,
        model_output.as_ref(),
        &context_payload,
    );
    let output_source = match resolved.source {
        SafeOutputSource::ModelOutput => "model_output",
        SafeOutputSource::DeterministicFallback => {
            warn!(
                user_id = %user_id,
                "urgent email returned deterministic fallback output"
            );
            "deterministic_fallback"
        }
    };
    metadata.insert("llm_output_source".to_string(), output_source.to_string());
    let audit_result = if llm_request_succeeded && output_source == "model_output" {
        AuditResult::Success
    } else {
        AuditResult::Failure
    };

    let AssistantOutputContract::UrgentEmailSummary(contract) = resolved.contract else {
        return Err(JobExecutionError::permanent(
            "URGENT_EMAIL_INVALID_CONTRACT",
            "urgent email contract resolution returned unexpected capability",
        ));
    };

    metadata.insert(
        "urgent_email_should_notify".to_string(),
        contract.output.should_notify.to_string(),
    );
    metadata.insert(
        "urgent_email_urgency".to_string(),
        urgency_label(&contract.output.urgency).to_string(),
    );
    metadata.insert(
        "urgent_email_reason_present".to_string(),
        non_empty(&contract.output.reason).is_some().to_string(),
    );
    record_ai_audit_event(
        store,
        user_id,
        "AI_WORKER_URGENT_EMAIL_OUTPUT",
        audit_result,
        &metadata,
    )
    .await;

    if !contract.output.should_notify {
        metadata.insert("reason".to_string(), "llm_marked_not_urgent".to_string());
        return Ok(JobActionResult {
            notification: None,
            metadata,
        });
    }

    Ok(JobActionResult {
        notification: Some(notification_from_output(&contract.output)),
        metadata,
    })
}

fn notification_from_output(output: &UrgentEmailSummaryOutput) -> NotificationContent {
    let title = truncate_for_notification(
        notification_title_for_urgency(&output.urgency),
        URGENT_EMAIL_TITLE_MAX_CHARS,
    );
    let body = truncate_for_notification(
        &build_notification_body(output),
        URGENT_EMAIL_BODY_MAX_CHARS,
    );

    NotificationContent { title, body }
}

fn notification_title_for_urgency(urgency: &UrgencyLevel) -> &'static str {
    match urgency {
        UrgencyLevel::Critical => "Critical email alert",
        UrgencyLevel::High => "Urgent email alert",
        UrgencyLevel::Medium | UrgencyLevel::Low => "Email alert",
    }
}

fn build_notification_body(output: &UrgentEmailSummaryOutput) -> String {
    let mut segments = Vec::new();

    segments.push(
        non_empty(&output.summary)
            .unwrap_or("Urgent email needs attention.")
            .to_string(),
    );
    if let Some(action) = first_non_empty(&output.suggested_actions) {
        segments.push(format!("Action: {action}"));
    }

    segments.join(" • ")
}

fn urgency_label(urgency: &UrgencyLevel) -> &'static str {
    match urgency {
        UrgencyLevel::Low => "low",
        UrgencyLevel::Medium => "medium",
        UrgencyLevel::High => "high",
        UrgencyLevel::Critical => "critical",
    }
}

fn first_non_empty(values: &[String]) -> Option<&str> {
    values.iter().find_map(|value| non_empty(value.as_str()))
}

fn non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

#[cfg(test)]
mod tests;
