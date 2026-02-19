use axum::response::Response;
use shared::assistant_semantic_plan::{AssistantSemanticEmailFilters, AssistantSemanticTimeWindow};
use shared::llm::safety::sanitize_untrusted_text;
use shared::models::{AssistantQueryCapability, AssistantResponsePart, AssistantStructuredPayload};
use tracing::warn;
use uuid::Uuid;

use super::AssistantOrchestratorResult;
use super::calendar;
use super::email;
use crate::RuntimeState;

const MIXED_MAX_CALENDAR_KEY_POINTS: usize = 2;
const MIXED_MAX_EMAIL_KEY_POINTS: usize = 2;
const MIXED_MAX_FOLLOW_UPS: usize = 4;
const MIXED_QUERY_SNIPPET_MAX_CHARS: usize = 120;

pub(super) async fn execute_mixed_query(
    state: &RuntimeState,
    user_id: Uuid,
    request_id: &str,
    query: &str,
    time_window: &AssistantSemanticTimeWindow,
    email_filters: &AssistantSemanticEmailFilters,
) -> Result<AssistantOrchestratorResult, Response> {
    let (calendar_result, email_result) = tokio::join!(
        calendar::execute_calendar_query(
            state,
            user_id,
            request_id,
            AssistantQueryCapability::CalendarLookup,
            time_window,
        ),
        email::execute_email_query(state, user_id, request_id, email_filters,),
    );

    match (calendar_result, email_result) {
        (Ok(calendar), Ok(email)) => {
            let payload = compose_full_mixed_payload(query, &calendar.payload, &email.payload);
            let display_text = payload.summary.clone();
            let response_parts = compose_full_response_parts(
                display_text.clone(),
                &calendar.capability,
                &calendar.payload,
                &email.capability,
                &email.payload,
            );

            Ok(AssistantOrchestratorResult {
                capability: AssistantQueryCapability::Mixed,
                display_text,
                payload,
                response_parts,
                attested_identity: calendar.attested_identity,
            })
        }
        (Ok(calendar), Err(_)) => {
            warn!(
                user_id = %user_id,
                "mixed assistant query returned partial results: email lane failed"
            );
            let payload = compose_partial_payload(query, &calendar.payload, "email");
            let display_text = payload.summary.clone();
            let response_parts = compose_partial_response_parts(
                display_text.clone(),
                &calendar.capability,
                &calendar.payload,
            );

            Ok(AssistantOrchestratorResult {
                capability: AssistantQueryCapability::Mixed,
                display_text,
                payload,
                response_parts,
                attested_identity: calendar.attested_identity,
            })
        }
        (Err(_), Ok(email)) => {
            warn!(
                user_id = %user_id,
                "mixed assistant query returned partial results: calendar lane failed"
            );
            let payload = compose_partial_payload(query, &email.payload, "calendar");
            let display_text = payload.summary.clone();
            let response_parts = compose_partial_response_parts(
                display_text.clone(),
                &email.capability,
                &email.payload,
            );

            Ok(AssistantOrchestratorResult {
                capability: AssistantQueryCapability::Mixed,
                display_text,
                payload,
                response_parts,
                attested_identity: email.attested_identity,
            })
        }
        (Err(primary_error), Err(_)) => {
            warn!(
                user_id = %user_id,
                "mixed assistant query failed: both calendar and email lanes errored"
            );
            Err(primary_error)
        }
    }
}

fn compose_full_mixed_payload(
    query: &str,
    calendar: &AssistantStructuredPayload,
    email: &AssistantStructuredPayload,
) -> AssistantStructuredPayload {
    let mut key_points = Vec::new();
    key_points.extend(
        calendar
            .key_points
            .iter()
            .take(MIXED_MAX_CALENDAR_KEY_POINTS)
            .map(|entry| format!("Calendar: {entry}")),
    );
    key_points.extend(
        email
            .key_points
            .iter()
            .take(MIXED_MAX_EMAIL_KEY_POINTS)
            .map(|entry| format!("Email: {entry}")),
    );

    if key_points.is_empty() {
        key_points.push(format!("Calendar: {}", calendar.summary));
        key_points.push(format!("Email: {}", email.summary));
    }

    let query_snippet = sanitize_untrusted_text(query)
        .chars()
        .take(MIXED_QUERY_SNIPPET_MAX_CHARS)
        .collect::<String>();
    let summary = if query_snippet.is_empty() {
        "Here is a combined summary from your calendar and inbox.".to_string()
    } else {
        format!("For \"{query_snippet}\", here is a combined summary from your calendar and inbox.")
    };

    AssistantStructuredPayload {
        title: "Calendar and inbox summary".to_string(),
        summary,
        key_points,
        follow_ups: combine_follow_ups(&calendar.follow_ups, &email.follow_ups),
    }
}

fn compose_partial_payload(
    query: &str,
    successful_payload: &AssistantStructuredPayload,
    unavailable_lane: &str,
) -> AssistantStructuredPayload {
    let mut follow_ups = successful_payload.follow_ups.clone();
    follow_ups.push(format!(
        "Try again to include {} details in the combined summary.",
        unavailable_lane
    ));
    follow_ups.truncate(MIXED_MAX_FOLLOW_UPS);

    let query_snippet = sanitize_untrusted_text(query)
        .chars()
        .take(MIXED_QUERY_SNIPPET_MAX_CHARS)
        .collect::<String>();
    let summary = if query_snippet.is_empty() {
        format!(
            "I could only retrieve part of your request this turn; {} details were unavailable.",
            unavailable_lane
        )
    } else {
        format!(
            "For \"{query_snippet}\", I could only retrieve part of your request this turn; {} details were unavailable.",
            unavailable_lane
        )
    };

    AssistantStructuredPayload {
        title: "Partial combined summary".to_string(),
        summary,
        key_points: successful_payload.key_points.clone(),
        follow_ups,
    }
}

fn compose_full_response_parts(
    display_text: String,
    calendar_capability: &AssistantQueryCapability,
    calendar_payload: &AssistantStructuredPayload,
    email_capability: &AssistantQueryCapability,
    email_payload: &AssistantStructuredPayload,
) -> Vec<AssistantResponsePart> {
    vec![
        AssistantResponsePart::chat_text(display_text),
        AssistantResponsePart::tool_summary(calendar_capability.clone(), calendar_payload.clone()),
        AssistantResponsePart::tool_summary(email_capability.clone(), email_payload.clone()),
    ]
}

fn compose_partial_response_parts(
    display_text: String,
    capability: &AssistantQueryCapability,
    payload: &AssistantStructuredPayload,
) -> Vec<AssistantResponsePart> {
    vec![
        AssistantResponsePart::chat_text(display_text),
        AssistantResponsePart::tool_summary(capability.clone(), payload.clone()),
    ]
}

fn combine_follow_ups(calendar: &[String], email: &[String]) -> Vec<String> {
    let mut merged = Vec::new();
    for follow_up in calendar.iter().chain(email.iter()) {
        if follow_up.trim().is_empty() || merged.contains(follow_up) {
            continue;
        }
        merged.push(follow_up.clone());
        if merged.len() == MIXED_MAX_FOLLOW_UPS {
            return merged;
        }
    }

    if merged.is_empty() {
        merged.push("Ask a narrower calendar or inbox follow-up.".to_string());
    }

    merged
}

#[cfg(test)]
mod mixed_tests;
