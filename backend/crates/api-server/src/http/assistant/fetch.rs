use chrono::{DateTime, NaiveDate, Utc};
use shared::enclave::{
    ConnectorSecretRequest, EnclaveGoogleCalendarEvent, EnclaveRpcClient, EnclaveRpcError,
    ProviderOperation,
};
use shared::llm::GoogleCalendarMeetingSource;
use shared::timezone::local_day_bounds_utc;

use super::super::errors::{
    bad_gateway_response, bad_request_response, decrypt_not_authorized_response,
};

const MAX_CALENDAR_EVENTS: usize = 25;

pub(super) async fn fetch_meetings_for_day(
    enclave_client: &EnclaveRpcClient,
    connector_request: ConnectorSecretRequest,
    calendar_day: NaiveDate,
    time_zone: &str,
) -> Result<Vec<GoogleCalendarMeetingSource>, axum::response::Response> {
    let Some((start_utc, end_utc)) = local_day_bounds_utc(calendar_day, time_zone) else {
        return Ok(Vec::new());
    };

    let response = enclave_client
        .fetch_google_calendar_events(
            connector_request,
            start_utc.to_rfc3339(),
            end_utc.to_rfc3339(),
            MAX_CALENDAR_EVENTS,
        )
        .await
        .map_err(map_calendar_fetch_error)?;

    Ok(response
        .events
        .into_iter()
        .map(map_calendar_event)
        .collect())
}

fn map_calendar_fetch_error(err: EnclaveRpcError) -> axum::response::Response {
    match err {
        EnclaveRpcError::DecryptNotAuthorized { .. } => decrypt_not_authorized_response(),
        EnclaveRpcError::ConnectorTokenDecryptFailed { .. } => bad_gateway_response(
            "connector_token_decrypt_failed",
            "Connector token decrypt failed",
        ),
        EnclaveRpcError::ConnectorTokenUnavailable => bad_request_response(
            "connector_token_unavailable",
            "Connector token metadata changed; retry the request",
        ),
        EnclaveRpcError::ProviderRequestUnavailable { operation, .. } => match operation {
            ProviderOperation::TokenRefresh => bad_gateway_response(
                "google_token_refresh_unavailable",
                "Unable to reach Google OAuth token endpoint",
            ),
            ProviderOperation::CalendarFetch => bad_gateway_response(
                "google_calendar_unavailable",
                "Unable to reach Google Calendar endpoint",
            ),
            ProviderOperation::TokenRevoke | ProviderOperation::GmailFetch => bad_gateway_response(
                "google_calendar_unavailable",
                "Google Calendar is unavailable",
            ),
        },
        EnclaveRpcError::ProviderRequestFailed { operation, .. } => match operation {
            ProviderOperation::TokenRefresh => bad_gateway_response(
                "google_token_refresh_failed",
                "Google OAuth token refresh failed",
            ),
            ProviderOperation::CalendarFetch => {
                bad_gateway_response("google_calendar_failed", "Google Calendar request failed")
            }
            ProviderOperation::TokenRevoke | ProviderOperation::GmailFetch => {
                bad_gateway_response("google_calendar_failed", "Google Calendar request failed")
            }
        },
        EnclaveRpcError::ProviderResponseInvalid { operation, .. } => match operation {
            ProviderOperation::TokenRefresh => bad_gateway_response(
                "google_token_refresh_failed",
                "Google OAuth token refresh failed",
            ),
            ProviderOperation::CalendarFetch => bad_gateway_response(
                "google_calendar_invalid_response",
                "Google Calendar response was invalid",
            ),
            ProviderOperation::TokenRevoke | ProviderOperation::GmailFetch => bad_gateway_response(
                "google_calendar_invalid_response",
                "Google Calendar response was invalid",
            ),
        },
        EnclaveRpcError::RpcUnauthorized { .. }
        | EnclaveRpcError::RpcContractRejected { .. }
        | EnclaveRpcError::RpcTransportUnavailable { .. }
        | EnclaveRpcError::RpcResponseInvalid { .. } => {
            bad_gateway_response("enclave_rpc_failed", "Secure enclave RPC request failed")
        }
    }
}

fn map_calendar_event(event: EnclaveGoogleCalendarEvent) -> GoogleCalendarMeetingSource {
    GoogleCalendarMeetingSource {
        event_id: event.id,
        title: event.summary,
        start_at: parse_utc_datetime(event.start.and_then(|start| start.date_time)),
        end_at: parse_utc_datetime(event.end.and_then(|end| end.date_time)),
        attendee_emails: event
            .attendees
            .into_iter()
            .filter_map(|attendee| attendee.email)
            .collect(),
    }
}

fn parse_utc_datetime(value: Option<String>) -> Option<DateTime<Utc>> {
    let value = value?;
    DateTime::parse_from_rfc3339(&value)
        .ok()
        .map(|parsed| parsed.with_timezone(&Utc))
}
