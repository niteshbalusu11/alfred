use axum::Json;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use shared::llm::{
    AssistantCapability, AssistantOutputContract, LlmExecutionSource, LlmGatewayRequest,
    SafeOutputSource, assemble_meetings_today_context, generate_with_telemetry,
    resolve_safe_output, sanitize_context_payload, template_for_capability,
};
use shared::models::{
    AssistantMeetingsTodayPayload, AssistantQueryCapability, AssistantQueryRequest,
    AssistantQueryResponse,
};
use shared::timezone::user_local_date;
use tracing::warn;

use super::super::errors::{bad_gateway_response, bad_request_response, store_error_response};
use super::super::observability::RequestContext;
use super::super::{AppState, AuthUser};
use super::ai_observability::{
    append_llm_telemetry_metadata, log_llm_telemetry, record_ai_audit_event,
};
use super::fetch::fetch_meetings_for_day;
use super::session::build_google_session;

pub(crate) async fn query_assistant(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Extension(request_context): Extension<RequestContext>,
    Json(req): Json<AssistantQueryRequest>,
) -> Response {
    let trimmed_query = req.query.trim();
    if trimmed_query.is_empty() {
        return bad_request_response("invalid_query", "Query must not be empty");
    }

    let capability = match detect_query_capability(trimmed_query) {
        Some(capability) => capability,
        None => {
            return bad_request_response(
                "unsupported_assistant_query",
                "Only meetings-today queries are currently supported",
            );
        }
    };

    match capability {
        AssistantQueryCapability::MeetingsToday => {
            handle_meetings_today_query(&state, user.user_id, &request_context.request_id).await
        }
    }
}

async fn handle_meetings_today_query(
    state: &AppState,
    user_id: uuid::Uuid,
    request_id: &str,
) -> Response {
    let session = match build_google_session(state, user_id).await {
        Ok(session) => session,
        Err(response) => return response,
    };

    let preferences = match state.store.get_or_create_preferences(user_id).await {
        Ok(preferences) => preferences,
        Err(err) => return store_error_response(err),
    };

    let calendar_day = user_local_date(Utc::now(), &preferences.time_zone);
    let meetings = match fetch_meetings_for_day(
        &state.http_client,
        &session.access_token,
        calendar_day,
        &preferences.time_zone,
    )
    .await
    {
        Ok(meetings) => meetings,
        Err(response) => return response,
    };

    let context = assemble_meetings_today_context(calendar_day, &meetings);
    let raw_context_payload = match serde_json::to_value(&context) {
        Ok(value) => value,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(shared::models::ErrorResponse {
                    error: shared::models::ErrorBody {
                        code: "internal_error".to_string(),
                        message: "Unexpected server error".to_string(),
                    },
                }),
            )
                .into_response();
        }
    };
    let context_payload = sanitize_context_payload(&raw_context_payload);
    if context_payload != raw_context_payload {
        warn!(user_id = %user_id, "assistant context payload sanitized by safety policy");
    }

    let request = LlmGatewayRequest::from_template(
        template_for_capability(AssistantCapability::MeetingsSummary),
        context_payload.clone(),
    )
    .with_requester_id(user_id.to_string());

    let (llm_result, telemetry) = generate_with_telemetry(
        &state.llm_gateway,
        LlmExecutionSource::ApiAssistantQuery,
        request,
    )
    .await;
    log_llm_telemetry(user_id, request_id, &telemetry);

    let mut audit_metadata = std::collections::HashMap::new();
    audit_metadata.insert(
        "action_source".to_string(),
        "assistant_query_llm_orchestrator".to_string(),
    );
    audit_metadata.insert("request_id".to_string(), request_id.to_string());
    append_llm_telemetry_metadata(&mut audit_metadata, &telemetry);

    let model_output = match llm_result {
        Ok(response) => Some(response.output),
        Err(err) => {
            warn!(user_id = %user_id, "assistant provider request failed: {err}");
            audit_metadata.insert("llm_error".to_string(), err.to_string());
            None
        }
    };

    let resolved = resolve_safe_output(
        AssistantCapability::MeetingsSummary,
        model_output.as_ref(),
        &context_payload,
    );
    let output_source = match resolved.source {
        SafeOutputSource::ModelOutput => "model_output",
        SafeOutputSource::DeterministicFallback => {
            warn!(user_id = %user_id, "assistant returned deterministic fallback output");
            "deterministic_fallback"
        }
    };
    audit_metadata.insert("llm_output_source".to_string(), output_source.to_string());

    let audit_result = if telemetry.outcome == "success" && output_source == "model_output" {
        shared::repos::AuditResult::Success
    } else {
        shared::repos::AuditResult::Failure
    };
    record_ai_audit_event(state, user_id, request_id, audit_result, &audit_metadata).await;

    let AssistantOutputContract::MeetingsSummary(contract) = resolved.contract else {
        return bad_gateway_response(
            "assistant_invalid_output",
            "Assistant provider returned invalid output",
        );
    };

    let payload = AssistantMeetingsTodayPayload {
        title: contract.output.title,
        summary: contract.output.summary.clone(),
        key_points: contract.output.key_points,
        follow_ups: contract.output.follow_ups,
    };

    (
        StatusCode::OK,
        Json(AssistantQueryResponse {
            capability: AssistantQueryCapability::MeetingsToday,
            display_text: payload.summary.clone(),
            payload,
        }),
    )
        .into_response()
}

fn detect_query_capability(query: &str) -> Option<AssistantQueryCapability> {
    let normalized = query.to_ascii_lowercase();
    let asks_for_today = normalized.contains("today");
    let asks_for_meetings = normalized.contains("meeting")
        || normalized.contains("calendar")
        || normalized.contains("schedule");

    if asks_for_today && asks_for_meetings {
        return Some(AssistantQueryCapability::MeetingsToday);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::detect_query_capability;
    use shared::models::AssistantQueryCapability;

    #[test]
    fn detect_query_capability_matches_meetings_today_queries() {
        let query = "What meetings do I have today?";
        assert_eq!(
            detect_query_capability(query),
            Some(AssistantQueryCapability::MeetingsToday)
        );
    }

    #[test]
    fn detect_query_capability_rejects_unsupported_queries() {
        let query = "Show me urgent emails";
        assert_eq!(detect_query_capability(query), None);
    }
}
