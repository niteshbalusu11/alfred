use super::*;

impl TryFrom<EnclaveRpcExchangeGoogleTokenResponse> for ExchangeGoogleTokenResponse {
    type Error = EnclaveRpcError;

    fn try_from(value: EnclaveRpcExchangeGoogleTokenResponse) -> Result<Self, Self::Error> {
        if value.contract_version != ENCLAVE_RPC_CONTRACT_VERSION {
            return Err(EnclaveRpcError::RpcResponseInvalid {
                message: format!(
                    "enclave rpc contract mismatch: expected={}, got={}",
                    ENCLAVE_RPC_CONTRACT_VERSION, value.contract_version
                ),
            });
        }

        if value.request_id.trim().is_empty() {
            return Err(EnclaveRpcError::RpcResponseInvalid {
                message: "missing request_id in exchange response".to_string(),
            });
        }

        Ok(Self {
            access_token: value.access_token,
            attested_identity: value.attested_identity,
        })
    }
}

impl TryFrom<EnclaveRpcRevokeGoogleTokenResponse> for RevokeGoogleTokenResponse {
    type Error = EnclaveRpcError;

    fn try_from(value: EnclaveRpcRevokeGoogleTokenResponse) -> Result<Self, Self::Error> {
        if value.contract_version != ENCLAVE_RPC_CONTRACT_VERSION {
            return Err(EnclaveRpcError::RpcResponseInvalid {
                message: format!(
                    "enclave rpc contract mismatch: expected={}, got={}",
                    ENCLAVE_RPC_CONTRACT_VERSION, value.contract_version
                ),
            });
        }

        if value.request_id.trim().is_empty() {
            return Err(EnclaveRpcError::RpcResponseInvalid {
                message: "missing request_id in revoke response".to_string(),
            });
        }

        Ok(Self {
            attested_identity: value.attested_identity,
        })
    }
}

impl TryFrom<EnclaveRpcFetchGoogleCalendarEventsResponse> for FetchGoogleCalendarEventsResponse {
    type Error = EnclaveRpcError;

    fn try_from(value: EnclaveRpcFetchGoogleCalendarEventsResponse) -> Result<Self, Self::Error> {
        if value.contract_version != ENCLAVE_RPC_CONTRACT_VERSION {
            return Err(EnclaveRpcError::RpcResponseInvalid {
                message: format!(
                    "enclave rpc contract mismatch: expected={}, got={}",
                    ENCLAVE_RPC_CONTRACT_VERSION, value.contract_version
                ),
            });
        }

        if value.request_id.trim().is_empty() {
            return Err(EnclaveRpcError::RpcResponseInvalid {
                message: "missing request_id in calendar fetch response".to_string(),
            });
        }

        Ok(Self {
            events: value.events,
            attested_identity: value.attested_identity,
        })
    }
}

impl TryFrom<EnclaveRpcFetchGoogleUrgentEmailCandidatesResponse>
    for FetchGoogleUrgentEmailCandidatesResponse
{
    type Error = EnclaveRpcError;

    fn try_from(
        value: EnclaveRpcFetchGoogleUrgentEmailCandidatesResponse,
    ) -> Result<Self, Self::Error> {
        if value.contract_version != ENCLAVE_RPC_CONTRACT_VERSION {
            return Err(EnclaveRpcError::RpcResponseInvalid {
                message: format!(
                    "enclave rpc contract mismatch: expected={}, got={}",
                    ENCLAVE_RPC_CONTRACT_VERSION, value.contract_version
                ),
            });
        }

        if value.request_id.trim().is_empty() {
            return Err(EnclaveRpcError::RpcResponseInvalid {
                message: "missing request_id in gmail fetch response".to_string(),
            });
        }

        Ok(Self {
            candidates: value.candidates,
            attested_identity: value.attested_identity,
        })
    }
}

impl TryFrom<EnclaveRpcFetchAssistantAttestedKeyResponse> for FetchAssistantAttestedKeyResponse {
    type Error = EnclaveRpcError;

    fn try_from(value: EnclaveRpcFetchAssistantAttestedKeyResponse) -> Result<Self, Self::Error> {
        if value.contract_version != ENCLAVE_RPC_CONTRACT_VERSION {
            return Err(EnclaveRpcError::RpcResponseInvalid {
                message: format!(
                    "enclave rpc contract mismatch: expected={}, got={}",
                    ENCLAVE_RPC_CONTRACT_VERSION, value.contract_version
                ),
            });
        }

        if value.request_id.trim().is_empty() {
            return Err(EnclaveRpcError::RpcResponseInvalid {
                message: "missing request_id in assistant key response".to_string(),
            });
        }

        Ok(Self {
            request_id: value.request_id,
            runtime: value.runtime,
            measurement: value.measurement,
            challenge_nonce: value.challenge_nonce,
            issued_at: value.issued_at,
            expires_at: value.expires_at,
            evidence_issued_at: value.evidence_issued_at,
            key_id: value.key_id,
            algorithm: value.algorithm,
            public_key: value.public_key,
            key_expires_at: value.key_expires_at,
            signature: value.signature,
        })
    }
}

impl TryFrom<EnclaveRpcProcessAssistantQueryResponse> for ProcessAssistantQueryResponse {
    type Error = EnclaveRpcError;

    fn try_from(value: EnclaveRpcProcessAssistantQueryResponse) -> Result<Self, Self::Error> {
        if value.contract_version != ENCLAVE_RPC_CONTRACT_VERSION {
            return Err(EnclaveRpcError::RpcResponseInvalid {
                message: format!(
                    "enclave rpc contract mismatch: expected={}, got={}",
                    ENCLAVE_RPC_CONTRACT_VERSION, value.contract_version
                ),
            });
        }

        if value.request_id.trim().is_empty() {
            return Err(EnclaveRpcError::RpcResponseInvalid {
                message: "missing request_id in assistant query response".to_string(),
            });
        }

        Ok(Self {
            session_id: value.session_id,
            envelope: value.envelope,
            session_state: value.session_state,
            attested_identity: value.attested_identity,
        })
    }
}

impl TryFrom<EnclaveRpcGenerateMorningBriefResponse> for GenerateMorningBriefResponse {
    type Error = EnclaveRpcError;

    fn try_from(value: EnclaveRpcGenerateMorningBriefResponse) -> Result<Self, Self::Error> {
        if value.contract_version != ENCLAVE_RPC_CONTRACT_VERSION {
            return Err(EnclaveRpcError::RpcResponseInvalid {
                message: format!(
                    "enclave rpc contract mismatch: expected={}, got={}",
                    ENCLAVE_RPC_CONTRACT_VERSION, value.contract_version
                ),
            });
        }

        if value.request_id.trim().is_empty() {
            return Err(EnclaveRpcError::RpcResponseInvalid {
                message: "missing request_id in morning brief response".to_string(),
            });
        }

        Ok(Self {
            notification: super::super::EnclaveGeneratedNotification {
                title: value.notification.title,
                body: value.notification.body,
            },
            metadata: value.metadata,
            attested_identity: value.attested_identity,
        })
    }
}

impl TryFrom<EnclaveRpcGenerateUrgentEmailSummaryResponse> for GenerateUrgentEmailSummaryResponse {
    type Error = EnclaveRpcError;

    fn try_from(value: EnclaveRpcGenerateUrgentEmailSummaryResponse) -> Result<Self, Self::Error> {
        if value.contract_version != ENCLAVE_RPC_CONTRACT_VERSION {
            return Err(EnclaveRpcError::RpcResponseInvalid {
                message: format!(
                    "enclave rpc contract mismatch: expected={}, got={}",
                    ENCLAVE_RPC_CONTRACT_VERSION, value.contract_version
                ),
            });
        }

        if value.request_id.trim().is_empty() {
            return Err(EnclaveRpcError::RpcResponseInvalid {
                message: "missing request_id in urgent email response".to_string(),
            });
        }

        Ok(Self {
            should_notify: value.should_notify,
            notification: value.notification.map(|notification| {
                super::super::EnclaveGeneratedNotification {
                    title: notification.title,
                    body: notification.body,
                }
            }),
            metadata: value.metadata,
            attested_identity: value.attested_identity,
        })
    }
}
