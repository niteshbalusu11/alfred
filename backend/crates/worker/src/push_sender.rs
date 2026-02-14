use reqwest::StatusCode;
use serde::Serialize;
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
}

#[derive(Debug, Serialize)]
struct PushDeliveryRequest<'a> {
    device_token: &'a str,
    title: &'a str,
    body: &'a str,
}

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
    ) -> Result<(), PushSendError> {
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
            return Ok(());
        };

        let request = PushDeliveryRequest {
            device_token: &device.apns_token,
            title: &content.title,
            body: &content.body,
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
            return Ok(());
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
    use reqwest::StatusCode;

    use super::classify_http_failure;
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
}
