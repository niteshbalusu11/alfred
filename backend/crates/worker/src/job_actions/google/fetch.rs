use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use serde::Deserialize;
use shared::enclave::{
    ConnectorSecretRequest, EnclaveGoogleCalendarAttendee, EnclaveGoogleCalendarEvent,
    EnclaveGoogleCalendarEventDateTime, EnclaveGoogleEmailCandidate, EnclaveRpcClient,
    EnclaveRpcError, ProviderOperation,
};
use shared::llm::GoogleEmailCandidateSource;

use crate::{FailureClass, JobExecutionError};

pub(super) struct CalendarFetchOutcome {
    pub(super) events: Vec<GoogleCalendarEvent>,
    pub(super) attested_measurement: String,
}

pub(super) struct UrgentEmailFetchOutcome {
    pub(super) candidates: Vec<GoogleEmailCandidateSource>,
    pub(super) attested_measurement: String,
}

pub(super) async fn fetch_calendar_events(
    enclave_client: &EnclaveRpcClient,
    connector_request: ConnectorSecretRequest,
    time_min: DateTime<Utc>,
    time_max: DateTime<Utc>,
    max_results: usize,
) -> Result<CalendarFetchOutcome, JobExecutionError> {
    let response = enclave_client
        .fetch_google_calendar_events(
            connector_request,
            time_min.to_rfc3339(),
            time_max.to_rfc3339(),
            max_results,
        )
        .await
        .map_err(map_calendar_fetch_error)?;

    Ok(CalendarFetchOutcome {
        events: response.events.into_iter().map(Into::into).collect(),
        attested_measurement: response.attested_identity.measurement,
    })
}

pub(super) async fn fetch_urgent_email_candidates(
    enclave_client: &EnclaveRpcClient,
    connector_request: ConnectorSecretRequest,
    max_results: usize,
) -> Result<UrgentEmailFetchOutcome, JobExecutionError> {
    let response = enclave_client
        .fetch_google_urgent_email_candidates(connector_request, max_results)
        .await
        .map_err(map_gmail_fetch_error)?;

    Ok(UrgentEmailFetchOutcome {
        candidates: response
            .candidates
            .into_iter()
            .map(map_enclave_email_candidate)
            .collect(),
        attested_measurement: response.attested_identity.measurement,
    })
}

fn map_calendar_fetch_error(err: EnclaveRpcError) -> JobExecutionError {
    map_enclave_fetch_error(
        err,
        "GOOGLE_CALENDAR_UNAVAILABLE",
        "GOOGLE_CALENDAR_FAILED",
        "GOOGLE_CALENDAR_PARSE_FAILED",
    )
}

fn map_gmail_fetch_error(err: EnclaveRpcError) -> JobExecutionError {
    map_enclave_fetch_error(
        err,
        "GMAIL_UNAVAILABLE",
        "GMAIL_MESSAGES_FAILED",
        "GMAIL_MESSAGES_PARSE_FAILED",
    )
}

fn map_enclave_fetch_error(
    err: EnclaveRpcError,
    provider_unavailable_code: &str,
    provider_failed_code: &str,
    provider_parse_code: &str,
) -> JobExecutionError {
    match err {
        EnclaveRpcError::DecryptNotAuthorized { message } => JobExecutionError::permanent(
            "CONNECTOR_DECRYPT_NOT_AUTHORIZED",
            format!("decrypt authorization failed: {message}"),
        ),
        EnclaveRpcError::ConnectorTokenDecryptFailed { message } => JobExecutionError::transient(
            "CONNECTOR_TOKEN_DECRYPT_FAILED",
            format!("failed to decrypt refresh token: {message}"),
        ),
        EnclaveRpcError::ConnectorTokenUnavailable => JobExecutionError::permanent(
            "CONNECTOR_TOKEN_MISSING",
            "refresh token was unavailable for active connector",
        ),
        EnclaveRpcError::ProviderRequestUnavailable { operation, message } => match operation {
            ProviderOperation::TokenRefresh => JobExecutionError::transient(
                "GOOGLE_TOKEN_REFRESH_UNAVAILABLE",
                format!("google token refresh request failed: {message}"),
            ),
            ProviderOperation::CalendarFetch | ProviderOperation::GmailFetch => {
                JobExecutionError::transient(
                    provider_unavailable_code,
                    format!("provider request failed: {message}"),
                )
            }
            ProviderOperation::TokenRevoke => JobExecutionError::transient(
                provider_unavailable_code,
                format!("provider request failed: {message}"),
            ),
        },
        EnclaveRpcError::ProviderRequestFailed {
            operation,
            status,
            oauth_error,
        } => {
            let status = StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY);
            let message = match oauth_error {
                Some(error) => format!("provider request rejected: {error}"),
                None => format!("provider request failed with HTTP {}", status.as_u16()),
            };

            match operation {
                ProviderOperation::TokenRefresh => {
                    classified_http_error(status, "GOOGLE_TOKEN_REFRESH_FAILED", message)
                }
                ProviderOperation::CalendarFetch
                | ProviderOperation::GmailFetch
                | ProviderOperation::TokenRevoke => {
                    classified_http_error(status, provider_failed_code, message)
                }
            }
        }
        EnclaveRpcError::ProviderResponseInvalid { operation, message } => match operation {
            ProviderOperation::TokenRefresh => JobExecutionError::transient(
                "GOOGLE_TOKEN_REFRESH_PARSE_FAILED",
                format!("google token refresh response was invalid: {message}"),
            ),
            ProviderOperation::CalendarFetch
            | ProviderOperation::GmailFetch
            | ProviderOperation::TokenRevoke => JobExecutionError::transient(
                provider_parse_code,
                format!("provider response was invalid: {message}"),
            ),
        },
        EnclaveRpcError::RpcUnauthorized { code }
        | EnclaveRpcError::RpcContractRejected { code } => JobExecutionError::permanent(
            "ENCLAVE_RPC_REJECTED",
            format!("secure enclave rpc request rejected: {code}"),
        ),
        EnclaveRpcError::RpcTransportUnavailable { message }
        | EnclaveRpcError::RpcResponseInvalid { message } => JobExecutionError::transient(
            "ENCLAVE_RPC_UNAVAILABLE",
            format!("secure enclave rpc unavailable: {message}"),
        ),
    }
}

pub(super) fn classified_http_error(
    status: StatusCode,
    code: &str,
    message: String,
) -> JobExecutionError {
    match classify_http_failure(status) {
        FailureClass::Transient => JobExecutionError::transient(code, message),
        FailureClass::Permanent => JobExecutionError::permanent(code, message),
    }
}

fn classify_http_failure(status: StatusCode) -> FailureClass {
    match status.as_u16() {
        408 | 425 | 429 | 500 | 502 | 503 | 504 => FailureClass::Transient,
        _ => FailureClass::Permanent,
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct GoogleCalendarEvent {
    pub(super) id: Option<String>,
    pub(super) summary: Option<String>,
    pub(super) start: Option<GoogleCalendarEventStart>,
    pub(super) end: Option<GoogleCalendarEventStart>,
    #[serde(default)]
    pub(super) attendees: Vec<GoogleCalendarAttendee>,
}

#[derive(Debug, Deserialize)]
pub(super) struct GoogleCalendarEventStart {
    #[serde(rename = "dateTime")]
    pub(super) date_time: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct GoogleCalendarAttendee {
    pub(super) email: Option<String>,
}

impl From<EnclaveGoogleCalendarEvent> for GoogleCalendarEvent {
    fn from(event: EnclaveGoogleCalendarEvent) -> Self {
        Self {
            id: event.id,
            summary: event.summary,
            start: event.start.map(Into::into),
            end: event.end.map(Into::into),
            attendees: event.attendees.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<EnclaveGoogleCalendarEventDateTime> for GoogleCalendarEventStart {
    fn from(value: EnclaveGoogleCalendarEventDateTime) -> Self {
        Self {
            date_time: value.date_time,
        }
    }
}

impl From<EnclaveGoogleCalendarAttendee> for GoogleCalendarAttendee {
    fn from(value: EnclaveGoogleCalendarAttendee) -> Self {
        Self { email: value.email }
    }
}

fn map_enclave_email_candidate(value: EnclaveGoogleEmailCandidate) -> GoogleEmailCandidateSource {
    GoogleEmailCandidateSource {
        message_id: value.message_id,
        from: value.from,
        subject: value.subject,
        snippet: value.snippet,
        received_at: value.received_at.as_deref().and_then(parse_rfc3339_utc),
        label_ids: value.label_ids,
        has_attachments: value.has_attachments,
    }
}

fn parse_rfc3339_utc(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|parsed| parsed.with_timezone(&Utc))
}
