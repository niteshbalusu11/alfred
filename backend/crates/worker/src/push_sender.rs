use chrono::Utc;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use reqwest::StatusCode;
use serde::Serialize;
use serde_json::{Value, json};
use shared::assistant_crypto::ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305;
use shared::enclave::EncryptedAutomationNotificationEnvelope;
use shared::models::ApnsEnvironment;
use shared::repos::DeviceRegistration;

use crate::{FailureClass, JobExecutionError};

const APNS_MAX_PAYLOAD_BYTES: usize = 4096;
const APNS_PRODUCTION_BASE_URL: &str = "https://api.push.apple.com";
const APNS_SANDBOX_BASE_URL: &str = "https://api.sandbox.push.apple.com";

#[derive(Clone)]
pub(crate) struct PushSender {
    client: reqwest::Client,
    key_id: String,
    team_id: String,
    topic: String,
    signing_key: EncodingKey,
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
struct ApnsProviderTokenClaims {
    iss: String,
    iat: i64,
}

impl PushSender {
    pub(crate) fn new(
        key_id: String,
        team_id: String,
        topic: String,
        auth_key_pem: String,
    ) -> Result<Self, String> {
        let signing_key = EncodingKey::from_ec_pem(auth_key_pem.as_bytes())
            .map_err(|err| format!("invalid APNS auth key PEM: {err}"))?;

        Ok(Self {
            client: reqwest::Client::new(),
            key_id,
            team_id,
            topic,
            signing_key,
        })
    }

    pub(crate) async fn send(
        &self,
        device: &DeviceRegistration,
        content: &NotificationContent,
    ) -> Result<PushPayloadMode, PushSendError> {
        let payload = apns_payload(content)?;
        let payload_mode = if payload
            .as_object()
            .is_some_and(|object| object.contains_key("alfred_automation"))
        {
            PushPayloadMode::Encrypted
        } else {
            PushPayloadMode::Fallback
        };

        let provider_token = self
            .provider_token()
            .map_err(|message| PushSendError::Permanent {
                code: "APNS_PROVIDER_TOKEN_INVALID".to_string(),
                message,
            })?;

        let url = format!(
            "{}/3/device/{}",
            apns_base_url(&device.environment),
            device.apns_token
        );

        let response = self
            .client
            .post(url)
            .header("authorization", format!("bearer {provider_token}"))
            .header("apns-topic", self.topic.as_str())
            .header("apns-push-type", "alert")
            .header("apns-priority", "10")
            .json(&payload)
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
        let reason = extract_apns_reason(body.as_str());
        let code = match reason.as_deref() {
            Some(value) => format!("APNS_{}", normalize_apns_reason(value)),
            None => format!("APNS_HTTP_{}", status.as_u16()),
        };
        let message = match reason.as_deref() {
            Some(value) if !body.is_empty() => {
                format!("APNs responded with status {status} ({value}): {body}")
            }
            Some(value) => format!("APNs responded with status {status} ({value})"),
            None if body.is_empty() => format!("APNs responded with status {status}"),
            None => format!("APNs responded with status {status}: {body}"),
        };

        match classify_http_failure(status) {
            FailureClass::Transient => Err(PushSendError::Transient { code, message }),
            FailureClass::Permanent => Err(PushSendError::Permanent { code, message }),
        }
    }

    fn provider_token(&self) -> Result<String, String> {
        let claims = ApnsProviderTokenClaims {
            iss: self.team_id.clone(),
            iat: Utc::now().timestamp(),
        };
        let mut header = Header::new(Algorithm::ES256);
        header.kid = Some(self.key_id.clone());

        encode(&header, &claims, &self.signing_key)
            .map_err(|err| format!("failed to sign APNs provider token: {err}"))
    }
}

fn apns_base_url(environment: &ApnsEnvironment) -> &'static str {
    match environment {
        ApnsEnvironment::Sandbox => APNS_SANDBOX_BASE_URL,
        ApnsEnvironment::Production => APNS_PRODUCTION_BASE_URL,
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

fn extract_apns_reason(body: &str) -> Option<String> {
    let parsed: Value = serde_json::from_str(body).ok()?;
    let reason = parsed.get("reason")?.as_str()?.trim();
    if reason.is_empty() {
        return None;
    }
    Some(reason.to_string())
}

fn normalize_apns_reason(reason: &str) -> String {
    let mut output = String::with_capacity(reason.len());
    let mut previous_was_separator = false;

    for ch in reason.chars() {
        if ch.is_ascii_alphanumeric() {
            output.push(ch.to_ascii_uppercase());
            previous_was_separator = false;
        } else if !previous_was_separator {
            output.push('_');
            previous_was_separator = true;
        }
    }

    output.trim_matches('_').to_string()
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
        apns_payload, classify_http_failure, enforce_apns_payload_size, extract_apns_reason,
        is_valid_encrypted_envelope, normalize_apns_reason,
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

    #[test]
    fn extracts_apns_reason_from_error_body() {
        let reason =
            extract_apns_reason(r#"{"reason":"BadDeviceToken"}"#).expect("reason should be parsed");
        assert_eq!(reason, "BadDeviceToken");
    }

    #[test]
    fn normalizes_apns_reason_to_code_suffix() {
        assert_eq!(normalize_apns_reason("BadDeviceToken"), "BADDEVICETOKEN");
        assert_eq!(
            normalize_apns_reason("Too-Many Requests"),
            "TOO_MANY_REQUESTS"
        );
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
