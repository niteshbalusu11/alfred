use axum::Json;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use serde_json::Value;
use shared::llm::{
    AssistantCapability, AssistantOutputContract, LlmExecutionSource, LlmGatewayRequest,
    SafeOutputSource, assemble_meetings_today_context, generate_with_telemetry,
    resolve_safe_output, sanitize_context_payload, template_for_capability,
};
use shared::models::{
    AssistantMeetingsTodayPayload, AssistantQueryCapability, AssistantQueryRequest,
    AssistantQueryResponse,
};
use shared::repos::{AssistantSessionRecord, AuditResult};
use shared::timezone::user_local_date;
use tracing::warn;
use uuid::Uuid;

use super::super::errors::{bad_gateway_response, bad_request_response, store_error_response};
use super::super::observability::RequestContext;
use super::super::{AppState, AuthUser};
use super::ai_observability::{
    append_llm_telemetry_metadata, log_llm_telemetry, record_ai_audit_event,
};
use super::fetch::fetch_meetings_for_day;
use super::memory::{
    ASSISTANT_SESSION_TTL_SECONDS, build_updated_memory, detect_query_capability,
    query_context_snippet, resolve_query_capability, session_memory_context,
};
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

    let now = Utc::now();
    let session_id = req.session_id.unwrap_or_else(Uuid::new_v4);
    let session_record = match state
        .store
        .load_assistant_session(user.user_id, session_id, now)
        .await
    {
        Ok(record) => record,
        Err(err) => return store_error_response(err),
    };

    let capability = match resolve_query_capability(
        trimmed_query,
        detect_query_capability(trimmed_query),
        session_record
            .as_ref()
            .map(|record| record.last_capability.clone()),
    ) {
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
            handle_meetings_today_query(
                &state,
                user.user_id,
                &request_context.request_id,
                session_id,
                trimmed_query,
                session_record.as_ref(),
                now,
            )
            .await
        }
    }
}

async fn handle_meetings_today_query(
    state: &AppState,
    user_id: uuid::Uuid,
    request_id: &str,
    session_id: Uuid,
    query: &str,
    session_record: Option<&AssistantSessionRecord>,
    now: chrono::DateTime<Utc>,
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
    let mut context_payload = sanitize_context_payload(&raw_context_payload);
    if context_payload != raw_context_payload {
        warn!(user_id = %user_id, "assistant context payload sanitized by safety policy");
    }
    if let Value::Object(payload_object) = &mut context_payload {
        payload_object.insert(
            "current_query".to_string(),
            Value::String(query_context_snippet(query)),
        );
    }
    if let Some(memory_context) =
        session_memory_context(session_record.map(|record| &record.memory))
        && let Value::Object(payload_object) = &mut context_payload
    {
        payload_object.insert("session_memory".to_string(), memory_context);
    }
    let sanitized_context_payload = sanitize_context_payload(&context_payload);
    if sanitized_context_payload != context_payload {
        warn!(
            user_id = %user_id,
            "assistant session memory context sanitized by safety policy"
        );
    }
    context_payload = sanitized_context_payload;

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
    audit_metadata.insert("assistant_session_id".to_string(), session_id.to_string());
    if let Some(record) = session_record {
        audit_metadata.insert(
            "assistant_session_turn_count".to_string(),
            record.turn_count.to_string(),
        );
        audit_metadata.insert(
            "assistant_session_expires_at".to_string(),
            record.expires_at.to_rfc3339(),
        );
    }
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

    let updated_memory = build_updated_memory(
        session_record.map(|record| &record.memory),
        query,
        &payload.summary,
        AssistantQueryCapability::MeetingsToday,
        now,
    );

    let session_persisted = match state
        .store
        .upsert_assistant_session(
            user_id,
            session_id,
            AssistantQueryCapability::MeetingsToday,
            &updated_memory,
            now,
            ASSISTANT_SESSION_TTL_SECONDS,
        )
        .await
    {
        Ok(()) => true,
        Err(err) => {
            warn!(
                user_id = %user_id,
                session_id = %session_id,
                "failed to persist assistant session memory: {err}"
            );
            false
        }
    };
    audit_metadata.insert(
        "assistant_session_persisted".to_string(),
        session_persisted.to_string(),
    );

    let audit_result =
        if telemetry.outcome == "success" && output_source == "model_output" && session_persisted {
            AuditResult::Success
        } else {
            AuditResult::Failure
        };
    record_ai_audit_event(state, user_id, request_id, audit_result, &audit_metadata).await;

    (
        StatusCode::OK,
        Json(AssistantQueryResponse {
            session_id,
            capability: AssistantQueryCapability::MeetingsToday,
            display_text: payload.summary.clone(),
            payload,
        }),
    )
        .into_response()
}
