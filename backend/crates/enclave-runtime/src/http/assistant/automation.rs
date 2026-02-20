use std::collections::HashMap;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use base64::Engine as _;
use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{ChaCha20Poly1305, KeyInit, Nonce};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use shared::assistant_crypto::{
    ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305, ASSISTANT_ENVELOPE_VERSION_V1,
    decrypt_assistant_request,
};
use shared::enclave::{
    AttestedIdentityPayload, ENCLAVE_RPC_CONTRACT_VERSION,
    EnclaveAutomationEncryptedNotificationEnvelope, EnclaveAutomationNotificationArtifact,
    EnclaveAutomationRecipientDevice, EnclaveRpcExecuteAutomationRequest,
    EnclaveRpcExecuteAutomationResponse,
};
use shared::llm::contracts::GeneralChatSummaryOutput;
use shared::llm::{
    AssistantCapability, AssistantOutputContract, LlmExecutionSource, LlmGatewayRequest,
    SafeOutputSource, generate_with_telemetry, resolve_safe_output, sanitize_context_payload,
    template_for_capability,
};
use tracing::warn;
use x25519_dalek::{PublicKey, StaticSecret};

use super::mapping::{append_llm_telemetry_metadata, log_telemetry};
use crate::RuntimeState;
use crate::http::rpc;

const AUTOMATION_NOTIFICATION_TITLE_MAX_CHARS: usize = 64;
const AUTOMATION_NOTIFICATION_BODY_MAX_CHARS: usize = 180;
const AUTOMATION_PROMPT_MAX_CHARS: usize = 4_000;

#[derive(Debug, Clone, Serialize)]
struct AutomationNotificationPlaintext {
    title: String,
    body: String,
}

#[derive(Debug, Clone)]
struct NotificationContent {
    title: String,
    body: String,
}

pub(super) async fn execute_automation(
    state: RuntimeState,
    request: EnclaveRpcExecuteAutomationRequest,
) -> Response {
    let request_id = request.request_id.clone();
    let (prompt_query, decrypted_key_id) = match decrypt_automation_prompt(&state, &request) {
        Ok(result) => result,
        Err(err) => {
            return rpc::reject(
                StatusCode::BAD_REQUEST,
                shared::enclave::EnclaveRpcErrorEnvelope::new(
                    Some(request_id),
                    "invalid_request_payload",
                    err,
                    false,
                ),
            )
            .into_response();
        }
    };

    let raw_context_payload = json!({
        "current_query": prompt_query,
        "automation": {
            "automation_rule_id": request.automation_rule_id,
            "automation_run_id": request.automation_run_id,
            "scheduled_for": request.scheduled_for,
        }
    });
    let context_payload = sanitize_context_payload(&raw_context_payload);

    let llm_request = LlmGatewayRequest::from_template(
        template_for_capability(AssistantCapability::GeneralChatSummary),
        context_payload.clone(),
    )
    .with_requester_id(request.user_id.to_string());

    let (llm_result, telemetry) = generate_with_telemetry(
        state.worker_gateway(),
        LlmExecutionSource::WorkerAutomationRun,
        llm_request,
    )
    .await;
    log_telemetry(request.user_id, &telemetry, "automation_run");

    let model_output = match llm_result {
        Ok(response) => Some(response.output),
        Err(err) => {
            warn!(
                user_id = %request.user_id,
                "automation provider request failed: {err}"
            );
            None
        }
    };
    let (notification, output_source) =
        resolve_notification_content(&context_payload, model_output.as_ref());

    let mut notification_artifacts = Vec::with_capacity(request.recipient_devices.len());
    for device in &request.recipient_devices {
        let artifact =
            match encrypt_for_recipient(&state, request.request_id.as_str(), device, &notification)
            {
                Ok(artifact) => artifact,
                Err(err) => {
                    return rpc::reject(
                        StatusCode::BAD_REQUEST,
                        shared::enclave::EnclaveRpcErrorEnvelope::new(
                            Some(request.request_id),
                            "invalid_request_payload",
                            err,
                            false,
                        ),
                    )
                    .into_response();
                }
            };
        notification_artifacts.push(artifact);
    }

    let mut metadata = HashMap::new();
    metadata.insert(
        "action_source".to_string(),
        "enclave_automation_llm_orchestrator".to_string(),
    );
    metadata.insert(
        "automation_rule_id".to_string(),
        request.automation_rule_id.to_string(),
    );
    metadata.insert(
        "automation_run_id".to_string(),
        request.automation_run_id.to_string(),
    );
    metadata.insert(
        "scheduled_for".to_string(),
        request.scheduled_for.to_rfc3339(),
    );
    metadata.insert(
        "llm_output_source".to_string(),
        match output_source {
            SafeOutputSource::ModelOutput => "model_output",
            SafeOutputSource::DeterministicFallback => "deterministic_fallback",
        }
        .to_string(),
    );
    metadata.insert("prompt_key_id".to_string(), decrypted_key_id);
    metadata.insert(
        "recipient_device_count".to_string(),
        request.recipient_devices.len().to_string(),
    );
    metadata.insert(
        "encrypted_artifact_count".to_string(),
        notification_artifacts.len().to_string(),
    );
    metadata.insert(
        "attested_measurement".to_string(),
        state.config.measurement.clone(),
    );
    append_llm_telemetry_metadata(&mut metadata, &telemetry);

    let attested_identity = runtime_attested_identity(&state);
    Json(EnclaveRpcExecuteAutomationResponse {
        contract_version: ENCLAVE_RPC_CONTRACT_VERSION.to_string(),
        request_id: request.request_id,
        should_notify: !notification_artifacts.is_empty(),
        notification_artifacts,
        metadata,
        attested_identity,
    })
    .into_response()
}

fn decrypt_automation_prompt(
    state: &RuntimeState,
    request: &EnclaveRpcExecuteAutomationRequest,
) -> Result<(String, String), String> {
    let envelope = shared::models::AssistantEncryptedRequestEnvelope {
        version: request.prompt_envelope.version.clone(),
        algorithm: request.prompt_envelope.algorithm.clone(),
        key_id: request.prompt_envelope.key_id.clone(),
        request_id: request.prompt_envelope.request_id.clone(),
        client_ephemeral_public_key: request.prompt_envelope.client_ephemeral_public_key.clone(),
        nonce: request.prompt_envelope.nonce.clone(),
        ciphertext: request.prompt_envelope.ciphertext.clone(),
    };
    let (plaintext, selected_key) =
        decrypt_assistant_request(&state.config.assistant_ingress_keys, &envelope)
            .map_err(|_| "automation prompt envelope decrypt failed".to_string())?;

    let prompt_query = validate_prompt_query(plaintext.query.as_str())?;
    Ok((prompt_query, selected_key.key_id))
}

fn validate_prompt_query(value: &str) -> Result<String, String> {
    let prompt_query = value.trim();
    if prompt_query.is_empty() {
        return Err("automation prompt must not be empty".to_string());
    }
    if prompt_query.chars().count() > AUTOMATION_PROMPT_MAX_CHARS {
        return Err(format!(
            "automation prompt exceeds maximum length of {AUTOMATION_PROMPT_MAX_CHARS} characters"
        ));
    }
    Ok(prompt_query.to_string())
}

fn resolve_notification_content(
    context_payload: &Value,
    model_output: Option<&Value>,
) -> (NotificationContent, SafeOutputSource) {
    let resolved = resolve_safe_output(
        AssistantCapability::GeneralChatSummary,
        model_output,
        context_payload,
    );
    let AssistantOutputContract::GeneralChatSummary(contract) = resolved.contract else {
        return (
            NotificationContent {
                title: "Automation update".to_string(),
                body: "Your scheduled automation ran.".to_string(),
            },
            SafeOutputSource::DeterministicFallback,
        );
    };

    (
        notification_from_general_chat_output(&contract.output),
        resolved.source,
    )
}

fn notification_from_general_chat_output(output: &GeneralChatSummaryOutput) -> NotificationContent {
    let title = truncate_for_notification(
        non_empty(&output.title).unwrap_or("Automation update"),
        AUTOMATION_NOTIFICATION_TITLE_MAX_CHARS,
    );

    let body = if let Some(summary) = non_empty(&output.summary) {
        truncate_for_notification(summary, AUTOMATION_NOTIFICATION_BODY_MAX_CHARS)
    } else if let Some(key_point) = output
        .key_points
        .iter()
        .find_map(|item| non_empty(item.as_str()))
    {
        truncate_for_notification(key_point, AUTOMATION_NOTIFICATION_BODY_MAX_CHARS)
    } else {
        "Your scheduled automation ran.".to_string()
    };

    NotificationContent { title, body }
}

fn encrypt_for_recipient(
    state: &RuntimeState,
    request_id: &str,
    device: &EnclaveAutomationRecipientDevice,
    notification: &NotificationContent,
) -> Result<EnclaveAutomationNotificationArtifact, String> {
    if device.device_id.trim().is_empty() {
        return Err("recipient device_id is required".to_string());
    }
    if device.key_id.trim().is_empty() {
        return Err("recipient key_id is required".to_string());
    }
    if device.algorithm != ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305 {
        return Err("recipient key algorithm is not supported".to_string());
    }

    let recipient_public_key = decode_public_key(device.public_key.as_str())?;
    let sender_secret = StaticSecret::from(state.config.assistant_ingress_keys.active.private_key);
    let shared_secret = sender_secret.diffie_hellman(&recipient_public_key);
    let derived_key = derive_notification_key(
        shared_secret.as_bytes(),
        request_id,
        device.device_id.as_str(),
    );

    let plaintext = serde_json::to_vec(&AutomationNotificationPlaintext {
        title: notification.title.clone(),
        body: notification.body.clone(),
    })
    .map_err(|_| "failed to serialize notification payload".to_string())?;

    let nonce_bytes = build_nonce_bytes();
    let aad = format!("{request_id}|{}", device.device_id);
    let cipher = ChaCha20Poly1305::new_from_slice(&derived_key)
        .map_err(|_| "failed to initialize notification cipher".to_string())?;
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce_bytes),
            Payload {
                msg: plaintext.as_slice(),
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| "failed to encrypt notification payload".to_string())?;

    Ok(EnclaveAutomationNotificationArtifact {
        device_id: device.device_id.clone(),
        envelope: EnclaveAutomationEncryptedNotificationEnvelope {
            version: ASSISTANT_ENVELOPE_VERSION_V1.to_string(),
            algorithm: ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305.to_string(),
            key_id: state.config.assistant_ingress_keys.active.key_id.clone(),
            request_id: request_id.to_string(),
            sender_public_key: state
                .config
                .assistant_ingress_keys
                .active
                .public_key
                .clone(),
            nonce: base64::engine::general_purpose::STANDARD.encode(nonce_bytes),
            ciphertext: base64::engine::general_purpose::STANDARD.encode(ciphertext),
        },
    })
}

fn runtime_attested_identity(state: &RuntimeState) -> AttestedIdentityPayload {
    AttestedIdentityPayload {
        runtime: state.config.runtime_id.clone(),
        measurement: state.config.measurement.clone(),
    }
}

fn decode_public_key(value: &str) -> Result<PublicKey, String> {
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(value.as_bytes())
        .map_err(|_| "recipient public_key must be valid base64".to_string())?;
    let key_bytes: [u8; 32] = decoded
        .try_into()
        .map_err(|_| "recipient public_key must decode to 32 bytes".to_string())?;
    Ok(PublicKey::from(key_bytes))
}

fn derive_notification_key(
    shared_secret_bytes: &[u8; 32],
    request_id: &str,
    device_id: &str,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(shared_secret_bytes);
    hasher.update(b"|");
    hasher.update(request_id.as_bytes());
    hasher.update(b"|");
    hasher.update(device_id.as_bytes());
    hasher.update(b"|notification");
    hasher.finalize().into()
}

fn build_nonce_bytes() -> [u8; 12] {
    let uuid_bytes = uuid::Uuid::new_v4();
    let mut nonce = [0_u8; 12];
    nonce.copy_from_slice(&uuid_bytes.as_bytes()[..12]);
    nonce
}

fn truncate_for_notification(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }

    let truncated = trimmed
        .chars()
        .take(max_chars)
        .collect::<String>()
        .trim_end()
        .to_string();
    format!("{truncated}...")
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
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn resolve_notification_content_uses_model_output_when_valid() {
        let context = json!({"current_query":"status update"});
        let output = json!({
            "version":"2026-02-15",
            "output":{
                "title":"Build status",
                "summary":"Everything is healthy.",
                "key_points":[],
                "follow_ups":[],
                "response_style":"conversational"
            }
        });

        let (notification, source) = resolve_notification_content(&context, Some(&output));
        assert_eq!(notification.title, "Build status");
        assert_eq!(notification.body, "Everything is healthy.");
        assert!(matches!(source, SafeOutputSource::ModelOutput));
    }

    #[test]
    fn resolve_notification_content_falls_back_when_model_output_is_invalid() {
        let context = json!({"current_query":"status update"});
        let invalid_output = json!({"version":"invalid"});

        let (notification, source) = resolve_notification_content(&context, Some(&invalid_output));
        assert!(matches!(source, SafeOutputSource::DeterministicFallback));
        assert!(!notification.title.is_empty());
        assert!(!notification.body.is_empty());
    }

    #[test]
    fn decode_public_key_rejects_invalid_bytes() {
        let err = decode_public_key("not-base64").expect_err("public key must reject invalid b64");
        assert!(
            err.contains("valid base64"),
            "unexpected error detail: {err}"
        );
    }

    #[test]
    fn derive_notification_key_is_device_scoped() {
        let shared_secret = [9_u8; 32];
        let first = derive_notification_key(&shared_secret, "req-1", "device-a");
        let second = derive_notification_key(&shared_secret, "req-1", "device-b");
        assert_ne!(first, second);
    }

    #[test]
    fn validate_prompt_query_rejects_empty() {
        let err = validate_prompt_query("   ").expect_err("empty prompt should fail");
        assert_eq!(err, "automation prompt must not be empty");
    }

    #[test]
    fn validate_prompt_query_rejects_oversized_prompt() {
        let oversized = "a".repeat(AUTOMATION_PROMPT_MAX_CHARS + 1);
        let err = validate_prompt_query(oversized.as_str()).expect_err("oversized prompt");
        assert!(
            err.contains("exceeds maximum length"),
            "unexpected error detail: {err}"
        );
    }
}
