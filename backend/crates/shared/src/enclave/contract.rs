use serde::{Deserialize, Serialize};

pub const ENCLAVE_RPC_CONTRACT_VERSION: &str = "v1";
pub const ENCLAVE_RPC_PATH_EXCHANGE_GOOGLE_TOKEN: &str = "/v1/rpc/google/token/exchange";
pub const ENCLAVE_RPC_PATH_REVOKE_GOOGLE_TOKEN: &str = "/v1/rpc/google/token/revoke";

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
