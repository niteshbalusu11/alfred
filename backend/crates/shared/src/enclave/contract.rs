use serde::{Deserialize, Serialize};

pub const ENCLAVE_RPC_CONTRACT_VERSION: &str = "v1";
pub const ENCLAVE_RPC_PATH_EXCHANGE_GOOGLE_TOKEN: &str = "/v1/rpc/google/token/exchange";
pub const ENCLAVE_RPC_PATH_REVOKE_GOOGLE_TOKEN: &str = "/v1/rpc/google/token/revoke";
pub const ENCLAVE_RPC_PATH_FETCH_GOOGLE_CALENDAR_EVENTS: &str = "/v1/rpc/google/calendar/events";
pub const ENCLAVE_RPC_PATH_FETCH_GOOGLE_URGENT_EMAIL_CANDIDATES: &str =
    "/v1/rpc/google/gmail/urgent-candidates";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttestedIdentityPayload {
    pub runtime: String,
    pub measurement: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnclaveRpcExchangeGoogleTokenRequest {
    pub contract_version: String,
    pub request_id: String,
    pub connector: super::ConnectorSecretRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnclaveRpcExchangeGoogleTokenResponse {
    pub contract_version: String,
    pub request_id: String,
    pub access_token: String,
    pub attested_identity: AttestedIdentityPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnclaveRpcRevokeGoogleTokenRequest {
    pub contract_version: String,
    pub request_id: String,
    pub connector: super::ConnectorSecretRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnclaveRpcRevokeGoogleTokenResponse {
    pub contract_version: String,
    pub request_id: String,
    pub attested_identity: AttestedIdentityPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnclaveRpcFetchGoogleCalendarEventsRequest {
    pub contract_version: String,
    pub request_id: String,
    pub connector: super::ConnectorSecretRequest,
    pub time_min: String,
    pub time_max: String,
    pub max_results: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnclaveRpcFetchGoogleCalendarEventsResponse {
    pub contract_version: String,
    pub request_id: String,
    pub events: Vec<EnclaveGoogleCalendarEvent>,
    pub attested_identity: AttestedIdentityPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnclaveGoogleCalendarEvent {
    pub id: Option<String>,
    pub summary: Option<String>,
    pub start: Option<EnclaveGoogleCalendarEventDateTime>,
    pub end: Option<EnclaveGoogleCalendarEventDateTime>,
    #[serde(default)]
    pub attendees: Vec<EnclaveGoogleCalendarAttendee>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnclaveGoogleCalendarEventDateTime {
    #[serde(rename = "dateTime")]
    pub date_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnclaveGoogleCalendarAttendee {
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnclaveRpcFetchGoogleUrgentEmailCandidatesRequest {
    pub contract_version: String,
    pub request_id: String,
    pub connector: super::ConnectorSecretRequest,
    pub max_results: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnclaveRpcFetchGoogleUrgentEmailCandidatesResponse {
    pub contract_version: String,
    pub request_id: String,
    pub candidates: Vec<EnclaveGoogleEmailCandidate>,
    pub attested_identity: AttestedIdentityPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnclaveGoogleEmailCandidate {
    pub message_id: Option<String>,
    pub from: Option<String>,
    pub subject: Option<String>,
    pub snippet: Option<String>,
    pub received_at: Option<String>,
    #[serde(default)]
    pub label_ids: Vec<String>,
    pub has_attachments: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnclaveRpcErrorEnvelope {
    pub contract_version: String,
    pub request_id: Option<String>,
    pub error: EnclaveRpcErrorPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnclaveRpcErrorPayload {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oauth_error: Option<String>,
}

impl EnclaveRpcErrorEnvelope {
    pub fn new(
        request_id: Option<String>,
        code: impl Into<String>,
        message: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self {
            contract_version: ENCLAVE_RPC_CONTRACT_VERSION.to_string(),
            request_id,
            error: EnclaveRpcErrorPayload {
                code: code.into(),
                message: message.into(),
                retryable,
                provider_status: None,
                oauth_error: None,
            },
        }
    }

    pub fn with_provider_failure(
        request_id: Option<String>,
        status: u16,
        oauth_error: Option<String>,
    ) -> Self {
        Self {
            contract_version: ENCLAVE_RPC_CONTRACT_VERSION.to_string(),
            request_id,
            error: EnclaveRpcErrorPayload {
                code: "provider_failed".to_string(),
                message: "Provider request failed".to_string(),
                retryable: status >= 500,
                provider_status: Some(status),
                oauth_error,
            },
        }
    }
}
