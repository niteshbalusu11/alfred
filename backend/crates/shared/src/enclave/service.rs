use reqwest::{RequestBuilder, StatusCode};
use serde::de::DeserializeOwned;

use crate::repos::{ConnectorKeyMetadata as PersistedConnectorKeyMetadata, Store};
use crate::security::{ConnectorKeyMetadata as AuthorizedConnectorKeyMetadata, SecretRuntime};

mod google_types;

use self::google_types::{
    GmailMessageMetadataResponse, GmailMessagesResponse, GoogleCalendarEventsResponse,
    GoogleOAuthCodeExchangeResponse, GoogleRefreshTokenResponse, parse_google_error_code,
};

use super::{
    AttestedIdentityPayload, CompleteGoogleConnectResponse, ConnectorSecretRequest,
    EnclaveGoogleCalendarAttendee, EnclaveGoogleCalendarEvent, EnclaveGoogleCalendarEventDateTime,
    EnclaveRpcError, ExchangeGoogleTokenResponse, FetchGoogleCalendarEventsResponse,
    FetchGoogleUrgentEmailCandidatesResponse, GoogleEnclaveOauthConfig, ProviderOperation,
    RevokeGoogleTokenResponse,
};

const GOOGLE_CALENDAR_EVENTS_URL: &str =
    "https://www.googleapis.com/calendar/v3/calendars/primary/events";
const GMAIL_MESSAGES_URL: &str = "https://gmail.googleapis.com/gmail/v1/users/me/messages";
const MAX_GMAIL_CANDIDATES: usize = 50;
const DEFAULT_GOOGLE_CONNECT_SCOPES: [&str; 2] = [
    "https://www.googleapis.com/auth/gmail.readonly",
    "https://www.googleapis.com/auth/calendar.readonly",
];

#[derive(Clone)]
pub struct EnclaveOperationService {
    store: Store,
    secret_runtime: SecretRuntime,
    http_client: reqwest::Client,
    oauth: GoogleEnclaveOauthConfig,
}

impl EnclaveOperationService {
    pub fn new(
        store: Store,
        secret_runtime: SecretRuntime,
        http_client: reqwest::Client,
        oauth: GoogleEnclaveOauthConfig,
    ) -> Self {
        Self {
            store,
            secret_runtime,
            http_client,
            oauth,
        }
    }

    pub async fn exchange_google_access_token(
        &self,
        request: ConnectorSecretRequest,
    ) -> Result<ExchangeGoogleTokenResponse, EnclaveRpcError> {
        let (refresh_token, attested_identity) =
            self.load_authorized_refresh_token(&request).await?;
        let access_token = self.exchange_access_token(&refresh_token).await?;

        Ok(ExchangeGoogleTokenResponse {
            access_token,
            attested_identity,
        })
    }

    pub async fn complete_google_connect(
        &self,
        user_id: uuid::Uuid,
        code: String,
        redirect_uri: String,
    ) -> Result<CompleteGoogleConnectResponse, EnclaveRpcError> {
        let response = self
            .http_client
            .post(&self.oauth.token_url)
            .form(&[
                ("code", code.as_str()),
                ("client_id", self.oauth.client_id.as_str()),
                ("client_secret", self.oauth.client_secret.as_str()),
                ("redirect_uri", redirect_uri.as_str()),
                ("grant_type", "authorization_code"),
            ])
            .send()
            .await
            .map_err(|err| EnclaveRpcError::ProviderRequestUnavailable {
                operation: ProviderOperation::OAuthCodeExchange,
                message: err.to_string(),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            let oauth_error = parse_google_error_code(&body)
                .filter(|value| matches!(value.as_str(), "invalid_grant" | "access_denied"));
            return Err(EnclaveRpcError::ProviderRequestFailed {
                operation: ProviderOperation::OAuthCodeExchange,
                status: status.as_u16(),
                oauth_error,
            });
        }

        let payload = response
            .json::<GoogleOAuthCodeExchangeResponse>()
            .await
            .map_err(|err| EnclaveRpcError::ProviderResponseInvalid {
                operation: ProviderOperation::OAuthCodeExchange,
                message: err.to_string(),
            })?;

        let refresh_token = payload
            .refresh_token
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| EnclaveRpcError::ProviderResponseInvalid {
                operation: ProviderOperation::OAuthCodeExchange,
                message: "oauth code exchange response missing refresh token".to_string(),
            })?;

        let granted_scopes = payload
            .scope
            .map(|scope| {
                scope
                    .split_whitespace()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            })
            .filter(|scopes| !scopes.is_empty())
            .unwrap_or_else(|| {
                DEFAULT_GOOGLE_CONNECT_SCOPES
                    .iter()
                    .map(|scope| (*scope).to_string())
                    .collect::<Vec<_>>()
            });

        let connector_id = self
            .store
            .upsert_google_connector(
                user_id,
                &refresh_token,
                &granted_scopes,
                self.secret_runtime.kms_key_id(),
                self.secret_runtime.kms_key_version(),
            )
            .await
            .map_err(|err| EnclaveRpcError::ConnectorTokenDecryptFailed {
                message: err.to_string(),
            })?;

        Ok(CompleteGoogleConnectResponse {
            connector_id,
            granted_scopes,
        })
    }

    pub async fn revoke_google_connector_token(
        &self,
        request: ConnectorSecretRequest,
    ) -> Result<RevokeGoogleTokenResponse, EnclaveRpcError> {
        let (refresh_token, attested_identity) =
            self.load_authorized_refresh_token(&request).await?;

        let response = self
            .http_client
            .post(&self.oauth.revoke_url)
            .form(&[("token", refresh_token.as_str())])
            .send()
            .await
            .map_err(|err| EnclaveRpcError::ProviderRequestUnavailable {
                operation: ProviderOperation::TokenRevoke,
                message: err.to_string(),
            })?;

        if response.status().is_success() {
            return Ok(RevokeGoogleTokenResponse { attested_identity });
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if status == StatusCode::BAD_REQUEST
            && let Some(error) = parse_google_error_code(&body)
            && error == "invalid_token"
        {
            return Ok(RevokeGoogleTokenResponse { attested_identity });
        }

        Err(EnclaveRpcError::ProviderRequestFailed {
            operation: ProviderOperation::TokenRevoke,
            status: status.as_u16(),
            oauth_error: parse_google_error_code(&body),
        })
    }

    pub async fn fetch_google_calendar_events(
        &self,
        request: ConnectorSecretRequest,
        time_min: String,
        time_max: String,
        max_results: usize,
    ) -> Result<FetchGoogleCalendarEventsResponse, EnclaveRpcError> {
        let (refresh_token, attested_identity) =
            self.load_authorized_refresh_token(&request).await?;
        let access_token = self.exchange_access_token(&refresh_token).await?;
        let max_results = max_results.to_string();

        let payload: GoogleCalendarEventsResponse = self
            .send_google_json_request(
                self.http_client
                    .get(GOOGLE_CALENDAR_EVENTS_URL)
                    .bearer_auth(access_token)
                    .query(&[
                        ("singleEvents", "true"),
                        ("orderBy", "startTime"),
                        ("timeMin", time_min.as_str()),
                        ("timeMax", time_max.as_str()),
                        ("maxResults", max_results.as_str()),
                    ]),
                ProviderOperation::CalendarFetch,
            )
            .await?;

        let events = payload
            .items
            .into_iter()
            .map(|event| EnclaveGoogleCalendarEvent {
                id: event.id,
                summary: event.summary,
                start: event.start.map(|start| EnclaveGoogleCalendarEventDateTime {
                    date_time: start.date_time,
                }),
                end: event.end.map(|end| EnclaveGoogleCalendarEventDateTime {
                    date_time: end.date_time,
                }),
                attendees: event
                    .attendees
                    .into_iter()
                    .map(|attendee| EnclaveGoogleCalendarAttendee {
                        email: attendee.email,
                    })
                    .collect(),
            })
            .collect();

        Ok(FetchGoogleCalendarEventsResponse {
            events,
            attested_identity,
        })
    }

    pub async fn fetch_google_urgent_email_candidates(
        &self,
        request: ConnectorSecretRequest,
        max_results: usize,
    ) -> Result<FetchGoogleUrgentEmailCandidatesResponse, EnclaveRpcError> {
        self.fetch_google_email_candidates(request, Some("newer_than:2d".to_string()), max_results)
            .await
    }

    pub async fn fetch_google_email_candidates(
        &self,
        request: ConnectorSecretRequest,
        gmail_query: Option<String>,
        max_results: usize,
    ) -> Result<FetchGoogleUrgentEmailCandidatesResponse, EnclaveRpcError> {
        let (refresh_token, attested_identity) =
            self.load_authorized_refresh_token(&request).await?;
        let access_token = self.exchange_access_token(&refresh_token).await?;
        let max_results = max_results.clamp(1, MAX_GMAIL_CANDIDATES).to_string();
        let mut query_params = vec![
            ("labelIds".to_string(), "INBOX".to_string()),
            ("maxResults".to_string(), max_results),
        ];
        if let Some(gmail_query) = gmail_query.map(|value| value.trim().to_string())
            && !gmail_query.is_empty()
        {
            query_params.push(("q".to_string(), gmail_query));
        }

        let payload: GmailMessagesResponse = self
            .send_google_json_request(
                self.http_client
                    .get(GMAIL_MESSAGES_URL)
                    .bearer_auth(&access_token)
                    .query(&query_params),
                ProviderOperation::GmailFetch,
            )
            .await?;

        let mut candidates = Vec::with_capacity(payload.messages.len());
        for message in payload.messages {
            let details: GmailMessageMetadataResponse = self
                .send_google_json_request(
                    self.http_client
                        .get(format!("{GMAIL_MESSAGES_URL}/{}", message.id))
                        .bearer_auth(&access_token)
                        .query(&[
                            ("format", "metadata"),
                            ("metadataHeaders", "From"),
                            ("metadataHeaders", "Subject"),
                        ]),
                    ProviderOperation::GmailFetch,
                )
                .await?;
            candidates.push(details.into_candidate());
        }

        Ok(FetchGoogleUrgentEmailCandidatesResponse {
            candidates,
            attested_identity,
        })
    }

    pub async fn resolve_active_google_connector_request(
        &self,
        user_id: uuid::Uuid,
    ) -> Result<ConnectorSecretRequest, EnclaveRpcError> {
        let connector = self
            .store
            .list_active_connector_metadata(user_id)
            .await
            .map_err(|err| EnclaveRpcError::ConnectorTokenDecryptFailed {
                message: err.to_string(),
            })?
            .into_iter()
            .find(|connector| connector.provider == "google")
            .ok_or(EnclaveRpcError::ConnectorTokenUnavailable)?;

        if connector.token_key_id != self.secret_runtime.kms_key_id()
            || connector.token_version != self.secret_runtime.kms_key_version()
        {
            match self
                .store
                .ensure_active_connector_key_metadata(
                    user_id,
                    connector.connector_id,
                    self.secret_runtime.kms_key_id(),
                    self.secret_runtime.kms_key_version(),
                )
                .await
            {
                Ok(Some(_)) => {}
                Ok(None) => return Err(EnclaveRpcError::ConnectorTokenUnavailable),
                Err(err) => {
                    return Err(EnclaveRpcError::ConnectorTokenDecryptFailed {
                        message: err.to_string(),
                    });
                }
            }
        }

        Ok(ConnectorSecretRequest {
            user_id,
            connector_id: connector.connector_id,
        })
    }

    async fn exchange_access_token(&self, refresh_token: &str) -> Result<String, EnclaveRpcError> {
        let response = self
            .http_client
            .post(&self.oauth.token_url)
            .form(&[
                ("grant_type", "refresh_token"),
                ("client_id", self.oauth.client_id.as_str()),
                ("client_secret", self.oauth.client_secret.as_str()),
                ("refresh_token", refresh_token),
            ])
            .send()
            .await
            .map_err(|err| EnclaveRpcError::ProviderRequestUnavailable {
                operation: ProviderOperation::TokenRefresh,
                message: err.to_string(),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(EnclaveRpcError::ProviderRequestFailed {
                operation: ProviderOperation::TokenRefresh,
                status: status.as_u16(),
                oauth_error: parse_google_error_code(&body),
            });
        }

        let payload = response
            .json::<GoogleRefreshTokenResponse>()
            .await
            .map_err(|err| EnclaveRpcError::ProviderResponseInvalid {
                operation: ProviderOperation::TokenRefresh,
                message: err.to_string(),
            })?;

        Ok(payload.access_token)
    }

    async fn send_google_json_request<T>(
        &self,
        request: RequestBuilder,
        operation: ProviderOperation,
    ) -> Result<T, EnclaveRpcError>
    where
        T: DeserializeOwned,
    {
        let response =
            request
                .send()
                .await
                .map_err(|err| EnclaveRpcError::ProviderRequestUnavailable {
                    operation,
                    message: err.to_string(),
                })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(EnclaveRpcError::ProviderRequestFailed {
                operation,
                status: status.as_u16(),
                oauth_error: parse_google_error_code(&body),
            });
        }

        response
            .json::<T>()
            .await
            .map_err(|err| EnclaveRpcError::ProviderResponseInvalid {
                operation,
                message: err.to_string(),
            })
    }

    async fn load_authorized_refresh_token(
        &self,
        request: &ConnectorSecretRequest,
    ) -> Result<(String, AttestedIdentityPayload), EnclaveRpcError> {
        let connector_metadata = self
            .store
            .get_active_connector_key_metadata(request.user_id, request.connector_id)
            .await
            .map_err(|err| EnclaveRpcError::ConnectorTokenDecryptFailed {
                message: err.to_string(),
            })?
            .ok_or(EnclaveRpcError::ConnectorTokenUnavailable)?;

        let attested_identity = self
            .secret_runtime
            .authorize_connector_decrypt(&AuthorizedConnectorKeyMetadata {
                key_id: connector_metadata.token_key_id.clone(),
                key_version: connector_metadata.token_version,
            })
            .await
            .map_err(|err| EnclaveRpcError::DecryptNotAuthorized {
                message: err.to_string(),
            })?;

        let refresh_token = self
            .store
            .decrypt_active_connector_refresh_token(
                request.user_id,
                request.connector_id,
                &PersistedConnectorKeyMetadata {
                    provider: connector_metadata.provider,
                    token_key_id: connector_metadata.token_key_id,
                    token_version: connector_metadata.token_version,
                },
            )
            .await
            .map_err(|err| EnclaveRpcError::ConnectorTokenDecryptFailed {
                message: err.to_string(),
            })?
            .ok_or(EnclaveRpcError::ConnectorTokenUnavailable)?;

        Ok((
            refresh_token,
            AttestedIdentityPayload {
                runtime: attested_identity.runtime,
                measurement: attested_identity.measurement,
            },
        ))
    }
}
