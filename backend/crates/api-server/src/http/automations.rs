use std::collections::HashMap;

use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use base64::Engine as _;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use shared::assistant_crypto::{
    ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305, ASSISTANT_ENVELOPE_VERSION_V1,
};
use shared::automation_schedule::{
    AutomationScheduleSpec, build_schedule_spec, format_local_time_hhmm, next_run_after,
    parse_local_time_hhmm,
};
use shared::models::{
    AutomationRuleSummary, AutomationSchedule, AutomationStatus, CreateAutomationRequest,
    ErrorBody, ErrorResponse, ListAutomationsResponse, OkResponse,
    TriggerAutomationDebugRunResponse, UpdateAutomationRequest,
};
use shared::repos::{
    AuditResult, AutomationRuleRecord, AutomationRuleStatus as RepoAutomationRuleStatus, JobType,
    StoreError,
};
use uuid::Uuid;

use super::errors::{bad_request_response, store_error_response};
use super::{AppState, AuthUser};

const AUTOMATION_LIST_DEFAULT_LIMIT: i64 = 50;
const AUTOMATION_LIST_MAX_LIMIT: i64 = 200;
const MAX_PROMPT_ENVELOPE_CIPHERTEXT_BYTES: usize = 65_536;
type PromptValidationError = (&'static str, &'static str);
type ScheduleValidationError = (&'static str, &'static str);

#[derive(Debug, Deserialize)]
pub(super) struct ListAutomationsQuery {
    pub(super) limit: Option<i64>,
}

#[derive(Debug, Serialize)]
struct AutomationRunJobPayload {
    automation_run_id: Uuid,
    automation_rule_id: Uuid,
    scheduled_for: DateTime<Utc>,
    prompt_sha256: String,
    prompt_envelope_ciphertext_b64: String,
}

pub(super) async fn create_automation(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Json(request): Json<CreateAutomationRequest>,
) -> Response {
    let prompt_payload = match validated_prompt_payload(&request.prompt_envelope) {
        Ok(payload) => payload,
        Err((code, message)) => return bad_request_response(code, message),
    };
    let now = Utc::now();
    let (schedule, next_run_at) = match validated_schedule_and_next_run(&request.schedule, now) {
        Ok(value) => value,
        Err((code, message)) => return bad_request_response(code, message),
    };
    let prompt_sha256 = format!("{:x}", Sha256::digest(&prompt_payload));

    let created_rule = match state
        .store
        .create_automation_rule(
            user.user_id,
            &schedule,
            next_run_at,
            &prompt_payload,
            &prompt_sha256,
        )
        .await
    {
        Ok(rule) => rule,
        Err(err) => return automation_store_error_response(err),
    };

    let mut metadata = HashMap::new();
    metadata.insert("rule_id".to_string(), created_rule.id.to_string());
    metadata.insert(
        "schedule_type".to_string(),
        created_rule.schedule_type.as_str().to_string(),
    );
    metadata.insert("time_zone".to_string(), created_rule.time_zone.clone());
    metadata.insert(
        "local_time".to_string(),
        format_local_time_hhmm(u16::try_from(created_rule.local_time_minutes).unwrap_or(0)),
    );
    if let Err(err) = state
        .store
        .add_audit_event(
            user.user_id,
            "AUTOMATION_RULE_CREATED",
            None,
            AuditResult::Success,
            &metadata,
        )
        .await
    {
        return store_error_response(err);
    }

    (StatusCode::OK, Json(automation_rule_summary(created_rule))).into_response()
}

pub(super) async fn list_automations(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Query(query): Query<ListAutomationsQuery>,
) -> Response {
    let limit = query.limit.unwrap_or(AUTOMATION_LIST_DEFAULT_LIMIT);
    if !(1..=AUTOMATION_LIST_MAX_LIMIT).contains(&limit) {
        return bad_request_response("invalid_limit", "limit must be between 1 and 200");
    }

    let rules = match state.store.list_automation_rules(user.user_id, limit).await {
        Ok(rules) => rules,
        Err(err) => return automation_store_error_response(err),
    };

    let items = rules.into_iter().map(automation_rule_summary).collect();
    (StatusCode::OK, Json(ListAutomationsResponse { items })).into_response()
}

pub(super) async fn update_automation(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(rule_id): Path<String>,
    Json(request): Json<UpdateAutomationRequest>,
) -> Response {
    let rule_id = match Uuid::parse_str(&rule_id) {
        Ok(rule_id) => rule_id,
        Err(_) => return automation_not_found_response(),
    };

    if request.schedule.is_none() && request.prompt_envelope.is_none() && request.status.is_none() {
        return bad_request_response(
            "invalid_automation_update",
            "Provide at least one update field: schedule, prompt_envelope, or status",
        );
    }

    let mut rule = match state.store.get_automation_rule(user.user_id, rule_id).await {
        Ok(Some(rule)) => rule,
        Ok(None) => return automation_not_found_response(),
        Err(err) => return automation_store_error_response(err),
    };

    let mut changed_fields: Vec<&str> = Vec::new();

    if let Some(schedule_update) = request.schedule {
        let now = Utc::now();
        let (schedule, next_run_at) = match validated_schedule_and_next_run(&schedule_update, now) {
            Ok(value) => value,
            Err((code, message)) => return bad_request_response(code, message),
        };

        rule = match state
            .store
            .update_automation_rule_schedule(user.user_id, rule_id, &schedule, next_run_at)
            .await
        {
            Ok(Some(rule)) => rule,
            Ok(None) => return automation_not_found_response(),
            Err(err) => return automation_store_error_response(err),
        };
        changed_fields.push("schedule");
    }

    if let Some(prompt_envelope) = request.prompt_envelope {
        let prompt_payload = match validated_prompt_payload(&prompt_envelope) {
            Ok(payload) => payload,
            Err((code, message)) => return bad_request_response(code, message),
        };
        let prompt_sha256 = format!("{:x}", Sha256::digest(&prompt_payload));
        rule = match state
            .store
            .update_automation_rule_prompt(user.user_id, rule_id, &prompt_payload, &prompt_sha256)
            .await
        {
            Ok(Some(rule)) => rule,
            Ok(None) => return automation_not_found_response(),
            Err(err) => return automation_store_error_response(err),
        };
        changed_fields.push("prompt");
    }

    if let Some(status) = request.status {
        match status {
            AutomationStatus::Paused => {
                match state
                    .store
                    .pause_automation_rule(user.user_id, rule_id)
                    .await
                {
                    Ok(true) => {}
                    Ok(false) => return automation_not_found_response(),
                    Err(err) => return automation_store_error_response(err),
                }
                changed_fields.push("status");
            }
            AutomationStatus::Active => {
                let schedule = match rule.schedule_spec() {
                    Ok(schedule) => schedule,
                    Err(err) => return automation_store_error_response(err),
                };
                let Some(next_run_at) = next_run_after(Utc::now(), &schedule) else {
                    return bad_request_response(
                        "invalid_schedule",
                        "unable to compute next run for automation schedule",
                    );
                };

                match state
                    .store
                    .resume_automation_rule(user.user_id, rule_id, next_run_at)
                    .await
                {
                    Ok(true) => {}
                    Ok(false) => return automation_not_found_response(),
                    Err(err) => return automation_store_error_response(err),
                }
                changed_fields.push("status");
            }
        }

        rule = match state.store.get_automation_rule(user.user_id, rule_id).await {
            Ok(Some(rule)) => rule,
            Ok(None) => return automation_not_found_response(),
            Err(err) => return automation_store_error_response(err),
        };
    }

    if !changed_fields.is_empty() {
        let mut metadata = HashMap::new();
        metadata.insert("rule_id".to_string(), rule.id.to_string());
        metadata.insert("updated_fields".to_string(), changed_fields.join(","));
        if let Err(err) = state
            .store
            .add_audit_event(
                user.user_id,
                "AUTOMATION_RULE_UPDATED",
                None,
                AuditResult::Success,
                &metadata,
            )
            .await
        {
            return store_error_response(err);
        }
    }

    (StatusCode::OK, Json(automation_rule_summary(rule))).into_response()
}

pub(super) async fn delete_automation(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(rule_id): Path<String>,
) -> Response {
    let rule_id = match Uuid::parse_str(&rule_id) {
        Ok(rule_id) => rule_id,
        Err(_) => return automation_not_found_response(),
    };

    match state
        .store
        .delete_automation_rule(user.user_id, rule_id)
        .await
    {
        Ok(true) => {}
        Ok(false) => return automation_not_found_response(),
        Err(err) => return automation_store_error_response(err),
    }

    let mut metadata = HashMap::new();
    metadata.insert("rule_id".to_string(), rule_id.to_string());
    if let Err(err) = state
        .store
        .add_audit_event(
            user.user_id,
            "AUTOMATION_RULE_DELETED",
            None,
            AuditResult::Success,
            &metadata,
        )
        .await
    {
        return store_error_response(err);
    }

    (StatusCode::OK, Json(OkResponse { ok: true })).into_response()
}

pub(super) async fn trigger_debug_run(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(rule_id): Path<String>,
) -> Response {
    if !state.allow_debug_automation_run {
        return automation_not_found_response();
    }

    let rule_id = match Uuid::parse_str(&rule_id) {
        Ok(rule_id) => rule_id,
        Err(_) => return automation_not_found_response(),
    };

    let Some(rule) = (match state.store.get_automation_rule(user.user_id, rule_id).await {
        Ok(rule) => rule,
        Err(err) => return automation_store_error_response(err),
    }) else {
        return automation_not_found_response();
    };

    if !matches!(rule.status, RepoAutomationRuleStatus::Active) {
        return bad_request_response(
            "automation_not_active",
            "Automation rule must be ACTIVE to trigger a debug run",
        );
    }

    let Some(prompt_material) = (match state
        .store
        .get_automation_rule_prompt_material(user.user_id, rule_id)
        .await
    {
        Ok(material) => material,
        Err(err) => return automation_store_error_response(err),
    }) else {
        return automation_not_found_response();
    };

    let scheduled_for = Utc::now();
    let automation_run_id = Uuid::new_v4();
    let payload = AutomationRunJobPayload {
        automation_run_id,
        automation_rule_id: rule_id,
        scheduled_for,
        prompt_sha256: prompt_material.prompt_sha256,
        prompt_envelope_ciphertext_b64: base64::engine::general_purpose::STANDARD
            .encode(prompt_material.prompt_ciphertext),
    };
    let payload_json = match serde_json::to_vec(&payload) {
        Ok(payload_json) => payload_json,
        Err(_) => {
            return bad_request_response(
                "invalid_automation_payload",
                "failed to serialize automation run payload",
            );
        }
    };

    let idempotency_key = format!("AUTOMATION_DEBUG_RUN:{rule_id}:{automation_run_id}");
    let job_id = match state
        .store
        .enqueue_job_with_idempotency_key(
            user.user_id,
            JobType::AutomationRun,
            scheduled_for,
            Some(&payload_json),
            &idempotency_key,
        )
        .await
    {
        Ok(job_id) => job_id,
        Err(err) => return automation_store_error_response(err),
    };

    let mut metadata = HashMap::new();
    metadata.insert("rule_id".to_string(), rule_id.to_string());
    metadata.insert("job_id".to_string(), job_id.to_string());
    metadata.insert(
        "job_type".to_string(),
        JobType::AutomationRun.as_str().to_string(),
    );
    metadata.insert("mode".to_string(), "DEBUG_MANUAL".to_string());

    if let Err(err) = state
        .store
        .add_audit_event(
            user.user_id,
            "AUTOMATION_DEBUG_RUN_QUEUED",
            None,
            AuditResult::Success,
            &metadata,
        )
        .await
    {
        return store_error_response(err);
    }

    (
        StatusCode::OK,
        Json(TriggerAutomationDebugRunResponse {
            queued_job_id: job_id.to_string(),
            status: "QUEUED".to_string(),
        }),
    )
        .into_response()
}

fn validated_schedule_and_next_run(
    schedule: &AutomationSchedule,
    reference_utc: DateTime<Utc>,
) -> Result<(AutomationScheduleSpec, DateTime<Utc>), ScheduleValidationError> {
    let local_time_minutes = parse_local_time_hhmm(schedule.local_time.as_str()).ok_or((
        "invalid_local_time",
        "local_time must use HH:MM 24-hour format",
    ))?;

    let schedule_spec = build_schedule_spec(
        schedule.schedule_type,
        schedule.time_zone.as_str(),
        local_time_minutes,
        reference_utc,
    )
    .map_err(|_| {
        (
            "invalid_schedule",
            "schedule contains invalid frequency/time/time_zone values",
        )
    })?;

    let next_run_at = next_run_after(reference_utc, &schedule_spec).ok_or((
        "invalid_schedule",
        "unable to compute next run for schedule",
    ))?;

    Ok((schedule_spec, next_run_at))
}

fn validated_prompt_payload(
    envelope: &shared::models::AutomationPromptEnvelope,
) -> Result<Vec<u8>, PromptValidationError> {
    if envelope.version != ASSISTANT_ENVELOPE_VERSION_V1 {
        return Err((
            "invalid_envelope_version",
            "automation prompt envelope version is not supported",
        ));
    }

    if envelope.algorithm != ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305 {
        return Err((
            "invalid_envelope_algorithm",
            "automation prompt envelope algorithm is not supported",
        ));
    }

    if envelope.key_id.trim().is_empty() {
        return Err(("invalid_key_id", "key_id is required"));
    }

    if envelope.request_id.trim().is_empty() {
        return Err(("invalid_request_id", "request_id is required"));
    }

    let client_public_key = match base64::engine::general_purpose::STANDARD
        .decode(envelope.client_ephemeral_public_key.as_bytes())
    {
        Ok(bytes) => bytes,
        Err(_) => {
            return Err((
                "invalid_client_public_key",
                "client_ephemeral_public_key must be valid base64",
            ));
        }
    };
    if client_public_key.len() != 32 {
        return Err((
            "invalid_client_public_key",
            "client_ephemeral_public_key must decode to 32 bytes",
        ));
    }

    let nonce = match base64::engine::general_purpose::STANDARD.decode(envelope.nonce.as_bytes()) {
        Ok(bytes) => bytes,
        Err(_) => return Err(("invalid_nonce", "nonce must be valid base64")),
    };
    if nonce.len() != 12 {
        return Err(("invalid_nonce", "nonce must decode to 12 bytes"));
    }

    let ciphertext =
        match base64::engine::general_purpose::STANDARD.decode(envelope.ciphertext.as_bytes()) {
            Ok(ciphertext) => ciphertext,
            Err(_) => {
                return Err(("invalid_ciphertext", "ciphertext must be valid base64"));
            }
        };

    if ciphertext.is_empty() {
        return Err(("invalid_ciphertext", "ciphertext must not be empty"));
    }

    if ciphertext.len() > MAX_PROMPT_ENVELOPE_CIPHERTEXT_BYTES {
        return Err(("invalid_ciphertext", "ciphertext exceeds size limit"));
    }

    serde_json::to_vec(envelope).map_err(|_| {
        (
            "invalid_prompt_envelope",
            "automation prompt envelope payload is invalid",
        )
    })
}

fn automation_rule_summary(rule: AutomationRuleRecord) -> AutomationRuleSummary {
    let status = match rule.status {
        RepoAutomationRuleStatus::Active => AutomationStatus::Active,
        RepoAutomationRuleStatus::Paused => AutomationStatus::Paused,
    };

    let local_time = u16::try_from(rule.local_time_minutes)
        .ok()
        .map(format_local_time_hhmm)
        .unwrap_or_else(|| "00:00".to_string());

    AutomationRuleSummary {
        rule_id: rule.id.to_string(),
        status,
        schedule: AutomationSchedule {
            schedule_type: rule.schedule_type,
            time_zone: rule.time_zone,
            local_time,
        },
        next_run_at: rule.next_run_at,
        last_run_at: rule.last_run_at,
        prompt_sha256: rule.prompt_sha256,
        created_at: rule.created_at,
        updated_at: rule.updated_at,
    }
}

fn automation_store_error_response(err: StoreError) -> Response {
    match err {
        StoreError::InvalidData(message) => {
            bad_request_response("invalid_automation_request", &message)
        }
        other => store_error_response(other),
    }
}

fn automation_not_found_response() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: ErrorBody {
                code: "not_found".to_string(),
                message: "Automation rule not found".to_string(),
            },
        }),
    )
        .into_response()
}
