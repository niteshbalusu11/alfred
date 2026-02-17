use chrono::{DateTime, SecondsFormat, TimeZone, Utc};
use serde::Deserialize;

use crate::enclave::EnclaveGoogleEmailCandidate;

#[derive(Debug, Deserialize)]
pub(super) struct GoogleRefreshTokenResponse {
    pub(super) access_token: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct GoogleOAuthCodeExchangeResponse {
    pub(super) refresh_token: Option<String>,
    pub(super) scope: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct GoogleCalendarEventsResponse {
    #[serde(default)]
    pub(super) items: Vec<GoogleCalendarEvent>,
}

#[derive(Debug, Deserialize)]
pub(super) struct GoogleCalendarEvent {
    pub(super) id: Option<String>,
    pub(super) summary: Option<String>,
    pub(super) start: Option<GoogleCalendarEventDateTime>,
    pub(super) end: Option<GoogleCalendarEventDateTime>,
    #[serde(default)]
    pub(super) attendees: Vec<GoogleCalendarAttendee>,
}

#[derive(Debug, Deserialize)]
pub(super) struct GoogleCalendarEventDateTime {
    #[serde(rename = "dateTime")]
    pub(super) date_time: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct GoogleCalendarAttendee {
    pub(super) email: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct GmailMessagesResponse {
    #[serde(default)]
    pub(super) messages: Vec<GmailMessageListEntry>,
}

#[derive(Debug, Deserialize)]
pub(super) struct GmailMessageListEntry {
    pub(super) id: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct GmailMessageMetadataResponse {
    id: String,
    snippet: Option<String>,
    #[serde(rename = "internalDate")]
    internal_date: Option<String>,
    #[serde(default, rename = "labelIds")]
    label_ids: Vec<String>,
    payload: Option<GmailMessagePayload>,
}

impl GmailMessageMetadataResponse {
    pub(super) fn into_candidate(self) -> EnclaveGoogleEmailCandidate {
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

pub(super) fn parse_google_error_code(body: &str) -> Option<String> {
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
