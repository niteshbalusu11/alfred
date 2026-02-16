use std::collections::HashMap;

use chrono::{Duration as ChronoDuration, Utc};
use shared::config::WorkerConfig;
use shared::llm::LlmGateway;
use shared::models::Preferences;
use shared::repos::{ClaimedJob, JobType, Store};
use shared::security::SecretRuntime;

use super::JobActionResult;
use crate::{JobExecutionError, NotificationContent};

mod fetch;
mod morning_brief;
mod session;
mod util;

use fetch::fetch_calendar_events;
use session::build_google_session;
use util::{first_timed_event, truncate_for_notification};

pub(super) async fn resolve_job_action(
    store: &Store,
    config: &WorkerConfig,
    secret_runtime: &SecretRuntime,
    oauth_client: &reqwest::Client,
    llm_gateway: &dyn LlmGateway,
    job: &ClaimedJob,
    preferences: &Preferences,
) -> Result<JobActionResult, JobExecutionError> {
    match job.job_type {
        JobType::MeetingReminder => {
            let session =
                build_google_session(store, config, secret_runtime, oauth_client, job.user_id)
                    .await?;
            build_meeting_reminder(
                oauth_client,
                &session.access_token,
                &session.attested_measurement,
                preferences.meeting_reminder_minutes,
            )
            .await
        }
        JobType::MorningBrief => {
            morning_brief::build_morning_brief(
                store,
                config,
                secret_runtime,
                oauth_client,
                llm_gateway,
                job.user_id,
                preferences,
            )
            .await
        }
        JobType::UrgentEmailCheck => build_urgent_email_alert().await,
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

async fn build_urgent_email_alert() -> Result<JobActionResult, JobExecutionError> {
    let mut metadata = HashMap::new();
    metadata.insert(
        "action_source".to_string(),
        "urgent_email_llm_orchestrator".to_string(),
    );
    metadata.insert(
        "reason".to_string(),
        "llm_orchestration_pending".to_string(),
    );
    metadata.insert(
        "attested_measurement".to_string(),
        "not_requested_for_llm_pending_path".to_string(),
    );

    Ok(JobActionResult {
        notification: None,
        metadata,
    })
}
