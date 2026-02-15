use chrono::{DateTime, Days, NaiveDate, Utc};
use serde::Deserialize;
use shared::llm::GoogleCalendarMeetingSource;

use super::super::errors::bad_gateway_response;

const GOOGLE_CALENDAR_EVENTS_URL: &str =
    "https://www.googleapis.com/calendar/v3/calendars/primary/events";
const MAX_CALENDAR_EVENTS: usize = 25;

pub(super) async fn fetch_meetings_for_day(
    http_client: &reqwest::Client,
    access_token: &str,
    calendar_day: NaiveDate,
) -> Result<Vec<GoogleCalendarMeetingSource>, axum::response::Response> {
    let Some(start_of_day) = calendar_day.and_hms_opt(0, 0, 0) else {
        return Ok(Vec::new());
    };
    let Some(next_day) = calendar_day.checked_add_days(Days::new(1)) else {
        return Ok(Vec::new());
    };
    let Some(start_of_next_day) = next_day.and_hms_opt(0, 0, 0) else {
        return Ok(Vec::new());
    };

    let time_min = DateTime::<Utc>::from_naive_utc_and_offset(start_of_day, Utc).to_rfc3339();
    let time_max = DateTime::<Utc>::from_naive_utc_and_offset(start_of_next_day, Utc).to_rfc3339();
    let max_results = MAX_CALENDAR_EVENTS.to_string();

    let response = match http_client
        .get(GOOGLE_CALENDAR_EVENTS_URL)
        .bearer_auth(access_token)
        .query(&[
            ("singleEvents", "true"),
            ("orderBy", "startTime"),
            ("timeMin", time_min.as_str()),
            ("timeMax", time_max.as_str()),
            ("maxResults", max_results.as_str()),
        ])
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => {
            return Err(bad_gateway_response(
                "google_calendar_unavailable",
                "Unable to reach Google Calendar endpoint",
            ));
        }
    };

    if !response.status().is_success() {
        return Err(bad_gateway_response(
            "google_calendar_failed",
            "Google Calendar request failed",
        ));
    }

    let payload: GoogleCalendarEventsResponse = match response.json().await {
        Ok(payload) => payload,
        Err(_) => {
            return Err(bad_gateway_response(
                "google_calendar_invalid_response",
                "Google Calendar response was invalid",
            ));
        }
    };

    Ok(payload
        .items
        .into_iter()
        .map(GoogleCalendarMeetingSource::from)
        .collect())
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

impl From<GoogleCalendarEvent> for GoogleCalendarMeetingSource {
    fn from(event: GoogleCalendarEvent) -> Self {
        Self {
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
}

fn parse_utc_datetime(value: Option<String>) -> Option<DateTime<Utc>> {
    let value = value?;
    DateTime::parse_from_rfc3339(&value)
        .ok()
        .map(|parsed| parsed.with_timezone(&Utc))
}
