use axum::Json;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use shared::llm::{
    AssistantCapability, AssistantOutputContract, LlmGateway, LlmGatewayError, LlmGatewayRequest,
    assemble_meetings_today_context, template_for_capability, validate_output_value,
};
use shared::models::{
    AssistantMeetingsTodayPayload, AssistantQueryCapability, AssistantQueryRequest,
    AssistantQueryResponse,
};
use tracing::warn;

use super::super::errors::{bad_gateway_response, bad_request_response};
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

    let calendar_day = Utc::now().date_naive();
    let meetings =
        match fetch_meetings_for_day(&state.http_client, &session.access_token, calendar_day).await
        {
            Ok(meetings) => meetings,
            Err(response) => return response,
        };

    let context = assemble_meetings_today_context(calendar_day, &meetings);
    let context_payload = match serde_json::to_value(&context) {
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

    let request = LlmGatewayRequest::from_template(
        template_for_capability(AssistantCapability::MeetingsSummary),
        context_payload,
    );

    let llm_response = match state.llm_gateway.generate(request).await {
        Ok(response) => response,
        Err(err) => return map_gateway_error(err),
    };

    let contract =
        match validate_output_value(AssistantCapability::MeetingsSummary, &llm_response.output) {
            Ok(contract) => contract,
            Err(err) => {
                warn!("assistant output validation failed: {err}");
                return bad_gateway_response(
                    "assistant_invalid_output",
                    "Assistant provider returned invalid output",
                );
            }
        };

    let AssistantOutputContract::MeetingsSummary(contract) = contract else {
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

fn map_gateway_error(err: LlmGatewayError) -> Response {
    match err {
        LlmGatewayError::Timeout => {
            bad_gateway_response("assistant_provider_timeout", "Assistant provider timed out")
        }
        LlmGatewayError::ProviderFailure(_) => bad_gateway_response(
            "assistant_provider_failed",
            "Assistant provider request failed",
        ),
        LlmGatewayError::InvalidProviderPayload(_) => bad_gateway_response(
            "assistant_provider_invalid",
            "Assistant provider returned invalid payload",
        ),
    }
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
