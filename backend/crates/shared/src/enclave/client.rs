use chrono::Utc;

use super::{
    ENCLAVE_RPC_AUTH_NONCE_HEADER, ENCLAVE_RPC_AUTH_SIGNATURE_HEADER,
    ENCLAVE_RPC_AUTH_TIMESTAMP_HEADER, ENCLAVE_RPC_CONTRACT_VERSION,
    ENCLAVE_RPC_CONTRACT_VERSION_HEADER, ENCLAVE_RPC_PATH_EXCHANGE_GOOGLE_TOKEN,
    ENCLAVE_RPC_PATH_FETCH_ASSISTANT_ATTESTED_KEY, ENCLAVE_RPC_PATH_FETCH_GOOGLE_CALENDAR_EVENTS,
    ENCLAVE_RPC_PATH_FETCH_GOOGLE_URGENT_EMAIL_CANDIDATES,
    ENCLAVE_RPC_PATH_PROCESS_ASSISTANT_QUERY, ENCLAVE_RPC_PATH_REVOKE_GOOGLE_TOKEN,
    EnclaveRpcAuthConfig, EnclaveRpcError, EnclaveRpcErrorEnvelope,
    EnclaveRpcExchangeGoogleTokenRequest, EnclaveRpcExchangeGoogleTokenResponse,
    EnclaveRpcFetchAssistantAttestedKeyRequest, EnclaveRpcFetchAssistantAttestedKeyResponse,
    EnclaveRpcFetchGoogleCalendarEventsRequest, EnclaveRpcFetchGoogleCalendarEventsResponse,
    EnclaveRpcFetchGoogleUrgentEmailCandidatesRequest,
    EnclaveRpcFetchGoogleUrgentEmailCandidatesResponse, EnclaveRpcProcessAssistantQueryRequest,
    EnclaveRpcProcessAssistantQueryResponse, EnclaveRpcRevokeGoogleTokenRequest,
    EnclaveRpcRevokeGoogleTokenResponse, ExchangeGoogleTokenResponse,
    FetchAssistantAttestedKeyResponse, FetchGoogleCalendarEventsResponse,
    FetchGoogleUrgentEmailCandidatesResponse, ProcessAssistantQueryResponse, ProviderOperation,
    RevokeGoogleTokenResponse, sign_rpc_request,
};

#[derive(Clone)]
pub struct EnclaveRpcClient {
    base_url: String,
    auth: EnclaveRpcAuthConfig,
    http_client: reqwest::Client,
}

impl EnclaveRpcClient {
    pub fn new(base_url: String, auth: EnclaveRpcAuthConfig, http_client: reqwest::Client) -> Self {
        Self {
            base_url,
            auth,
            http_client,
        }
    }

    pub async fn exchange_google_access_token(
        &self,
        request: super::ConnectorSecretRequest,
    ) -> Result<ExchangeGoogleTokenResponse, EnclaveRpcError> {
        let payload = EnclaveRpcExchangeGoogleTokenRequest {
            contract_version: ENCLAVE_RPC_CONTRACT_VERSION.to_string(),
            request_id: uuid::Uuid::new_v4().to_string(),
            connector: request,
        };

        let response: EnclaveRpcExchangeGoogleTokenResponse = self
            .send_enclave_rpc(
                ProviderOperation::TokenRefresh,
                ENCLAVE_RPC_PATH_EXCHANGE_GOOGLE_TOKEN,
                &payload,
            )
            .await?;

        if response.request_id != payload.request_id {
            return Err(EnclaveRpcError::RpcResponseInvalid {
                message: "enclave rpc response request_id mismatch for exchange".to_string(),
            });
        }

        response.try_into()
    }

    pub async fn revoke_google_connector_token(
        &self,
        request: super::ConnectorSecretRequest,
    ) -> Result<RevokeGoogleTokenResponse, EnclaveRpcError> {
        let payload = EnclaveRpcRevokeGoogleTokenRequest {
            contract_version: ENCLAVE_RPC_CONTRACT_VERSION.to_string(),
            request_id: uuid::Uuid::new_v4().to_string(),
            connector: request,
        };

        let response: EnclaveRpcRevokeGoogleTokenResponse = self
            .send_enclave_rpc(
                ProviderOperation::TokenRevoke,
                ENCLAVE_RPC_PATH_REVOKE_GOOGLE_TOKEN,
                &payload,
            )
            .await?;

        if response.request_id != payload.request_id {
            return Err(EnclaveRpcError::RpcResponseInvalid {
                message: "enclave rpc response request_id mismatch for revoke".to_string(),
            });
        }

        response.try_into()
    }

    pub async fn fetch_google_calendar_events(
        &self,
        connector: super::ConnectorSecretRequest,
        time_min: String,
        time_max: String,
        max_results: usize,
    ) -> Result<FetchGoogleCalendarEventsResponse, EnclaveRpcError> {
        let payload = EnclaveRpcFetchGoogleCalendarEventsRequest {
            contract_version: ENCLAVE_RPC_CONTRACT_VERSION.to_string(),
            request_id: uuid::Uuid::new_v4().to_string(),
            connector,
            time_min,
            time_max,
            max_results,
        };

        let response: EnclaveRpcFetchGoogleCalendarEventsResponse = self
            .send_enclave_rpc(
                ProviderOperation::CalendarFetch,
                ENCLAVE_RPC_PATH_FETCH_GOOGLE_CALENDAR_EVENTS,
                &payload,
            )
            .await?;

        if response.request_id != payload.request_id {
            return Err(EnclaveRpcError::RpcResponseInvalid {
                message: "enclave rpc response request_id mismatch for calendar fetch".to_string(),
            });
        }

        response.try_into()
    }

    pub async fn fetch_google_urgent_email_candidates(
        &self,
        connector: super::ConnectorSecretRequest,
        max_results: usize,
    ) -> Result<FetchGoogleUrgentEmailCandidatesResponse, EnclaveRpcError> {
        let payload = EnclaveRpcFetchGoogleUrgentEmailCandidatesRequest {
            contract_version: ENCLAVE_RPC_CONTRACT_VERSION.to_string(),
            request_id: uuid::Uuid::new_v4().to_string(),
            connector,
            max_results,
        };

        let response: EnclaveRpcFetchGoogleUrgentEmailCandidatesResponse = self
            .send_enclave_rpc(
                ProviderOperation::GmailFetch,
                ENCLAVE_RPC_PATH_FETCH_GOOGLE_URGENT_EMAIL_CANDIDATES,
                &payload,
            )
            .await?;

        if response.request_id != payload.request_id {
            return Err(EnclaveRpcError::RpcResponseInvalid {
                message: "enclave rpc response request_id mismatch for gmail fetch".to_string(),
            });
        }

        response.try_into()
    }

    pub async fn fetch_assistant_attested_key(
        &self,
        challenge_nonce: String,
        issued_at: i64,
        expires_at: i64,
        request_id: String,
    ) -> Result<FetchAssistantAttestedKeyResponse, EnclaveRpcError> {
        let payload = EnclaveRpcFetchAssistantAttestedKeyRequest {
            contract_version: ENCLAVE_RPC_CONTRACT_VERSION.to_string(),
            request_id,
            challenge_nonce,
            issued_at,
            expires_at,
        };

        let response: EnclaveRpcFetchAssistantAttestedKeyResponse = self
            .send_enclave_rpc(
                ProviderOperation::AssistantAttestedKey,
                ENCLAVE_RPC_PATH_FETCH_ASSISTANT_ATTESTED_KEY,
                &payload,
            )
            .await?;

        if response.request_id != payload.request_id {
            return Err(EnclaveRpcError::RpcResponseInvalid {
                message: "enclave rpc response request_id mismatch for assistant key fetch"
                    .to_string(),
            });
        }

        response.try_into()
    }

    pub async fn process_assistant_query(
        &self,
        user_id: uuid::Uuid,
        request: crate::models::AssistantQueryRequest,
        prior_session_state: Option<crate::models::AssistantSessionStateEnvelope>,
    ) -> Result<ProcessAssistantQueryResponse, EnclaveRpcError> {
        let payload = EnclaveRpcProcessAssistantQueryRequest {
            contract_version: ENCLAVE_RPC_CONTRACT_VERSION.to_string(),
            request_id: uuid::Uuid::new_v4().to_string(),
            user_id,
            envelope: request.envelope,
            session_id: request.session_id,
            prior_session_state,
        };

        let response: EnclaveRpcProcessAssistantQueryResponse = self
            .send_enclave_rpc(
                ProviderOperation::AssistantQuery,
                ENCLAVE_RPC_PATH_PROCESS_ASSISTANT_QUERY,
                &payload,
            )
            .await?;

        if response.request_id != payload.request_id {
            return Err(EnclaveRpcError::RpcResponseInvalid {
                message: "enclave rpc response request_id mismatch for assistant query".to_string(),
            });
        }

        response.try_into()
    }

    async fn send_enclave_rpc<Req, Res>(
        &self,
        operation: ProviderOperation,
        path: &str,
        payload: &Req,
    ) -> Result<Res, EnclaveRpcError>
    where
        Req: serde::Serialize,
        Res: serde::de::DeserializeOwned,
    {
        let body =
            serde_json::to_vec(payload).map_err(|err| EnclaveRpcError::RpcResponseInvalid {
                message: format!("failed to serialize enclave rpc payload: {err}"),
            })?;

        let timestamp = Utc::now().timestamp();
        let nonce = uuid::Uuid::new_v4().simple().to_string();
        let signature = sign_rpc_request(
            &self.auth.shared_secret,
            "POST",
            path,
            timestamp,
            &nonce,
            &body,
        );

        let url = format!("{}{}", self.base_url.trim_end_matches('/'), path);
        let response = self
            .http_client
            .post(url)
            .header(
                ENCLAVE_RPC_CONTRACT_VERSION_HEADER,
                ENCLAVE_RPC_CONTRACT_VERSION,
            )
            .header(ENCLAVE_RPC_AUTH_TIMESTAMP_HEADER, timestamp.to_string())
            .header(ENCLAVE_RPC_AUTH_NONCE_HEADER, nonce)
            .header(ENCLAVE_RPC_AUTH_SIGNATURE_HEADER, signature)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await
            .map_err(|err| EnclaveRpcError::RpcTransportUnavailable {
                message: err.to_string(),
            })?;

        let status = response.status().as_u16();
        let bytes = response
            .bytes()
            .await
            .map_err(|err| EnclaveRpcError::RpcResponseInvalid {
                message: format!("failed to read enclave rpc response body: {err}"),
            })?;

        if (200..300).contains(&status) {
            let parsed = serde_json::from_slice::<Res>(&bytes).map_err(|err| {
                EnclaveRpcError::RpcResponseInvalid {
                    message: format!("failed to parse enclave rpc success response: {err}"),
                }
            })?;
            return Ok(parsed);
        }

        let error_envelope =
            serde_json::from_slice::<EnclaveRpcErrorEnvelope>(&bytes).map_err(|err| {
                EnclaveRpcError::RpcResponseInvalid {
                    message: format!("failed to parse enclave rpc error response: {err}"),
                }
            })?;
        if error_envelope.contract_version != ENCLAVE_RPC_CONTRACT_VERSION {
            return Err(EnclaveRpcError::RpcResponseInvalid {
                message: format!(
                    "enclave rpc contract mismatch in error response: expected={}, got={}",
                    ENCLAVE_RPC_CONTRACT_VERSION, error_envelope.contract_version
                ),
            });
        }

        Err(EnclaveRpcError::from_error_envelope(
            operation,
            status,
            error_envelope,
        ))
    }
}

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
