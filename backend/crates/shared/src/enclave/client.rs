use chrono::Utc;

mod conversions;

use super::{
    AutomationRecipientDevice, CompleteGoogleConnectResponse, ENCLAVE_RPC_AUTH_NONCE_HEADER,
    ENCLAVE_RPC_AUTH_SIGNATURE_HEADER, ENCLAVE_RPC_AUTH_TIMESTAMP_HEADER,
    ENCLAVE_RPC_CONTRACT_VERSION, ENCLAVE_RPC_CONTRACT_VERSION_HEADER,
    ENCLAVE_RPC_PATH_COMPLETE_GOOGLE_CONNECT, ENCLAVE_RPC_PATH_EXCHANGE_GOOGLE_TOKEN,
    ENCLAVE_RPC_PATH_EXECUTE_AUTOMATION, ENCLAVE_RPC_PATH_FETCH_ASSISTANT_ATTESTED_KEY,
    ENCLAVE_RPC_PATH_FETCH_GOOGLE_CALENDAR_EVENTS,
    ENCLAVE_RPC_PATH_FETCH_GOOGLE_URGENT_EMAIL_CANDIDATES, ENCLAVE_RPC_PATH_GENERATE_MORNING_BRIEF,
    ENCLAVE_RPC_PATH_GENERATE_URGENT_EMAIL_SUMMARY, ENCLAVE_RPC_PATH_PROCESS_ASSISTANT_QUERY,
    ENCLAVE_RPC_PATH_REVOKE_GOOGLE_TOKEN, EnclaveRpcAuthConfig,
    EnclaveRpcCompleteGoogleConnectRequest, EnclaveRpcCompleteGoogleConnectResponse,
    EnclaveRpcError, EnclaveRpcErrorEnvelope, EnclaveRpcExchangeGoogleTokenRequest,
    EnclaveRpcExchangeGoogleTokenResponse, EnclaveRpcExecuteAutomationRequest,
    EnclaveRpcExecuteAutomationResponse, EnclaveRpcFetchAssistantAttestedKeyRequest,
    EnclaveRpcFetchAssistantAttestedKeyResponse, EnclaveRpcFetchGoogleCalendarEventsRequest,
    EnclaveRpcFetchGoogleCalendarEventsResponse, EnclaveRpcFetchGoogleUrgentEmailCandidatesRequest,
    EnclaveRpcFetchGoogleUrgentEmailCandidatesResponse, EnclaveRpcGenerateMorningBriefRequest,
    EnclaveRpcGenerateMorningBriefResponse, EnclaveRpcGenerateUrgentEmailSummaryRequest,
    EnclaveRpcGenerateUrgentEmailSummaryResponse, EnclaveRpcProcessAssistantQueryRequest,
    EnclaveRpcProcessAssistantQueryResponse, EnclaveRpcRevokeGoogleTokenRequest,
    EnclaveRpcRevokeGoogleTokenResponse, ExchangeGoogleTokenResponse, ExecuteAutomationResponse,
    FetchAssistantAttestedKeyResponse, FetchGoogleCalendarEventsResponse,
    FetchGoogleUrgentEmailCandidatesResponse, GenerateMorningBriefResponse,
    GenerateUrgentEmailSummaryResponse, ProcessAssistantQueryResponse, ProviderOperation,
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

    pub async fn complete_google_connect(
        &self,
        user_id: uuid::Uuid,
        code: String,
        redirect_uri: String,
    ) -> Result<CompleteGoogleConnectResponse, EnclaveRpcError> {
        let payload = EnclaveRpcCompleteGoogleConnectRequest {
            contract_version: ENCLAVE_RPC_CONTRACT_VERSION.to_string(),
            request_id: uuid::Uuid::new_v4().to_string(),
            user_id,
            code,
            redirect_uri,
        };

        let response: EnclaveRpcCompleteGoogleConnectResponse = self
            .send_enclave_rpc(
                ProviderOperation::OAuthCodeExchange,
                ENCLAVE_RPC_PATH_COMPLETE_GOOGLE_CONNECT,
                &payload,
            )
            .await?;

        if response.request_id != payload.request_id {
            return Err(EnclaveRpcError::RpcResponseInvalid {
                message: "enclave rpc response request_id mismatch for oauth code exchange"
                    .to_string(),
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

    pub async fn execute_automation_run(
        &self,
        user_id: uuid::Uuid,
        automation_rule_id: uuid::Uuid,
        automation_run_id: uuid::Uuid,
        scheduled_for: chrono::DateTime<chrono::Utc>,
        prompt_envelope: crate::models::AutomationPromptEnvelope,
        recipient_devices: Vec<AutomationRecipientDevice>,
    ) -> Result<ExecuteAutomationResponse, EnclaveRpcError> {
        let payload = EnclaveRpcExecuteAutomationRequest {
            contract_version: ENCLAVE_RPC_CONTRACT_VERSION.to_string(),
            request_id: uuid::Uuid::new_v4().to_string(),
            user_id,
            automation_rule_id,
            automation_run_id,
            scheduled_for,
            prompt_envelope,
            recipient_devices: recipient_devices
                .into_iter()
                .map(|device| super::EnclaveAutomationRecipientDevice {
                    device_id: device.device_id,
                    key_id: device.key_id,
                    algorithm: device.algorithm,
                    public_key: device.public_key,
                })
                .collect(),
        };

        let response: EnclaveRpcExecuteAutomationResponse = self
            .send_enclave_rpc(
                ProviderOperation::AssistantAutomationRun,
                ENCLAVE_RPC_PATH_EXECUTE_AUTOMATION,
                &payload,
            )
            .await?;

        if response.request_id != payload.request_id {
            return Err(EnclaveRpcError::RpcResponseInvalid {
                message: "enclave rpc response request_id mismatch for automation run".to_string(),
            });
        }

        response.try_into()
    }

    pub async fn generate_morning_brief(
        &self,
        user_id: uuid::Uuid,
        connector: super::ConnectorSecretRequest,
        time_zone: String,
        morning_brief_local_time: String,
    ) -> Result<GenerateMorningBriefResponse, EnclaveRpcError> {
        let payload = EnclaveRpcGenerateMorningBriefRequest {
            contract_version: ENCLAVE_RPC_CONTRACT_VERSION.to_string(),
            request_id: uuid::Uuid::new_v4().to_string(),
            user_id,
            connector,
            time_zone,
            morning_brief_local_time,
        };

        let response: EnclaveRpcGenerateMorningBriefResponse = self
            .send_enclave_rpc(
                ProviderOperation::AssistantMorningBrief,
                ENCLAVE_RPC_PATH_GENERATE_MORNING_BRIEF,
                &payload,
            )
            .await?;

        if response.request_id != payload.request_id {
            return Err(EnclaveRpcError::RpcResponseInvalid {
                message: "enclave rpc response request_id mismatch for morning brief".to_string(),
            });
        }

        response.try_into()
    }

    pub async fn generate_urgent_email_summary(
        &self,
        user_id: uuid::Uuid,
        connector: super::ConnectorSecretRequest,
        max_results: usize,
    ) -> Result<GenerateUrgentEmailSummaryResponse, EnclaveRpcError> {
        let payload = EnclaveRpcGenerateUrgentEmailSummaryRequest {
            contract_version: ENCLAVE_RPC_CONTRACT_VERSION.to_string(),
            request_id: uuid::Uuid::new_v4().to_string(),
            user_id,
            connector,
            max_results,
        };

        let response: EnclaveRpcGenerateUrgentEmailSummaryResponse = self
            .send_enclave_rpc(
                ProviderOperation::AssistantUrgentEmail,
                ENCLAVE_RPC_PATH_GENERATE_URGENT_EMAIL_SUMMARY,
                &payload,
            )
            .await?;

        if response.request_id != payload.request_id {
            return Err(EnclaveRpcError::RpcResponseInvalid {
                message: "enclave rpc response request_id mismatch for urgent email".to_string(),
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
                message: format!(
                    "{err} (is_timeout={}, is_connect={})",
                    err.is_timeout(),
                    err.is_connect()
                ),
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
