use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
pub struct SendTestNotificationRequest {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendTestNotificationResponse {
    pub queued_job_id: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantQueryRequest {
    pub envelope: AssistantEncryptedRequestEnvelope,
    #[serde(default)]
    pub session_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssistantEncryptedRequestEnvelope {
    pub version: String,
    pub algorithm: String,
    pub key_id: String,
    pub request_id: String,
    pub client_ephemeral_public_key: String,
    pub nonce: String,
    pub ciphertext: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssistantEncryptedResponseEnvelope {
    pub version: String,
    pub algorithm: String,
    pub key_id: String,
    pub request_id: String,
    pub nonce: String,
    pub ciphertext: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssistantSessionStateEnvelope {
    pub version: String,
    pub algorithm: String,
    pub key_id: String,
    pub nonce: String,
    pub ciphertext: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssistantQueryCapability {
    MeetingsToday,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMeetingsTodayPayload {
    pub title: String,
    pub summary: String,
    pub key_points: Vec<String>,
    pub follow_ups: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantQueryResponse {
    pub session_id: Uuid,
    pub envelope: AssistantEncryptedResponseEnvelope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantPlaintextQueryRequest {
    pub query: String,
    #[serde(default)]
    pub session_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantPlaintextQueryResponse {
    pub session_id: Uuid,
    pub capability: AssistantQueryCapability,
    pub display_text: String,
    pub payload: AssistantMeetingsTodayPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssistantAttestedKeyRequest {
    pub challenge_nonce: String,
    pub issued_at: i64,
    pub expires_at: i64,
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssistantAttestedKeyResponse {
    pub key_id: String,
    pub algorithm: String,
    pub public_key: String,
    pub key_expires_at: i64,
    pub attestation: AssistantAttestedKeyAttestation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssistantAttestedKeyAttestation {
    pub runtime: String,
    pub measurement: String,
    pub challenge_nonce: String,
    pub issued_at: i64,
    pub expires_at: i64,
    pub request_id: String,
    pub evidence_issued_at: i64,
    pub signature: Option<String>,
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
    pub time_zone: String,
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
pub struct DeleteAllStatusResponse {
    pub request_id: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub failed_at: Option<DateTime<Utc>>,
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
