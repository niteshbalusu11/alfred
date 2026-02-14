use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::{FailureClass, JobExecutionError};

const GOOGLE_CALENDAR_EVENTS_URL: &str =
    "https://www.googleapis.com/calendar/v3/calendars/primary/events";
const GOOGLE_GMAIL_MESSAGES_URL: &str = "https://gmail.googleapis.com/gmail/v1/users/me/messages";

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

pub(super) async fn fetch_unread_email_count(
    oauth_client: &reqwest::Client,
    access_token: &str,
) -> Result<usize, JobExecutionError> {
    let request = oauth_client
        .get(GOOGLE_GMAIL_MESSAGES_URL)
        .bearer_auth(access_token)
        .query(&[("q", "is:unread newer_than:1d"), ("maxResults", "1")]);
    let payload: GoogleGmailListResponse = send_google_request(
        request,
        "GOOGLE_GMAIL_UNAVAILABLE",
        "gmail request failed",
        "GOOGLE_GMAIL_FAILED",
        "gmail request failed",
        "GOOGLE_GMAIL_PARSE_FAILED",
        "gmail response was invalid",
    )
    .await?;

    Ok(payload.result_size_estimate)
}

pub(super) async fn fetch_gmail_messages(
    oauth_client: &reqwest::Client,
    access_token: &str,
    query: &str,
    max_results: usize,
) -> Result<Vec<GoogleGmailMessageRef>, JobExecutionError> {
    let max_results = max_results.to_string();
    let request = oauth_client
        .get(GOOGLE_GMAIL_MESSAGES_URL)
        .bearer_auth(access_token)
        .query(&[("q", query), ("maxResults", &max_results)]);
    let payload: GoogleGmailListResponse = send_google_request(
        request,
        "GOOGLE_GMAIL_UNAVAILABLE",
        "gmail request failed",
        "GOOGLE_GMAIL_FAILED",
        "gmail request failed",
        "GOOGLE_GMAIL_PARSE_FAILED",
        "gmail response was invalid",
    )
    .await?;

    Ok(payload.messages)
}

pub(super) async fn fetch_gmail_message_detail(
    oauth_client: &reqwest::Client,
    access_token: &str,
    message_id: &str,
) -> Result<GoogleGmailMessageDetail, JobExecutionError> {
    let url = format!("{GOOGLE_GMAIL_MESSAGES_URL}/{message_id}");
    let request = oauth_client.get(url).bearer_auth(access_token).query(&[
        ("format", "metadata"),
        ("metadataHeaders", "Subject"),
        ("metadataHeaders", "From"),
    ]);
    send_google_request(
        request,
        "GOOGLE_GMAIL_MESSAGE_UNAVAILABLE",
        "gmail message request failed",
        "GOOGLE_GMAIL_MESSAGE_FAILED",
        "gmail message request failed",
        "GOOGLE_GMAIL_MESSAGE_PARSE_FAILED",
        "gmail message response was invalid",
    )
    .await
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
pub(super) struct GoogleCalendarEvent {
    pub(super) id: Option<String>,
    pub(super) summary: Option<String>,
    pub(super) start: Option<GoogleCalendarEventStart>,
}

#[derive(Debug, Deserialize)]
pub(super) struct GoogleCalendarEventStart {
    #[serde(rename = "dateTime")]
    pub(super) date_time: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoogleGmailListResponse {
    #[serde(default)]
    messages: Vec<GoogleGmailMessageRef>,
    #[serde(rename = "resultSizeEstimate", default)]
    result_size_estimate: usize,
}

#[derive(Debug, Deserialize)]
pub(super) struct GoogleGmailMessageRef {
    pub(super) id: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct GoogleGmailMessageDetail {
    pub(super) id: String,
    #[serde(rename = "labelIds", default)]
    pub(super) label_ids: Vec<String>,
    #[serde(default)]
    pub(super) snippet: String,
    pub(super) payload: Option<GoogleGmailPayload>,
}

#[derive(Debug, Deserialize)]
pub(super) struct GoogleGmailPayload {
    #[serde(default)]
    pub(super) headers: Vec<GoogleGmailHeader>,
}

#[derive(Debug, Deserialize)]
pub(super) struct GoogleGmailHeader {
    pub(super) name: String,
    pub(super) value: String,
}
