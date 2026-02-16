use std::collections::HashMap;

use chrono::{DateTime, Utc};
use shared::config::WorkerConfig;
use shared::llm::contracts::MorningBriefOutput;
use shared::llm::{
    AssistantCapability, AssistantOutputContract, GoogleCalendarMeetingSource, LlmExecutionSource,
    LlmGateway, LlmGatewayRequest, SafeOutputSource, assemble_morning_brief_context,
    generate_with_telemetry, resolve_safe_output, sanitize_context_payload,
    template_for_capability,
};
use shared::models::Preferences;
use shared::repos::{AuditResult, Store};
use shared::security::SecretRuntime;
use shared::timezone::{local_day_bounds_utc, user_local_date};
use tracing::warn;

use super::super::JobActionResult;
use super::ai_observability::{
    append_llm_telemetry_metadata, log_llm_telemetry, record_ai_audit_event,
};
use super::fetch::{GoogleCalendarEvent, fetch_calendar_events};
use super::session::{build_enclave_client, build_google_session};
use super::util::truncate_for_notification;
use crate::{JobExecutionError, NotificationContent};

const MORNING_BRIEF_CALENDAR_MAX_RESULTS: usize = 20;
const MORNING_BRIEF_TITLE_MAX_CHARS: usize = 64;
const MORNING_BRIEF_BODY_MAX_CHARS: usize = 180;

pub(super) async fn build_morning_brief(
    store: &Store,
    config: &WorkerConfig,
    secret_runtime: &SecretRuntime,
    oauth_client: &reqwest::Client,
    llm_gateway: &dyn LlmGateway,
    user_id: uuid::Uuid,
    preferences: &Preferences,
) -> Result<JobActionResult, JobExecutionError> {
    let session =
        build_google_session(store, config, secret_runtime, oauth_client, user_id).await?;
    let enclave_client = build_enclave_client(config, oauth_client);
    let local_date = user_local_date(Utc::now(), &preferences.time_zone);
    let (time_min, time_max) = calendar_day_window(local_date, &preferences.time_zone)?;

    let fetch_outcome = fetch_calendar_events(
        &enclave_client,
        session.connector_request,
        time_min,
        time_max,
        MORNING_BRIEF_CALENDAR_MAX_RESULTS,
    )
    .await?;
    let events = fetch_outcome.events;
    let events_fetched = events.len();

    let meetings = events
        .into_iter()
        .filter_map(calendar_event_to_meeting_source)
        .collect::<Vec<_>>();

    let context = assemble_morning_brief_context(
        local_date,
        &preferences.morning_brief_local_time,
        &meetings,
        &[],
    );

    let raw_context_payload = serde_json::to_value(&context).map_err(|_err| {
        JobExecutionError::permanent(
            "MORNING_BRIEF_CONTEXT_SERIALIZATION_FAILED",
            "failed to serialize morning brief context",
        )
    })?;
    let context_payload = sanitize_context_payload(&raw_context_payload);
    if context_payload != raw_context_payload {
        warn!(
            user_id = %user_id,
            "morning brief context payload sanitized by safety policy"
        );
    }

    let request = LlmGatewayRequest::from_template(
        template_for_capability(AssistantCapability::MorningBrief),
        context_payload.clone(),
    )
    .with_requester_id(user_id.to_string());

    let mut metadata = HashMap::new();
    metadata.insert(
        "action_source".to_string(),
        "morning_brief_llm_orchestrator".to_string(),
    );
    metadata.insert(
        "morning_brief_local_time".to_string(),
        context.morning_brief_local_time.clone(),
    );
    metadata.insert(
        "meetings_in_context".to_string(),
        context.meetings_today_count.to_string(),
    );
    metadata.insert(
        "urgent_email_candidates_in_context".to_string(),
        context.urgent_email_candidate_count.to_string(),
    );
    metadata.insert(
        "calendar_events_fetched".to_string(),
        events_fetched.to_string(),
    );
    metadata.insert(
        "urgent_email_context_source".to_string(),
        "pending_ai07_integration".to_string(),
    );
    metadata.insert(
        "attested_measurement".to_string(),
        fetch_outcome.attested_measurement,
    );

    let (llm_result, telemetry) =
        generate_with_telemetry(llm_gateway, LlmExecutionSource::WorkerMorningBrief, request).await;
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
            warn!(user_id = %user_id, "morning brief provider request failed: {err}");
            metadata.insert("llm_error".to_string(), err.to_string());
            None
        }
    };

    let resolved = resolve_safe_output(
        AssistantCapability::MorningBrief,
        model_output.as_ref(),
        &context_payload,
    );
    let output_source = match resolved.source {
        SafeOutputSource::ModelOutput => "model_output",
        SafeOutputSource::DeterministicFallback => {
            warn!(user_id = %user_id, "morning brief returned deterministic fallback output");
            "deterministic_fallback"
        }
    };
    metadata.insert("llm_output_source".to_string(), output_source.to_string());

    let audit_result = if llm_request_succeeded && output_source == "model_output" {
        AuditResult::Success
    } else {
        AuditResult::Failure
    };
    record_ai_audit_event(
        store,
        user_id,
        "AI_WORKER_MORNING_BRIEF_OUTPUT",
        audit_result,
        &metadata,
    )
    .await;

    let AssistantOutputContract::MorningBrief(contract) = resolved.contract else {
        return Err(JobExecutionError::permanent(
            "MORNING_BRIEF_INVALID_CONTRACT",
            "morning brief contract resolution returned unexpected capability",
        ));
    };

    Ok(JobActionResult {
        notification: Some(notification_from_output(&contract.output)),
        metadata,
    })
}

fn calendar_day_window(
    local_date: chrono::NaiveDate,
    time_zone: &str,
) -> Result<(DateTime<Utc>, DateTime<Utc>), JobExecutionError> {
    local_day_bounds_utc(local_date, time_zone).ok_or_else(|| {
        JobExecutionError::permanent(
            "MORNING_BRIEF_INVALID_CALENDAR_DATE",
            "unable to compute local day boundaries",
        )
    })
}

fn calendar_event_to_meeting_source(
    event: GoogleCalendarEvent,
) -> Option<GoogleCalendarMeetingSource> {
    let start_at = parse_rfc3339_utc(event.start?.date_time)?;
    let end_at = event.end.and_then(|end| parse_rfc3339_utc(end.date_time));

    Some(GoogleCalendarMeetingSource {
        event_id: event.id,
        title: event.summary,
        start_at: Some(start_at),
        end_at,
        attendee_emails: event
            .attendees
            .into_iter()
            .filter_map(|attendee| attendee.email)
            .collect(),
    })
}

fn parse_rfc3339_utc(value: Option<String>) -> Option<DateTime<Utc>> {
    let value = value?;
    DateTime::parse_from_rfc3339(&value)
        .ok()
        .map(|parsed| parsed.with_timezone(&Utc))
}

fn notification_from_output(output: &MorningBriefOutput) -> NotificationContent {
    let title = if output.headline.trim().is_empty() {
        "Morning brief".to_string()
    } else {
        truncate_for_notification(&output.headline, MORNING_BRIEF_TITLE_MAX_CHARS)
    };

    let body = build_notification_body(output);

    NotificationContent {
        title,
        body: truncate_for_notification(&body, MORNING_BRIEF_BODY_MAX_CHARS),
    }
}

fn build_notification_body(output: &MorningBriefOutput) -> String {
    let mut segments = Vec::new();

    if let Some(summary) = non_empty(&output.summary) {
        segments.push(summary.to_string());
    }
    if let Some(priority) = first_non_empty(&output.priorities) {
        segments.push(format!("Priority: {priority}"));
    }
    if let Some(schedule) = first_non_empty(&output.schedule) {
        segments.push(format!("Schedule: {schedule}"));
    }
    if let Some(alert) = first_non_empty(&output.alerts) {
        segments.push(format!("Alert: {alert}"));
    }

    if segments.is_empty() {
        return "Review your calendar and inbox for today.".to_string();
    }

    segments.join(" • ")
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
