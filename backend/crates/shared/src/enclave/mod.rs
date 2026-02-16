mod client;
mod contract;
mod service;
mod transport_auth;

#[cfg(test)]
mod tests;

use std::fmt;

use thiserror::Error;
use uuid::Uuid;

pub use client::EnclaveRpcClient;
pub use contract::{
    AttestedIdentityPayload, ENCLAVE_RPC_CONTRACT_VERSION, ENCLAVE_RPC_PATH_EXCHANGE_GOOGLE_TOKEN,
    ENCLAVE_RPC_PATH_FETCH_ASSISTANT_ATTESTED_KEY, ENCLAVE_RPC_PATH_FETCH_GOOGLE_CALENDAR_EVENTS,
    ENCLAVE_RPC_PATH_FETCH_GOOGLE_URGENT_EMAIL_CANDIDATES,
    ENCLAVE_RPC_PATH_PROCESS_ASSISTANT_QUERY, ENCLAVE_RPC_PATH_REVOKE_GOOGLE_TOKEN,
    EnclaveGoogleCalendarAttendee, EnclaveGoogleCalendarEvent, EnclaveGoogleCalendarEventDateTime,
    EnclaveGoogleEmailCandidate, EnclaveRpcErrorEnvelope, EnclaveRpcErrorPayload,
    EnclaveRpcExchangeGoogleTokenRequest, EnclaveRpcExchangeGoogleTokenResponse,
    EnclaveRpcFetchAssistantAttestedKeyRequest, EnclaveRpcFetchAssistantAttestedKeyResponse,
    EnclaveRpcFetchGoogleCalendarEventsRequest, EnclaveRpcFetchGoogleCalendarEventsResponse,
    EnclaveRpcFetchGoogleUrgentEmailCandidatesRequest,
    EnclaveRpcFetchGoogleUrgentEmailCandidatesResponse, EnclaveRpcProcessAssistantQueryRequest,
    EnclaveRpcProcessAssistantQueryResponse, EnclaveRpcRevokeGoogleTokenRequest,
    EnclaveRpcRevokeGoogleTokenResponse,
};
pub use service::EnclaveOperationService;
pub use transport_auth::{
    ENCLAVE_RPC_AUTH_NONCE_HEADER, ENCLAVE_RPC_AUTH_SIGNATURE_HEADER,
    ENCLAVE_RPC_AUTH_TIMESTAMP_HEADER, ENCLAVE_RPC_CONTRACT_VERSION_HEADER, EnclaveRpcAuthConfig,
    constant_time_eq, sign_rpc_request,
};

#[derive(Debug, Clone)]
pub struct GoogleEnclaveOauthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub token_url: String,
    pub revoke_url: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConnectorSecretRequest {
    pub user_id: Uuid,
    pub connector_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct ExchangeGoogleTokenResponse {
    pub access_token: String,
    pub attested_identity: AttestedIdentityPayload,
}

#[derive(Debug, Clone)]
pub struct RevokeGoogleTokenResponse {
    pub attested_identity: AttestedIdentityPayload,
}

#[derive(Debug, Clone)]
pub struct FetchGoogleCalendarEventsResponse {
    pub events: Vec<EnclaveGoogleCalendarEvent>,
    pub attested_identity: AttestedIdentityPayload,
}

#[derive(Debug, Clone)]
pub struct FetchGoogleUrgentEmailCandidatesResponse {
    pub candidates: Vec<EnclaveGoogleEmailCandidate>,
    pub attested_identity: AttestedIdentityPayload,
}

#[derive(Debug, Clone)]
pub struct FetchAssistantAttestedKeyResponse {
    pub request_id: String,
    pub runtime: String,
    pub measurement: String,
    pub challenge_nonce: String,
    pub issued_at: i64,
    pub expires_at: i64,
    pub evidence_issued_at: i64,
    pub key_id: String,
    pub algorithm: String,
    pub public_key: String,
    pub key_expires_at: i64,
    pub signature: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProcessAssistantQueryResponse {
    pub session_id: Uuid,
    pub envelope: crate::models::AssistantEncryptedResponseEnvelope,
    pub session_state: Option<crate::models::AssistantSessionStateEnvelope>,
    pub attested_identity: AttestedIdentityPayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderOperation {
    TokenRefresh,
    TokenRevoke,
    CalendarFetch,
    GmailFetch,
    AssistantAttestedKey,
    AssistantQuery,
}

impl fmt::Display for ProviderOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TokenRefresh => write!(f, "token_refresh"),
            Self::TokenRevoke => write!(f, "token_revoke"),
            Self::CalendarFetch => write!(f, "calendar_fetch"),
            Self::GmailFetch => write!(f, "gmail_fetch"),
            Self::AssistantAttestedKey => write!(f, "assistant_attested_key"),
            Self::AssistantQuery => write!(f, "assistant_query"),
        }
    }
}

#[derive(Debug, Error)]
pub enum EnclaveRpcError {
    #[error("enclave rpc request unauthorized: code={code}")]
    RpcUnauthorized { code: String },
    #[error("enclave rpc request rejected: code={code}")]
    RpcContractRejected { code: String },
    #[error("enclave rpc transport unavailable: {message}")]
    RpcTransportUnavailable { message: String },
    #[error("enclave rpc response invalid: {message}")]
    RpcResponseInvalid { message: String },
    #[error("connector decrypt authorization failed: {message}")]
    DecryptNotAuthorized { message: String },
    #[error("connector token decrypt failed: {message}")]
    ConnectorTokenDecryptFailed { message: String },
    #[error("connector token is unavailable for active connector")]
    ConnectorTokenUnavailable,
    #[error("provider request unavailable for {operation}: {message}")]
    ProviderRequestUnavailable {
        operation: ProviderOperation,
        message: String,
    },
    #[error("provider request failed for {operation}: status={status}")]
    ProviderRequestFailed {
        operation: ProviderOperation,
        status: u16,
        oauth_error: Option<String>,
    },
    #[error("provider response invalid for {operation}: {message}")]
    ProviderResponseInvalid {
        operation: ProviderOperation,
        message: String,
    },
}

impl EnclaveRpcError {
    pub fn from_error_envelope(
        operation: ProviderOperation,
        status: u16,
        envelope: EnclaveRpcErrorEnvelope,
    ) -> Self {
        match envelope.error.code.as_str() {
            "decrypt_not_authorized" => Self::DecryptNotAuthorized {
                message: envelope.error.message,
            },
            "connector_token_decrypt_failed" => Self::ConnectorTokenDecryptFailed {
                message: envelope.error.message,
            },
            "connector_token_unavailable" => Self::ConnectorTokenUnavailable,
            "provider_unavailable" => Self::ProviderRequestUnavailable {
                operation,
                message: envelope.error.message,
            },
            "provider_failed" => Self::ProviderRequestFailed {
                operation,
                status: envelope.error.provider_status.unwrap_or(status),
                oauth_error: envelope.error.oauth_error,
            },
            "provider_response_invalid" => Self::ProviderResponseInvalid {
                operation,
                message: envelope.error.message,
            },
            "missing_request_header"
            | "invalid_request_header"
            | "invalid_request_signature"
            | "invalid_request_timestamp"
            | "request_replay_detected" => Self::RpcUnauthorized {
                code: envelope.error.code,
            },
            "invalid_contract_version" | "invalid_request_payload" | "invalid_request_id" => {
                Self::RpcContractRejected {
                    code: envelope.error.code,
                }
            }
            _ => Self::RpcResponseInvalid {
                message: format!(
                    "unknown enclave error envelope code={} message={}",
                    envelope.error.code, envelope.error.message
                ),
            },
        }
    }
}
