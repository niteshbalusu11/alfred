use reqwest::StatusCode;
use serde::Serialize;
use serde_json::{Value, json};
use shared::assistant_crypto::ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305;
use shared::enclave::EncryptedAutomationNotificationEnvelope;
use shared::models::ApnsEnvironment;
use shared::repos::DeviceRegistration;
use tracing::info;

use crate::{FailureClass, JobExecutionError};

#[derive(Clone)]
pub(crate) struct PushSender {
    client: reqwest::Client,
    sandbox_endpoint: Option<String>,
    production_endpoint: Option<String>,
    auth_token: Option<String>,
}

#[derive(Debug)]
pub(crate) enum PushSendError {
    Transient { code: String, message: String },
    Permanent { code: String, message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PushPayloadMode {
    Encrypted,
    Fallback,
}

impl PushPayloadMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Encrypted => "encrypted",
            Self::Fallback => "fallback",
        }
    }
}

impl PushSendError {
    pub(crate) fn to_job_error(&self) -> JobExecutionError {
        match self {
            Self::Transient { code, message } => {
                JobExecutionError::transient(code.clone(), message.clone())
            }
            Self::Permanent { code, message } => {
                JobExecutionError::permanent(code.clone(), message.clone())
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct NotificationContent {
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) encrypted_envelope: Option<EncryptedAutomationNotificationEnvelope>,
}

impl NotificationContent {
    pub(crate) fn automation_fallback() -> Self {
        Self {
            title: "Automation update".to_string(),
            body: "Open Alfred to view your latest automation result.".to_string(),
            encrypted_envelope: None,
        }
    }
}

#[derive(Debug, Serialize)]
struct PushDeliveryRequest<'a> {
    device_token: &'a str,
    title: &'a str,
    body: &'a str,
    payload: Value,
}

const APNS_MAX_PAYLOAD_BYTES: usize = 4096;

impl PushSender {
    pub(crate) fn new(
        sandbox_endpoint: Option<String>,
        production_endpoint: Option<String>,
        auth_token: Option<String>,
    ) -> Self {
        Self {
            client: reqwest::Client::new(),
            sandbox_endpoint,
            production_endpoint,
            auth_token,
        }
    }

    pub(crate) async fn send(
        &self,
        device: &DeviceRegistration,
        content: &NotificationContent,
    ) -> Result<PushPayloadMode, PushSendError> {
        let endpoint = match device.environment {
            ApnsEnvironment::Sandbox => self.sandbox_endpoint.as_deref(),
            ApnsEnvironment::Production => self.production_endpoint.as_deref(),
        };

        let Some(endpoint) = endpoint else {
            info!(
                device_id = %device.device_id,
                environment = %apns_environment_label(&device.environment),
                "apns endpoint not configured for environment; simulated delivery"
            );
            return Ok(PushPayloadMode::Fallback);
        };

        let payload = apns_payload(content)?;
        let payload_mode = if payload
            .as_object()
            .is_some_and(|object| object.contains_key("alfred_automation"))
        {
            PushPayloadMode::Encrypted
        } else {
            PushPayloadMode::Fallback
        };

        let request = PushDeliveryRequest {
            device_token: &device.apns_token,
            title: &content.title,
            body: &content.body,
            payload,
        };

        let mut builder = self.client.post(endpoint).json(&request);
        if let Some(auth_token) = self.auth_token.as_deref() {
            builder = builder.bearer_auth(auth_token);
        }

        let response = builder
            .send()
            .await
            .map_err(|err| PushSendError::Transient {
                code: "APNS_NETWORK_ERROR".to_string(),
                message: format!("APNs request failed: {err}"),
            })?;

        let status = response.status();
        if status.is_success() {
            return Ok(payload_mode);
        }

        let body = response.text().await.unwrap_or_default();
        let code = format!("APNS_HTTP_{}", status.as_u16());
        let message = if body.is_empty() {
            format!("APNs responded with status {status}")
        } else {
            format!("APNs responded with status {status}: {body}")
        };

        match classify_http_failure(status) {
            FailureClass::Transient => Err(PushSendError::Transient { code, message }),
            FailureClass::Permanent => Err(PushSendError::Permanent { code, message }),
        }
    }
}

fn apns_payload(content: &NotificationContent) -> Result<Value, PushSendError> {
    let mut payload = json!({
        "aps": {
            "alert": {
                "title": content.title,
                "body": content.body
            }
        }
    });

    if let Some(envelope) = content
        .encrypted_envelope
        .as_ref()
        .filter(|value| is_valid_encrypted_envelope(value))
    {
        payload["aps"]["mutable-content"] = json!(1);
        payload["alfred_automation"] = json!({
            "version": envelope.version,
            "envelope": {
                "version": envelope.version,
                "algorithm": envelope.algorithm,
                "key_id": envelope.key_id,
                "request_id": envelope.request_id,
                "sender_public_key": envelope.sender_public_key,
                "nonce": envelope.nonce,
                "ciphertext": envelope.ciphertext
            }
        });
    }

    enforce_apns_payload_size(&mut payload)?;
    Ok(payload)
}

fn is_valid_encrypted_envelope(envelope: &EncryptedAutomationNotificationEnvelope) -> bool {
    envelope.algorithm == ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305
        && !envelope.version.trim().is_empty()
        && !envelope.key_id.trim().is_empty()
        && !envelope.request_id.trim().is_empty()
        && !envelope.sender_public_key.trim().is_empty()
        && !envelope.nonce.trim().is_empty()
        && !envelope.ciphertext.trim().is_empty()
}

fn enforce_apns_payload_size(payload: &mut Value) -> Result<(), PushSendError> {
    let payload_size = payload_size_bytes(payload)?;
    if payload_size <= APNS_MAX_PAYLOAD_BYTES {
        return Ok(());
    }

    if payload
        .as_object()
        .is_some_and(|object| object.contains_key("alfred_automation"))
    {
        drop_encrypted_payload(payload);
        if payload_size_bytes(payload)? <= APNS_MAX_PAYLOAD_BYTES {
            return Ok(());
        }
    }

    Err(PushSendError::Permanent {
        code: "APNS_PAYLOAD_TOO_LARGE".to_string(),
        message: format!(
            "APNs payload exceeds {APNS_MAX_PAYLOAD_BYTES} byte limit after fallback handling"
        ),
    })
}

fn payload_size_bytes(payload: &Value) -> Result<usize, PushSendError> {
    serde_json::to_vec(payload)
        .map(|bytes| bytes.len())
        .map_err(|_| PushSendError::Permanent {
            code: "APNS_PAYLOAD_INVALID".to_string(),
            message: "failed to serialize APNs payload".to_string(),
        })
}

fn drop_encrypted_payload(payload: &mut Value) {
    let Some(object) = payload.as_object_mut() else {
        return;
    };
    object.remove("alfred_automation");
    if let Some(aps) = object.get_mut("aps").and_then(Value::as_object_mut) {
        aps.remove("mutable-content");
    }
}

pub(crate) fn apns_environment_label(environment: &ApnsEnvironment) -> &'static str {
    match environment {
        ApnsEnvironment::Sandbox => "sandbox",
        ApnsEnvironment::Production => "production",
    }
}

fn classify_http_failure(status: StatusCode) -> FailureClass {
    match status.as_u16() {
        408 | 425 | 429 | 500 | 502 | 503 | 504 => FailureClass::Transient,
        _ => FailureClass::Permanent,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use shared::assistant_crypto::ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305;
    use shared::enclave::EncryptedAutomationNotificationEnvelope;

    use super::NotificationContent;
    use reqwest::StatusCode;

    use super::{
        apns_payload, classify_http_failure, enforce_apns_payload_size, is_valid_encrypted_envelope,
    };
    use crate::FailureClass;

    #[test]
    fn classifies_retryable_http_status_codes_as_transient() {
        assert!(matches!(
            classify_http_failure(StatusCode::TOO_MANY_REQUESTS),
            FailureClass::Transient
        ));
        assert!(matches!(
            classify_http_failure(StatusCode::SERVICE_UNAVAILABLE),
            FailureClass::Transient
        ));
    }

    #[test]
    fn classifies_client_errors_as_permanent() {
        assert!(matches!(
            classify_http_failure(StatusCode::BAD_REQUEST),
            FailureClass::Permanent
        ));
        assert!(matches!(
            classify_http_failure(StatusCode::GONE),
            FailureClass::Permanent
        ));
    }

    #[test]
    fn apns_payload_includes_envelope_and_mutable_content_when_valid() {
        let content = NotificationContent {
            title: "Automation update".to_string(),
            body: "Open Alfred to view your latest automation result.".to_string(),
            encrypted_envelope: Some(sample_envelope()),
        };

        let payload = apns_payload(&content).expect("payload should serialize");
        assert_eq!(payload["aps"]["mutable-content"], json!(1));
        assert_eq!(
            payload["alfred_automation"]["envelope"]["algorithm"],
            json!(ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305)
        );
    }

    #[test]
    fn apns_payload_drops_invalid_envelope() {
        let mut invalid_envelope = sample_envelope();
        invalid_envelope.algorithm = "invalid".to_string();
        assert!(!is_valid_encrypted_envelope(&invalid_envelope));

        let content = NotificationContent {
            title: "Automation update".to_string(),
            body: "Open Alfred to view your latest automation result.".to_string(),
            encrypted_envelope: Some(invalid_envelope),
        };
        let payload = apns_payload(&content).expect("payload should serialize");

        assert!(payload.get("alfred_automation").is_none());
        assert!(payload["aps"].get("mutable-content").is_none());
    }

    #[test]
    fn payload_size_guard_drops_encrypted_payload_before_failing() {
        let mut envelope = sample_envelope();
        envelope.ciphertext = "A".repeat(7000);

        let mut payload = json!({
            "aps": {
                "alert": {
                    "title": "Automation update",
                    "body": "Open Alfred to view your latest automation result."
                },
                "mutable-content": 1
            },
            "alfred_automation": {
                "version": "v1",
                "envelope": {
                    "version": envelope.version,
                    "algorithm": envelope.algorithm,
                    "key_id": envelope.key_id,
                    "request_id": envelope.request_id,
                    "sender_public_key": envelope.sender_public_key,
                    "nonce": envelope.nonce,
                    "ciphertext": envelope.ciphertext
                }
            }
        });

        enforce_apns_payload_size(&mut payload).expect("fallback payload should fit");
        assert!(payload.get("alfred_automation").is_none());
        assert!(payload["aps"].get("mutable-content").is_none());
    }

    #[test]
    fn payload_size_guard_rejects_oversized_fallback_payload() {
        let oversized_text = "B".repeat(5000);
        let mut payload = json!({
            "aps": {
                "alert": {
                    "title": oversized_text,
                    "body": "still too large"
                }
            }
        });

        let err = enforce_apns_payload_size(&mut payload).expect_err("payload should be rejected");
        assert!(matches!(
            err,
            super::PushSendError::Permanent { code, .. } if code == "APNS_PAYLOAD_TOO_LARGE"
        ));
    }

    fn sample_envelope() -> EncryptedAutomationNotificationEnvelope {
        EncryptedAutomationNotificationEnvelope {
            version: "v1".to_string(),
            algorithm: ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305.to_string(),
            key_id: "device-key".to_string(),
            request_id: "req-123".to_string(),
            sender_public_key: "pub".to_string(),
            nonce: "nonce".to_string(),
            ciphertext: "cipher".to_string(),
        }
    }
}
