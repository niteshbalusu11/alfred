use chrono::{DateTime, TimeZone, Utc};
use reqwest::StatusCode;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use shared::llm::GoogleEmailCandidateSource;

use crate::{FailureClass, JobExecutionError};

const GOOGLE_CALENDAR_EVENTS_URL: &str =
    "https://www.googleapis.com/calendar/v3/calendars/primary/events";
const GMAIL_MESSAGES_URL: &str = "https://gmail.googleapis.com/gmail/v1/users/me/messages";

pub(super) async fn fetch_calendar_events(
    oauth_client: &reqwest::Client,
    access_token: &str,
    time_min: DateTime<Utc>,
    time_max: DateTime<Utc>,
    max_results: usize,
) -> Result<Vec<GoogleCalendarEvent>, JobExecutionError> {
    let time_min = time_min.to_rfc3339();
    let time_max = time_max.to_rfc3339();
    let max_results = max_results.to_string();
    let request = oauth_client
        .get(GOOGLE_CALENDAR_EVENTS_URL)
        .bearer_auth(access_token)
        .query(&[
            ("singleEvents", "true"),
            ("orderBy", "startTime"),
            ("timeMin", &time_min),
            ("timeMax", &time_max),
            ("maxResults", &max_results),
        ]);
    let payload: GoogleCalendarEventsResponse = send_google_request(
        request,
        "GOOGLE_CALENDAR_UNAVAILABLE",
        "calendar request failed",
        "GOOGLE_CALENDAR_FAILED",
        "calendar request failed",
        "GOOGLE_CALENDAR_PARSE_FAILED",
        "calendar response was invalid",
    )
    .await?;

    Ok(payload.items)
}

pub(super) async fn fetch_urgent_email_candidates(
    oauth_client: &reqwest::Client,
    access_token: &str,
    max_results: usize,
) -> Result<Vec<GoogleEmailCandidateSource>, JobExecutionError> {
    let max_results = max_results.clamp(1, 50).to_string();
    let request = oauth_client
        .get(GMAIL_MESSAGES_URL)
        .bearer_auth(access_token)
        .query(&[
            ("labelIds", "INBOX"),
            ("q", "newer_than:2d"),
            ("maxResults", max_results.as_str()),
        ]);
    let payload: GmailMessagesResponse = send_google_request(
        request,
        "GMAIL_UNAVAILABLE",
        "gmail list request failed",
        "GMAIL_MESSAGES_FAILED",
        "gmail list request failed",
        "GMAIL_MESSAGES_PARSE_FAILED",
        "gmail list response was invalid",
    )
    .await?;

    let mut candidates = Vec::with_capacity(payload.messages.len());
    for message in payload.messages {
        let request = oauth_client
            .get(format!("{GMAIL_MESSAGES_URL}/{}", message.id))
            .bearer_auth(access_token)
            .query(&[
                ("format", "metadata"),
                ("metadataHeaders", "From"),
                ("metadataHeaders", "Subject"),
            ]);
        let details: GmailMessageMetadataResponse = send_google_request(
            request,
            "GMAIL_UNAVAILABLE",
            "gmail message request failed",
            "GMAIL_MESSAGE_FAILED",
            "gmail message request failed",
            "GMAIL_MESSAGE_PARSE_FAILED",
            "gmail message response was invalid",
        )
        .await?;
        candidates.push(details.into_candidate());
    }

    Ok(candidates)
}

async fn send_google_request<T>(
    request: reqwest::RequestBuilder,
    unavailable_code: &str,
    unavailable_message: &str,
    failed_code: &str,
    failed_message_prefix: &str,
    parse_code: &str,
    parse_message: &str,
) -> Result<T, JobExecutionError>
where
    T: DeserializeOwned,
{
    let response = request.send().await.map_err(|err| {
        JobExecutionError::transient(unavailable_code, format!("{unavailable_message}: {err}"))
    })?;

    if !response.status().is_success() {
        let status = response.status();
        return Err(classified_http_error(
            status,
            failed_code,
            format!("{failed_message_prefix} with HTTP {}", status.as_u16()),
        ));
    }

    response
        .json::<T>()
        .await
        .map_err(|err| JobExecutionError::transient(parse_code, format!("{parse_message}: {err}")))
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
struct GoogleCalendarEventsResponse {
    #[serde(default)]
    items: Vec<GoogleCalendarEvent>,
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
    fn into_candidate(self) -> GoogleEmailCandidateSource {
        let has_attachments = self.payload.as_ref().is_some_and(payload_has_attachments);
        let from = self
            .payload
            .as_ref()
            .and_then(|payload| payload.header_value("From"));
        let subject = self
            .payload
            .as_ref()
            .and_then(|payload| payload.header_value("Subject"));

        GoogleEmailCandidateSource {
            message_id: Some(self.id),
            from,
            subject,
            snippet: self.snippet,
            received_at: self
                .internal_date
                .as_deref()
                .and_then(parse_internal_date_millis),
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
