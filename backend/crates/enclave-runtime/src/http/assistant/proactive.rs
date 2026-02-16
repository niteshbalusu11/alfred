use std::collections::HashMap;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use serde_json::Value;
use shared::enclave::{
    ENCLAVE_RPC_CONTRACT_VERSION, EnclaveGeneratedNotificationPayload,
    EnclaveRpcGenerateMorningBriefRequest, EnclaveRpcGenerateMorningBriefResponse,
    EnclaveRpcGenerateUrgentEmailSummaryRequest, EnclaveRpcGenerateUrgentEmailSummaryResponse,
};
use shared::llm::{
    AssistantCapability, AssistantOutputContract, LlmExecutionSource, LlmGatewayRequest,
    SafeOutputSource, assemble_morning_brief_context, assemble_urgent_email_candidates_context,
    generate_with_telemetry, resolve_safe_output, sanitize_context_payload,
    template_for_capability,
};
use shared::timezone::{local_day_bounds_utc, user_local_date};
use tracing::warn;

use super::mapping::{
    append_llm_telemetry_metadata, log_telemetry, map_calendar_event_to_meeting_source,
    map_email_candidate_source,
};
use super::notifications::{
    non_empty, notification_from_morning_brief, notification_from_urgent_email, urgency_label,
};
use crate::RuntimeState;
use crate::http::rpc;

const CALENDAR_MAX_RESULTS: usize = 20;
const URGENT_EMAIL_CANDIDATE_MAX_RESULTS: usize = 10;

pub(super) async fn generate_morning_brief(
    state: RuntimeState,
    request: EnclaveRpcGenerateMorningBriefRequest,
) -> Response {
    if request.user_id != request.connector.user_id {
        return rpc::reject(
            StatusCode::BAD_REQUEST,
            shared::enclave::EnclaveRpcErrorEnvelope::new(
                Some(request.request_id),
                "invalid_request_payload",
                "user_id must match connector.user_id",
                false,
            ),
        )
        .into_response();
    }

    let local_date = user_local_date(Utc::now(), &request.time_zone);
    let Some((time_min, time_max)) = local_day_bounds_utc(local_date, &request.time_zone) else {
        return rpc::reject(
            StatusCode::BAD_REQUEST,
            shared::enclave::EnclaveRpcErrorEnvelope::new(
                Some(request.request_id),
                "invalid_request_payload",
                "unable to resolve local-day boundaries for the supplied time zone",
                false,
            ),
        )
        .into_response();
    };

    let calendar_response = match state
        .enclave_service
        .fetch_google_calendar_events(
            request.connector.clone(),
            time_min.to_rfc3339(),
            time_max.to_rfc3339(),
            CALENDAR_MAX_RESULTS,
        )
        .await
    {
        Ok(response) => response,
        Err(err) => {
            return rpc::map_rpc_service_error(err, Some(request.request_id)).into_response();
        }
    };

    let urgent_response = match state
        .enclave_service
        .fetch_google_urgent_email_candidates(request.connector, URGENT_EMAIL_CANDIDATE_MAX_RESULTS)
        .await
    {
        Ok(response) => response,
        Err(err) => {
            return rpc::map_rpc_service_error(err, Some(request.request_id)).into_response();
        }
    };

    let meetings = calendar_response
        .events
        .iter()
        .map(map_calendar_event_to_meeting_source)
        .collect::<Vec<_>>();
    let candidates = urgent_response
        .candidates
        .iter()
        .map(map_email_candidate_source)
        .collect::<Vec<_>>();

    let context = assemble_morning_brief_context(
        local_date,
        &request.morning_brief_local_time,
        &meetings,
        &candidates,
    );
    let raw_context_payload = match serde_json::to_value(&context) {
        Ok(payload) => payload,
        Err(_) => {
            return rpc::reject(
                StatusCode::INTERNAL_SERVER_ERROR,
                shared::enclave::EnclaveRpcErrorEnvelope::new(
                    Some(request.request_id),
                    "rpc_internal_error",
                    "failed to serialize morning brief context",
                    true,
                ),
            )
            .into_response();
        }
    };
    let context_payload = sanitize_context_payload(&raw_context_payload);

    let llm_request = LlmGatewayRequest::from_template(
        template_for_capability(AssistantCapability::MorningBrief),
        context_payload.clone(),
    )
    .with_requester_id(request.user_id.to_string());

    let (llm_result, telemetry) = generate_with_telemetry(
        state.llm_gateway.as_ref(),
        LlmExecutionSource::WorkerMorningBrief,
        llm_request,
    )
    .await;
    log_telemetry(request.user_id, &telemetry, "morning_brief");

    let model_output = match llm_result {
        Ok(response) => response.output,
        Err(err) => {
            warn!(user_id = %request.user_id, "morning brief provider request failed: {err}");
            Value::Null
        }
    };

    let resolved = resolve_safe_output(
        AssistantCapability::MorningBrief,
        if model_output.is_null() {
            None
        } else {
            Some(&model_output)
        },
        &context_payload,
    );

    let AssistantOutputContract::MorningBrief(contract) = resolved.contract else {
        return rpc::reject(
            StatusCode::INTERNAL_SERVER_ERROR,
            shared::enclave::EnclaveRpcErrorEnvelope::new(
                Some(request.request_id),
                "rpc_internal_error",
                "morning brief contract resolution failed",
                true,
            ),
        )
        .into_response();
    };

    let notification = notification_from_morning_brief(&contract.output);
    let mut metadata = HashMap::new();
    metadata.insert(
        "action_source".to_string(),
        "enclave_morning_brief_llm_orchestrator".to_string(),
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
        "llm_output_source".to_string(),
        match resolved.source {
            SafeOutputSource::ModelOutput => "model_output",
            SafeOutputSource::DeterministicFallback => "deterministic_fallback",
        }
        .to_string(),
    );
    metadata.insert(
        "attested_measurement".to_string(),
        calendar_response.attested_identity.measurement.clone(),
    );
    append_llm_telemetry_metadata(&mut metadata, &telemetry);

    Json(EnclaveRpcGenerateMorningBriefResponse {
        contract_version: ENCLAVE_RPC_CONTRACT_VERSION.to_string(),
        request_id: request.request_id,
        notification: EnclaveGeneratedNotificationPayload {
            title: notification.title,
            body: notification.body,
        },
        metadata,
        attested_identity: calendar_response.attested_identity,
    })
    .into_response()
}

pub(super) async fn generate_urgent_email_summary(
    state: RuntimeState,
    request: EnclaveRpcGenerateUrgentEmailSummaryRequest,
) -> Response {
    if request.user_id != request.connector.user_id {
        return rpc::reject(
            StatusCode::BAD_REQUEST,
            shared::enclave::EnclaveRpcErrorEnvelope::new(
                Some(request.request_id),
                "invalid_request_payload",
                "user_id must match connector.user_id",
                false,
            ),
        )
        .into_response();
    }

    let max_results = request
        .max_results
        .clamp(1, URGENT_EMAIL_CANDIDATE_MAX_RESULTS);
    let fetch_response = match state
        .enclave_service
        .fetch_google_urgent_email_candidates(request.connector, max_results)
        .await
    {
        Ok(response) => response,
        Err(err) => {
            return rpc::map_rpc_service_error(err, Some(request.request_id)).into_response();
        }
    };

    let candidates = fetch_response
        .candidates
        .iter()
        .map(map_email_candidate_source)
        .collect::<Vec<_>>();
    let context = assemble_urgent_email_candidates_context(&candidates);
    let raw_context_payload = match serde_json::to_value(&context) {
        Ok(payload) => payload,
        Err(_) => {
            return rpc::reject(
                StatusCode::INTERNAL_SERVER_ERROR,
                shared::enclave::EnclaveRpcErrorEnvelope::new(
                    Some(request.request_id),
                    "rpc_internal_error",
                    "failed to serialize urgent email context",
                    true,
                ),
            )
            .into_response();
        }
    };
    let context_payload = sanitize_context_payload(&raw_context_payload);

    let llm_request = LlmGatewayRequest::from_template(
        template_for_capability(AssistantCapability::UrgentEmailSummary),
        context_payload.clone(),
    )
    .with_requester_id(request.user_id.to_string());

    let (llm_result, telemetry) = generate_with_telemetry(
        state.llm_gateway.as_ref(),
        LlmExecutionSource::WorkerUrgentEmail,
        llm_request,
    )
    .await;
    log_telemetry(request.user_id, &telemetry, "urgent_email");

    let model_output = match llm_result {
        Ok(response) => response.output,
        Err(err) => {
            warn!(user_id = %request.user_id, "urgent email provider request failed: {err}");
            Value::Null
        }
    };

    let resolved = resolve_safe_output(
        AssistantCapability::UrgentEmailSummary,
        if model_output.is_null() {
            None
        } else {
            Some(&model_output)
        },
        &context_payload,
    );

    let AssistantOutputContract::UrgentEmailSummary(contract) = resolved.contract else {
        return rpc::reject(
            StatusCode::INTERNAL_SERVER_ERROR,
            shared::enclave::EnclaveRpcErrorEnvelope::new(
                Some(request.request_id),
                "rpc_internal_error",
                "urgent email contract resolution failed",
                true,
            ),
        )
        .into_response();
    };

    let mut metadata = HashMap::new();
    metadata.insert(
        "action_source".to_string(),
        "enclave_urgent_email_llm_orchestrator".to_string(),
    );
    metadata.insert(
        "email_candidates_in_context".to_string(),
        context.candidate_count.to_string(),
    );
    metadata.insert(
        "llm_output_source".to_string(),
        match resolved.source {
            SafeOutputSource::ModelOutput => "model_output",
            SafeOutputSource::DeterministicFallback => "deterministic_fallback",
        }
        .to_string(),
    );
    metadata.insert(
        "attested_measurement".to_string(),
        fetch_response.attested_identity.measurement.clone(),
    );
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
    append_llm_telemetry_metadata(&mut metadata, &telemetry);

    let notification = if contract.output.should_notify {
        Some(notification_from_urgent_email(&contract.output))
    } else {
        None
    };

    Json(EnclaveRpcGenerateUrgentEmailSummaryResponse {
        contract_version: ENCLAVE_RPC_CONTRACT_VERSION.to_string(),
        request_id: request.request_id,
        should_notify: contract.output.should_notify,
        notification: notification.map(|notification| EnclaveGeneratedNotificationPayload {
            title: notification.title,
            body: notification.body,
        }),
        metadata,
        attested_identity: fetch_response.attested_identity,
    })
    .into_response()
}
