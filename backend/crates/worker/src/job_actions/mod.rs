use std::collections::HashMap;

use chrono::Utc;
use shared::config::WorkerConfig;
use shared::repos::{AuditResult, ClaimedJob, Store};
use shared::security::SecretRuntime;
use tracing::{info, warn};

use crate::{
    FailureClass, JobExecutionError, NotificationContent, PushSendError, PushSender,
    WorkerTickMetrics, apns_environment_label,
};

mod google;
mod helpers;

struct JobActionResult {
    notification: Option<NotificationContent>,
    metadata: HashMap<String, String>,
}

pub(super) async fn dispatch_job_action(
    store: &Store,
    config: &WorkerConfig,
    secret_runtime: &SecretRuntime,
    oauth_client: &reqwest::Client,
    push_sender: &PushSender,
    job: &ClaimedJob,
    metrics: &mut WorkerTickMetrics,
) -> Result<(), JobExecutionError> {
    if let Some(simulated_failure) =
        helpers::parse_simulated_failure(job.payload_ciphertext.as_deref())
    {
        return Err(simulated_failure);
    }
    let request_id = helpers::extract_request_id(job.payload_ciphertext.as_deref());

    let preferences = store
        .get_or_create_preferences(job.user_id)
        .await
        .map_err(|err| {
            JobExecutionError::transient(
                "PREFERENCES_READ_FAILED",
                format!("failed to read user preferences: {err}"),
            )
        })?;

    let mut action = if let Some(content) =
        helpers::parse_notification_payload(job.payload_ciphertext.as_deref())
    {
        let mut metadata = HashMap::new();
        metadata.insert(
            "action_source".to_string(),
            "payload_notification".to_string(),
        );
        JobActionResult {
            notification: Some(content),
            metadata,
        }
    } else {
        google::resolve_job_action(
            store,
            config,
            secret_runtime,
            oauth_client,
            job,
            &preferences,
        )
        .await?
    };

    action
        .metadata
        .insert("job_id".to_string(), job.id.to_string());
    action
        .metadata
        .insert("job_type".to_string(), job.job_type.as_str().to_string());
    if let Some(request_id) = request_id {
        action.metadata.insert("request_id".to_string(), request_id);
    }

    let Some(content) = action.notification.as_ref() else {
        let mut metadata = action.metadata.clone();
        metadata.insert("outcome".to_string(), "no_notification".to_string());

        record_notification_audit(
            store,
            job.user_id,
            "JOB_ACTION_SKIPPED",
            AuditResult::Success,
            metadata,
        )
        .await;

        return Ok(());
    };

    if helpers::is_within_quiet_hours(
        Utc::now().time(),
        &preferences.quiet_hours_start,
        &preferences.quiet_hours_end,
    )
    .map_err(|message| JobExecutionError::permanent("INVALID_QUIET_HOURS", message))?
    {
        metrics.push_quiet_hours_suppressed += 1;

        let mut metadata = action.metadata.clone();
        metadata.insert("reason".to_string(), "quiet_hours".to_string());
        metadata.insert(
            "quiet_hours_start".to_string(),
            preferences.quiet_hours_start.clone(),
        );
        metadata.insert(
            "quiet_hours_end".to_string(),
            preferences.quiet_hours_end.clone(),
        );

        record_notification_audit(
            store,
            job.user_id,
            "NOTIFICATION_SUPPRESSED",
            AuditResult::Success,
            metadata,
        )
        .await;

        info!(
            job_id = %job.id,
            user_id = %job.user_id,
            request_id = ?action.metadata.get("request_id"),
            quiet_hours_start = %preferences.quiet_hours_start,
            quiet_hours_end = %preferences.quiet_hours_end,
            "notification suppressed by quiet hours"
        );

        return Ok(());
    }

    record_notification_audit(
        store,
        job.user_id,
        "JOB_ACTION_GENERATED",
        AuditResult::Success,
        action.metadata.clone(),
    )
    .await;

    send_notification_to_devices(store, push_sender, job, content, &action.metadata, metrics).await
}

async fn send_notification_to_devices(
    store: &Store,
    push_sender: &PushSender,
    job: &ClaimedJob,
    content: &NotificationContent,
    metadata_base: &HashMap<String, String>,
    metrics: &mut WorkerTickMetrics,
) -> Result<(), JobExecutionError> {
    let request_id = metadata_base.get("request_id").map(String::as_str);
    let devices = store
        .list_registered_devices(job.user_id)
        .await
        .map_err(|err| {
            JobExecutionError::transient(
                "DEVICE_LOOKUP_FAILED",
                format!("failed to fetch registered devices: {err}"),
            )
        })?;

    if devices.is_empty() {
        return Err(JobExecutionError::permanent(
            "NO_REGISTERED_DEVICE",
            "no APNs device registered for user",
        ));
    }

    let mut delivered = 0_usize;
    let mut first_transient_error: Option<JobExecutionError> = None;
    let mut first_permanent_error: Option<JobExecutionError> = None;

    for device in &devices {
        metrics.push_attempts += 1;

        match push_sender.send(device, content).await {
            Ok(()) => {
                delivered += 1;
                metrics.push_delivered += 1;

                let mut metadata = metadata_base.clone();
                metadata.insert("device_id".to_string(), device.device_id.clone());
                metadata.insert(
                    "environment".to_string(),
                    apns_environment_label(&device.environment).to_string(),
                );
                metadata.insert("outcome".to_string(), "delivered".to_string());

                record_notification_audit(
                    store,
                    job.user_id,
                    "NOTIFICATION_DELIVERY_ATTEMPT",
                    AuditResult::Success,
                    metadata,
                )
                .await;
            }
            Err(err) => {
                let (error_code, error_message, class) = match &err {
                    PushSendError::Transient { code, message } => {
                        metrics.push_transient_failures += 1;
                        (code.clone(), message.clone(), FailureClass::Transient)
                    }
                    PushSendError::Permanent { code, message } => {
                        metrics.push_permanent_failures += 1;
                        (code.clone(), message.clone(), FailureClass::Permanent)
                    }
                };

                let mut metadata = metadata_base.clone();
                metadata.insert("device_id".to_string(), device.device_id.clone());
                metadata.insert(
                    "environment".to_string(),
                    apns_environment_label(&device.environment).to_string(),
                );
                metadata.insert("outcome".to_string(), "failed".to_string());
                metadata.insert("error_code".to_string(), error_code.clone());

                record_notification_audit(
                    store,
                    job.user_id,
                    "NOTIFICATION_DELIVERY_ATTEMPT",
                    AuditResult::Failure,
                    metadata,
                )
                .await;

                match class {
                    FailureClass::Transient if first_transient_error.is_none() => {
                        first_transient_error = Some(err.to_job_error())
                    }
                    FailureClass::Permanent if first_permanent_error.is_none() => {
                        first_permanent_error = Some(err.to_job_error())
                    }
                    _ => {}
                }

                warn!(
                    job_id = %job.id,
                    user_id = %job.user_id,
                    request_id = ?request_id,
                    device_id = %device.device_id,
                    error_code = %error_code,
                    error_message = %error_message,
                    "push delivery attempt failed"
                );
            }
        }
    }

    if delivered > 0 {
        return Ok(());
    }

    if let Some(err) = first_transient_error {
        return Err(err);
    }

    if let Some(err) = first_permanent_error {
        return Err(err);
    }

    Err(JobExecutionError::permanent(
        "PUSH_DELIVERY_FAILED",
        "push delivery failed without a classified error",
    ))
}

async fn record_notification_audit(
    store: &Store,
    user_id: uuid::Uuid,
    event_type: &str,
    result: AuditResult,
    metadata: HashMap<String, String>,
) {
    let request_id = metadata.get("request_id").map(String::as_str);
    if let Err(err) = store
        .add_audit_event(user_id, event_type, None, result, &metadata)
        .await
    {
        warn!(
            user_id = %user_id,
            event_type = %event_type,
            request_id = ?request_id,
            "failed to persist notification audit event: {err}"
        );
    }
}
