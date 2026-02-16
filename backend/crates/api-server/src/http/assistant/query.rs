use axum::Json;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use shared::llm::{
    AssistantCapability, AssistantOutputContract, LlmGateway, LlmGatewayRequest, SafeOutputSource,
    assemble_meetings_today_context, resolve_safe_output, sanitize_context_payload,
    template_for_capability,
};
use shared::models::{
    AssistantMeetingsTodayPayload, AssistantQueryCapability, AssistantQueryRequest,
    AssistantQueryResponse,
};
use shared::timezone::user_local_date;
use tracing::warn;

use super::super::errors::{bad_gateway_response, bad_request_response, store_error_response};
use super::super::{AppState, AuthUser};
use super::fetch::fetch_meetings_for_day;
use super::session::build_google_session;

pub(crate) async fn query_assistant(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
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
            handle_meetings_today_query(&state, user.user_id).await
        }
    }
}

async fn handle_meetings_today_query(state: &AppState, user_id: uuid::Uuid) -> Response {
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
    );

    let model_output = match state.llm_gateway.generate(request).await {
        Ok(response) => Some(response.output),
        Err(err) => {
            warn!(user_id = %user_id, "assistant provider request failed: {err}");
            None
        }
    };

    let resolved = resolve_safe_output(
        AssistantCapability::MeetingsSummary,
        model_output.as_ref(),
        &context_payload,
    );
    if matches!(resolved.source, SafeOutputSource::DeterministicFallback) {
        warn!(user_id = %user_id, "assistant returned deterministic fallback output");
    }

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
