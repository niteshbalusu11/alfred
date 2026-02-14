use std::collections::HashMap;

use chrono::{Duration as ChronoDuration, Utc};
use shared::config::WorkerConfig;
use shared::models::Preferences;
use shared::repos::{ClaimedJob, JobType, Store};
use shared::security::SecretRuntime;

use super::JobActionResult;
use crate::{JobExecutionError, NotificationContent};

mod fetch;
mod session;
mod util;

use fetch::{
    fetch_calendar_events, fetch_gmail_message_detail, fetch_gmail_messages,
    fetch_unread_email_count,
};
use session::build_google_session;
use util::{classify_urgent_message, first_timed_event, message_header, truncate_for_notification};

pub(super) async fn resolve_job_action(
    store: &Store,
    config: &WorkerConfig,
    secret_runtime: &SecretRuntime,
    oauth_client: &reqwest::Client,
    job: &ClaimedJob,
    preferences: &Preferences,
) -> Result<JobActionResult, JobExecutionError> {
    let session =
        build_google_session(store, config, secret_runtime, oauth_client, job.user_id).await?;

    match job.job_type {
        JobType::MeetingReminder => {
            build_meeting_reminder(
                oauth_client,
                &session.access_token,
                &session.attested_measurement,
                preferences.meeting_reminder_minutes,
            )
            .await
        }
        JobType::MorningBrief => {
            build_morning_brief(
                oauth_client,
                &session.access_token,
                &session.attested_measurement,
                preferences,
            )
            .await
        }
        JobType::UrgentEmailCheck => {
            build_urgent_email_alert(
                oauth_client,
                &session.access_token,
                &session.attested_measurement,
            )
            .await
        }
    }
}

async fn build_meeting_reminder(
    oauth_client: &reqwest::Client,
    access_token: &str,
    attested_measurement: &str,
    reminder_minutes: u32,
) -> Result<JobActionResult, JobExecutionError> {
    let now = Utc::now();
    let lead_minutes = reminder_minutes.max(1);
    let time_max = now + ChronoDuration::minutes(i64::from(lead_minutes));
    let events = fetch_calendar_events(oauth_client, access_token, now, time_max, 5).await?;

    let mut metadata = HashMap::new();
    metadata.insert("action_source".to_string(), "google_calendar".to_string());
    metadata.insert("lead_minutes".to_string(), lead_minutes.to_string());
    metadata.insert("events_found".to_string(), events.len().to_string());
    metadata.insert(
        "attested_measurement".to_string(),
        attested_measurement.to_string(),
    );

    let Some(event) = first_timed_event(events) else {
        metadata.insert("reason".to_string(), "no_upcoming_event".to_string());
        return Ok(JobActionResult {
            notification: None,
            metadata,
        });
    };

    let minutes_until = (event.start_at - now).num_minutes().max(0);
    let title = if minutes_until <= 1 {
        "Meeting now".to_string()
    } else {
        format!("Meeting in {minutes_until} min")
    };

    let event_summary = event
        .summary
        .unwrap_or_else(|| "Upcoming meeting".to_string());
    let body = format!(
        "{} at {}",
        truncate_for_notification(&event_summary, 80),
        event.start_at.format("%H:%M UTC")
    );

    metadata.insert("selected_event_id".to_string(), event.id);

    Ok(JobActionResult {
        notification: Some(NotificationContent { title, body }),
        metadata,
    })
}

async fn build_morning_brief(
    oauth_client: &reqwest::Client,
    access_token: &str,
    attested_measurement: &str,
    preferences: &Preferences,
) -> Result<JobActionResult, JobExecutionError> {
    let now = Utc::now();
    let tomorrow = now + ChronoDuration::hours(24);

    let events = fetch_calendar_events(oauth_client, access_token, now, tomorrow, 10).await?;
    let unread_count = fetch_unread_email_count(oauth_client, access_token).await?;

    let title = "Morning brief".to_string();
    let body = format!(
        "Today: {} upcoming meetings, {} unread emails.",
        events.len(),
        unread_count
    );

    let mut metadata = HashMap::new();
    metadata.insert(
        "action_source".to_string(),
        "google_calendar_gmail".to_string(),
    );
    metadata.insert("upcoming_events".to_string(), events.len().to_string());
    metadata.insert("unread_emails".to_string(), unread_count.to_string());
    metadata.insert(
        "morning_brief_local_time".to_string(),
        preferences.morning_brief_local_time.clone(),
    );
    metadata.insert(
        "attested_measurement".to_string(),
        attested_measurement.to_string(),
    );

    Ok(JobActionResult {
        notification: Some(NotificationContent { title, body }),
        metadata,
    })
}

async fn build_urgent_email_alert(
    oauth_client: &reqwest::Client,
    access_token: &str,
    attested_measurement: &str,
) -> Result<JobActionResult, JobExecutionError> {
    let messages =
        fetch_gmail_messages(oauth_client, access_token, "is:unread newer_than:2d", 15).await?;

    let mut metadata = HashMap::new();
    metadata.insert("action_source".to_string(), "gmail_rule_engine".to_string());
    metadata.insert("messages_scanned".to_string(), messages.len().to_string());
    metadata.insert(
        "attested_measurement".to_string(),
        attested_measurement.to_string(),
    );

    for message_ref in messages.into_iter().take(10) {
        let detail =
            fetch_gmail_message_detail(oauth_client, access_token, &message_ref.id).await?;

        if let Some(rule_match) = classify_urgent_message(&detail) {
            let subject = message_header(&detail, "Subject").unwrap_or("Urgent email");
            let sender = message_header(&detail, "From").unwrap_or("Unknown sender");

            metadata.insert("matched_message_id".to_string(), detail.id.clone());
            metadata.insert("matched_rule".to_string(), rule_match.to_string());

            return Ok(JobActionResult {
                notification: Some(NotificationContent {
                    title: "Urgent email alert".to_string(),
                    body: format!(
                        "{}: {}",
                        truncate_for_notification(sender, 45),
                        truncate_for_notification(subject, 90)
                    ),
                }),
                metadata,
            });
        }
    }

    metadata.insert("reason".to_string(), "no_urgent_match".to_string());
    Ok(JobActionResult {
        notification: None,
        metadata,
    })
}
