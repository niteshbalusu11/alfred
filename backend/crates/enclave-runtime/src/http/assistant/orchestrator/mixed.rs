use axum::response::Response;
use shared::models::{AssistantQueryCapability, AssistantStructuredPayload};
use tracing::warn;
use uuid::Uuid;

use super::super::session_state::EnclaveAssistantSessionState;
use super::AssistantOrchestratorResult;
use super::calendar;
use super::email;
use crate::RuntimeState;

const MIXED_MAX_CALENDAR_KEY_POINTS: usize = 2;
const MIXED_MAX_EMAIL_KEY_POINTS: usize = 2;
const MIXED_MAX_FOLLOW_UPS: usize = 4;

pub(super) async fn execute_mixed_query(
    state: &RuntimeState,
    user_id: Uuid,
    request_id: &str,
    query: &str,
    user_time_zone: &str,
    prior_state: Option<&EnclaveAssistantSessionState>,
) -> Result<AssistantOrchestratorResult, Response> {
    let (calendar_result, email_result) = tokio::join!(
        calendar::execute_calendar_query(
            state,
            user_id,
            request_id,
            query,
            AssistantQueryCapability::CalendarLookup,
            user_time_zone,
            prior_state,
        ),
        email::execute_email_query(
            state,
            user_id,
            request_id,
            query,
            user_time_zone,
            prior_state,
        ),
    );

    match (calendar_result, email_result) {
        (Ok(calendar), Ok(email)) => {
            let payload = compose_full_mixed_payload(&calendar.payload, &email.payload);
            let display_text = payload.summary.clone();

            Ok(AssistantOrchestratorResult {
                capability: AssistantQueryCapability::Mixed,
                display_text,
                payload,
                attested_identity: calendar.attested_identity,
            })
        }
        (Ok(calendar), Err(_)) => {
            warn!(
                user_id = %user_id,
                "mixed assistant query returned partial results: email lane failed"
            );
            let payload = compose_partial_payload(&calendar.payload, "email");
            let display_text = payload.summary.clone();

            Ok(AssistantOrchestratorResult {
                capability: AssistantQueryCapability::Mixed,
                display_text,
                payload,
                attested_identity: calendar.attested_identity,
            })
        }
        (Err(_), Ok(email)) => {
            warn!(
                user_id = %user_id,
                "mixed assistant query returned partial results: calendar lane failed"
            );
            let payload = compose_partial_payload(&email.payload, "calendar");
            let display_text = payload.summary.clone();

            Ok(AssistantOrchestratorResult {
                capability: AssistantQueryCapability::Mixed,
                display_text,
                payload,
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

    AssistantStructuredPayload {
        title: "Calendar and inbox summary".to_string(),
        summary: "Here is a combined summary from your calendar and inbox.".to_string(),
        key_points,
        follow_ups: combine_follow_ups(&calendar.follow_ups, &email.follow_ups),
    }
}

fn compose_partial_payload(
    successful_payload: &AssistantStructuredPayload,
    unavailable_lane: &str,
) -> AssistantStructuredPayload {
    let mut follow_ups = successful_payload.follow_ups.clone();
    follow_ups.push(format!(
        "Try again to include {} details in the combined summary.",
        unavailable_lane
    ));
    follow_ups.truncate(MIXED_MAX_FOLLOW_UPS);

    AssistantStructuredPayload {
        title: "Partial combined summary".to_string(),
        summary: format!(
            "I could only retrieve part of your request this turn; {} details were unavailable.",
            unavailable_lane
        ),
        key_points: successful_payload.key_points.clone(),
        follow_ups,
    }
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
mod tests {
    use super::{combine_follow_ups, compose_full_mixed_payload};
    use shared::models::AssistantStructuredPayload;

    #[test]
    fn compose_full_mixed_payload_prefixes_calendar_and_email_points() {
        let calendar = AssistantStructuredPayload {
            title: "Calendar".to_string(),
            summary: "Calendar summary".to_string(),
            key_points: vec!["10:00 Team sync".to_string()],
            follow_ups: vec!["Ask for tomorrow.".to_string()],
        };
        let email = AssistantStructuredPayload {
            title: "Email".to_string(),
            summary: "Email summary".to_string(),
            key_points: vec!["finance@example.com - Invoice".to_string()],
            follow_ups: vec!["Filter by sender.".to_string()],
        };

        let payload = compose_full_mixed_payload(&calendar, &email);
        assert_eq!(payload.title, "Calendar and inbox summary");
        assert_eq!(
            payload.key_points,
            vec![
                "Calendar: 10:00 Team sync".to_string(),
                "Email: finance@example.com - Invoice".to_string(),
            ]
        );
        assert_eq!(
            payload.follow_ups,
            vec![
                "Ask for tomorrow.".to_string(),
                "Filter by sender.".to_string()
            ]
        );
    }

    #[test]
    fn combine_follow_ups_deduplicates_and_limits_results() {
        let follow_ups = combine_follow_ups(
            &[
                "Ask for tomorrow.".to_string(),
                "Filter by sender.".to_string(),
                "Filter by sender.".to_string(),
            ],
            &[
                "Ask for tomorrow.".to_string(),
                "Show this week.".to_string(),
                "Show next week.".to_string(),
                "Extra item".to_string(),
            ],
        );

        assert_eq!(
            follow_ups,
            vec![
                "Ask for tomorrow.".to_string(),
                "Filter by sender.".to_string(),
                "Show this week.".to_string(),
                "Show next week.".to_string(),
            ]
        );
    }
}
