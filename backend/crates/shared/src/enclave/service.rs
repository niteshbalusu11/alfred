use chrono::{DateTime, SecondsFormat, TimeZone, Utc};
use reqwest::{RequestBuilder, StatusCode};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::repos::{ConnectorKeyMetadata as PersistedConnectorKeyMetadata, Store};
use crate::security::{ConnectorKeyMetadata as AuthorizedConnectorKeyMetadata, SecretRuntime};

use super::{
    AttestedIdentityPayload, ConnectorSecretRequest, EnclaveGoogleCalendarAttendee,
    EnclaveGoogleCalendarEvent, EnclaveGoogleCalendarEventDateTime, EnclaveGoogleEmailCandidate,
    EnclaveRpcError, ExchangeGoogleTokenResponse, FetchGoogleCalendarEventsResponse,
    FetchGoogleUrgentEmailCandidatesResponse, GoogleEnclaveOauthConfig, ProviderOperation,
    RevokeGoogleTokenResponse,
};

const GOOGLE_CALENDAR_EVENTS_URL: &str =
    "https://www.googleapis.com/calendar/v3/calendars/primary/events";
const GMAIL_MESSAGES_URL: &str = "https://gmail.googleapis.com/gmail/v1/users/me/messages";
const MAX_GMAIL_CANDIDATES: usize = 50;

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
        let (refresh_token, attested_identity) =
            self.load_authorized_refresh_token(&request).await?;
        let access_token = self.exchange_access_token(&refresh_token).await?;
        let max_results = max_results.clamp(1, MAX_GMAIL_CANDIDATES).to_string();

        let payload: GmailMessagesResponse = self
            .send_google_json_request(
                self.http_client
                    .get(GMAIL_MESSAGES_URL)
                    .bearer_auth(&access_token)
                    .query(&[
                        ("labelIds", "INBOX"),
                        ("q", "newer_than:2d"),
                        ("maxResults", max_results.as_str()),
                    ]),
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

#[derive(Debug, Deserialize)]
struct GoogleRefreshTokenResponse {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct GoogleCalendarEventsResponse {
    #[serde(default)]
    items: Vec<GoogleCalendarEvent>,
}

#[derive(Debug, Deserialize)]
struct GoogleCalendarEvent {
    id: Option<String>,
    summary: Option<String>,
    start: Option<GoogleCalendarEventDateTime>,
    end: Option<GoogleCalendarEventDateTime>,
    #[serde(default)]
    attendees: Vec<GoogleCalendarAttendee>,
}

#[derive(Debug, Deserialize)]
struct GoogleCalendarEventDateTime {
    #[serde(rename = "dateTime")]
    date_time: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoogleCalendarAttendee {
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GmailMessagesResponse {
    #[serde(default)]
    messages: Vec<GmailMessageListEntry>,
}

#[derive(Debug, Deserialize)]
struct GmailMessageListEntry {
    id: String,
}

#[derive(Debug, Deserialize)]
struct GmailMessageMetadataResponse {
    id: String,
    snippet: Option<String>,
    #[serde(rename = "internalDate")]
    internal_date: Option<String>,
    #[serde(default, rename = "labelIds")]
    label_ids: Vec<String>,
    payload: Option<GmailMessagePayload>,
}

impl GmailMessageMetadataResponse {
    fn into_candidate(self) -> EnclaveGoogleEmailCandidate {
        let has_attachments = self.payload.as_ref().is_some_and(payload_has_attachments);
        let from = self
            .payload
            .as_ref()
            .and_then(|payload| payload.header_value("From"));
        let subject = self
            .payload
            .as_ref()
            .and_then(|payload| payload.header_value("Subject"));

        EnclaveGoogleEmailCandidate {
            message_id: Some(self.id),
            from,
            subject,
            snippet: self.snippet,
            received_at: self
                .internal_date
                .as_deref()
                .and_then(parse_internal_date_millis)
                .map(|value| value.to_rfc3339_opts(SecondsFormat::Secs, true)),
            label_ids: self.label_ids,
            has_attachments,
        }
    }
}

#[derive(Debug, Deserialize)]
struct GmailMessagePayload {
    #[serde(default)]
    headers: Vec<GmailMessageHeader>,
    #[serde(default)]
    parts: Vec<GmailMessagePayload>,
    #[serde(default)]
    filename: String,
    body: Option<GmailMessageBody>,
}

impl GmailMessagePayload {
    fn header_value(&self, target_name: &str) -> Option<String> {
        self.headers
            .iter()
            .find(|header| header.name.eq_ignore_ascii_case(target_name))
            .map(|header| header.value.trim().to_string())
            .filter(|value| !value.is_empty())
    }
}

#[derive(Debug, Deserialize)]
struct GmailMessageHeader {
    name: String,
    value: String,
}

#[derive(Debug, Deserialize)]
struct GmailMessageBody {
    #[serde(rename = "attachmentId")]
    attachment_id: Option<String>,
}

fn payload_has_attachments(payload: &GmailMessagePayload) -> bool {
    let has_attachment_id = payload
        .body
        .as_ref()
        .and_then(|body| body.attachment_id.as_ref())
        .is_some();
    if has_attachment_id || !payload.filename.trim().is_empty() {
        return true;
    }

    payload.parts.iter().any(payload_has_attachments)
}

fn parse_internal_date_millis(raw: &str) -> Option<DateTime<Utc>> {
    let millis = raw.parse::<i64>().ok()?;
    Utc.timestamp_millis_opt(millis).single()
}

#[derive(Debug, Deserialize)]
struct GoogleOAuthErrorResponse {
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoogleApiErrorEnvelope {
    error: Option<GoogleApiErrorBody>,
}

#[derive(Debug, Deserialize)]
struct GoogleApiErrorBody {
    status: Option<String>,
    message: Option<String>,
}

fn parse_google_error_code(body: &str) -> Option<String> {
    if let Ok(parsed) = serde_json::from_str::<GoogleOAuthErrorResponse>(body)
        && let Some(error) = parsed.error
        && !error.trim().is_empty()
    {
        return Some(error);
    }

    if let Ok(parsed) = serde_json::from_str::<GoogleApiErrorEnvelope>(body)
        && let Some(error) = parsed.error
    {
        if let Some(status) = error.status
            && !status.trim().is_empty()
        {
            return Some(status);
        }
        if let Some(message) = error.message
            && !message.trim().is_empty()
        {
            return Some(message);
        }
    }

    None
}
