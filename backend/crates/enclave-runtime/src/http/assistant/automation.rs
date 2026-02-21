use std::collections::HashMap;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use base64::Engine as _;
use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{ChaCha20Poly1305, KeyInit, Nonce};
use serde::Serialize;
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
use shared::models::AssistantQueryCapability;
use tracing::warn;
use x25519_dalek::{PublicKey, StaticSecret};

use super::orchestrator::AssistantOrchestratorResult;
use crate::RuntimeState;
use crate::http::rpc;

const AUTOMATION_NOTIFICATION_TITLE_MAX_CHARS: usize = 64;
const AUTOMATION_NOTIFICATION_BODY_MAX_CHARS: usize = 180;
const AUTOMATION_PROMPT_MAX_CHARS: usize = 4_000;
const AUTOMATION_NOTIFICATION_DEFAULT_TITLE: &str = "Task update";
const AUTOMATION_NOTIFICATION_DEFAULT_BODY: &str = "Your scheduled task ran.";

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AutomationNotificationSource {
    OrchestratorResult,
    DeterministicFallback,
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
    let execution = match super::orchestrator::execute_query(
        &state,
        request.user_id,
        request.request_id.as_str(),
        prompt_query.as_str(),
        None,
    )
    .await
    {
        Ok(execution) => execution,
        Err(response) => {
            warn!(
                user_id = %request.user_id,
                status = response.status().as_u16(),
                "automation orchestrator execution failed"
            );
            return response;
        }
    };
    let (notification, output_source) = resolve_notification_content(&execution);

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
        "enclave_automation_orchestrator".to_string(),
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
            AutomationNotificationSource::OrchestratorResult => "orchestrator_result",
            AutomationNotificationSource::DeterministicFallback => "deterministic_fallback",
        }
        .to_string(),
    );
    metadata.insert(
        "llm_capability".to_string(),
        capability_label(&execution.capability).to_string(),
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
    execution: &AssistantOrchestratorResult,
) -> (NotificationContent, AutomationNotificationSource) {
    let title = notification_candidate(execution.payload.title.as_str())
        .map(|value| {
            truncate_for_notification(value.as_str(), AUTOMATION_NOTIFICATION_TITLE_MAX_CHARS)
        })
        .unwrap_or_else(|| {
            truncate_for_notification(
                default_title_for_capability(&execution.capability),
                AUTOMATION_NOTIFICATION_TITLE_MAX_CHARS,
            )
        });

    let body = notification_candidate(execution.display_text.as_str())
        .or_else(|| notification_candidate(execution.payload.summary.as_str()))
        .or_else(|| {
            execution
                .payload
                .key_points
                .iter()
                .find_map(|item| notification_candidate(item.as_str()))
        });

    match body {
        Some(body) => (
            NotificationContent {
                title,
                body: truncate_for_notification(
                    body.as_str(),
                    AUTOMATION_NOTIFICATION_BODY_MAX_CHARS,
                ),
            },
            AutomationNotificationSource::OrchestratorResult,
        ),
        None => (
            NotificationContent {
                title,
                body: AUTOMATION_NOTIFICATION_DEFAULT_BODY.to_string(),
            },
            AutomationNotificationSource::DeterministicFallback,
        ),
    }
}

fn notification_candidate(value: &str) -> Option<String> {
    let collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    non_empty(collapsed.as_str()).map(ToString::to_string)
}

fn capability_label(capability: &AssistantQueryCapability) -> &'static str {
    match capability {
        AssistantQueryCapability::MeetingsToday => "meetings_today",
        AssistantQueryCapability::CalendarLookup => "calendar_lookup",
        AssistantQueryCapability::EmailLookup => "email_lookup",
        AssistantQueryCapability::GeneralChat => "general_chat",
        AssistantQueryCapability::Mixed => "mixed",
    }
}

fn default_title_for_capability(capability: &AssistantQueryCapability) -> &'static str {
    match capability {
        AssistantQueryCapability::MeetingsToday | AssistantQueryCapability::CalendarLookup => {
            "Calendar update"
        }
        AssistantQueryCapability::EmailLookup => "Email update",
        AssistantQueryCapability::GeneralChat | AssistantQueryCapability::Mixed => {
            AUTOMATION_NOTIFICATION_DEFAULT_TITLE
        }
    }
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
    use shared::enclave::AttestedIdentityPayload;
    use shared::models::{AssistantResponsePart, AssistantStructuredPayload};

    use super::*;

    #[test]
    fn resolve_notification_content_uses_orchestrator_result_when_present() {
        let execution = AssistantOrchestratorResult {
            capability: AssistantQueryCapability::CalendarLookup,
            display_text: "You have three meetings today.".to_string(),
            payload: AssistantStructuredPayload {
                title: "Today's calendar".to_string(),
                summary: "You have three meetings today.".to_string(),
                key_points: Vec::new(),
                follow_ups: Vec::new(),
            },
            response_parts: vec![AssistantResponsePart::chat_text(
                "You have three meetings today.".to_string(),
            )],
            attested_identity: AttestedIdentityPayload {
                runtime: "test-runtime".to_string(),
                measurement: "test-measurement".to_string(),
            },
        };

        let (notification, source) = resolve_notification_content(&execution);
        assert_eq!(notification.title, "Today's calendar");
        assert_eq!(notification.body, "You have three meetings today.");
        assert!(matches!(
            source,
            AutomationNotificationSource::OrchestratorResult
        ));
    }

    #[test]
    fn resolve_notification_content_falls_back_when_orchestrator_fields_are_empty() {
        let execution = AssistantOrchestratorResult {
            capability: AssistantQueryCapability::GeneralChat,
            display_text: "   ".to_string(),
            payload: AssistantStructuredPayload {
                title: "   ".to_string(),
                summary: "   ".to_string(),
                key_points: Vec::new(),
                follow_ups: Vec::new(),
            },
            response_parts: Vec::new(),
            attested_identity: AttestedIdentityPayload {
                runtime: "test-runtime".to_string(),
                measurement: "test-measurement".to_string(),
            },
        };

        let (notification, source) = resolve_notification_content(&execution);
        assert_eq!(notification.title, "Task update");
        assert_eq!(notification.body, "Your scheduled task ran.");
        assert!(matches!(
            source,
            AutomationNotificationSource::DeterministicFallback
        ));
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
