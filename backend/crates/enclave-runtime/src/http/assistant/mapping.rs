use std::collections::HashMap;

use chrono::{DateTime, Utc};
use tracing::info;
use uuid::Uuid;

pub(super) fn map_calendar_event_to_meeting_source(
    event: &shared::enclave::EnclaveGoogleCalendarEvent,
) -> shared::llm::GoogleCalendarMeetingSource {
    shared::llm::GoogleCalendarMeetingSource {
        event_id: event.id.clone(),
        title: event.summary.clone(),
        start_at: event
            .start
            .as_ref()
            .and_then(|start| start.date_time.as_deref())
            .and_then(parse_utc_datetime),
        end_at: event
            .end
            .as_ref()
            .and_then(|end| end.date_time.as_deref())
            .and_then(parse_utc_datetime),
        attendee_emails: event
            .attendees
            .iter()
            .filter_map(|attendee| attendee.email.clone())
            .collect(),
    }
}

pub(super) fn map_email_candidate_source(
    candidate: &shared::enclave::EnclaveGoogleEmailCandidate,
) -> shared::llm::GoogleEmailCandidateSource {
    shared::llm::GoogleEmailCandidateSource {
        message_id: candidate.message_id.clone(),
        from: candidate.from.clone(),
        subject: candidate.subject.clone(),
        snippet: candidate.snippet.clone(),
        received_at: candidate
            .received_at
            .as_deref()
            .and_then(parse_utc_datetime),
        label_ids: candidate.label_ids.clone(),
        has_attachments: candidate.has_attachments,
    }
}

pub(super) fn append_llm_telemetry_metadata(
    metadata: &mut HashMap<String, String>,
    telemetry: &shared::llm::LlmTelemetryEvent,
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
}

pub(super) fn log_telemetry(user_id: Uuid, telemetry: &shared::llm::LlmTelemetryEvent, flow: &str) {
    info!(
        flow,
        source = telemetry.source,
        capability = telemetry.capability,
        outcome = telemetry.outcome,
        user_id = %user_id,
        provider = telemetry.provider.as_str(),
        model = ?telemetry.model,
        latency_ms = telemetry.latency_ms,
        prompt_tokens = ?telemetry.prompt_tokens,
        completion_tokens = ?telemetry.completion_tokens,
        total_tokens = ?telemetry.total_tokens,
        estimated_cost_usd = ?telemetry.estimated_cost_usd,
        "enclave llm request metrics"
    );
}

fn parse_utc_datetime(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|parsed| parsed.with_timezone(&Utc))
}
