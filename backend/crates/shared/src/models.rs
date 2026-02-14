use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionRequest {
    pub apple_identity_token: String,
    pub device_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApnsEnvironment {
    Sandbox,
    Production,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterDeviceRequest {
    pub device_id: String,
    pub apns_token: String,
    pub environment: ApnsEnvironment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartGoogleConnectRequest {
    pub redirect_uri: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartGoogleConnectResponse {
    pub auth_url: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteGoogleConnectRequest {
    #[serde(default)]
    pub code: Option<String>,
    pub state: String,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub error_description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConnectorStatus {
    Active,
    Revoked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteGoogleConnectResponse {
    pub connector_id: String,
    pub status: ConnectorStatus,
    pub granted_scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevokeConnectorResponse {
    pub status: ConnectorStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preferences {
    pub meeting_reminder_minutes: u32,
    pub morning_brief_local_time: String,
    pub quiet_hours_start: String,
    pub quiet_hours_end: String,
    pub high_risk_requires_confirm: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub event_type: String,
    pub connector: Option<String>,
    pub result: String,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListAuditEventsResponse {
    pub items: Vec<AuditEvent>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteAllResponse {
    pub request_id: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OkResponse {
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: ErrorBody,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
}
