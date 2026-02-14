use std::collections::HashMap;

use chrono::{DateTime, Duration as ChronoDuration, NaiveTime, Utc};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use shared::config::WorkerConfig;
use shared::models::ApnsEnvironment;
use shared::repos::{AuditResult, ClaimedJob, DeviceRegistration, JobType, Store};
use tokio::signal;
use tokio::time::{self, Duration};
use tracing::{error, info, warn};
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
enum FailureClass {
    Transient,
    Permanent,
}

#[derive(Debug)]
struct JobExecutionError {
    class: FailureClass,
    code: String,
    message: String,
}

impl JobExecutionError {
    fn transient(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            class: FailureClass::Transient,
            code: code.into(),
            message: message.into(),
        }
    }

    fn permanent(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            class: FailureClass::Permanent,
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Default)]
struct WorkerTickMetrics {
    claimed_jobs: usize,
    processed_jobs: usize,
    successful_jobs: usize,
    retryable_failures: usize,
    permanent_failures: usize,
    dead_lettered_jobs: usize,
    push_attempts: usize,
    push_delivered: usize,
    push_quiet_hours_suppressed: usize,
    push_transient_failures: usize,
    push_permanent_failures: usize,
    total_lag_seconds: i64,
    max_lag_seconds: i64,
}

impl WorkerTickMetrics {
    fn record_lag(&mut self, due_at: DateTime<Utc>, now: DateTime<Utc>) {
        let lag_seconds = (now - due_at).num_seconds().max(0);
        self.total_lag_seconds += lag_seconds;
        self.max_lag_seconds = self.max_lag_seconds.max(lag_seconds);
    }

    fn average_lag_seconds(&self) -> f64 {
        if self.processed_jobs == 0 {
            return 0.0;
        }

        self.total_lag_seconds as f64 / self.processed_jobs as f64
    }

    fn success_rate(&self) -> f64 {
        if self.processed_jobs == 0 {
            return 1.0;
        }

        self.successful_jobs as f64 / self.processed_jobs as f64
    }
}

#[derive(Clone)]
struct PushSender {
    client: reqwest::Client,
    sandbox_endpoint: Option<String>,
    production_endpoint: Option<String>,
    auth_token: Option<String>,
}

#[derive(Debug)]
enum PushSendError {
    Transient { code: String, message: String },
    Permanent { code: String, message: String },
}

impl PushSendError {
    fn to_job_error(&self) -> JobExecutionError {
        match self {
            Self::Transient { code, message } => {
                JobExecutionError::transient(code.clone(), message.clone())
            }
            Self::Permanent { code, message } => {
                JobExecutionError::permanent(code.clone(), message.clone())
            }
        }
    }
}

#[derive(Debug, Clone)]
struct NotificationContent {
    title: String,
    body: String,
}

#[derive(Debug, Deserialize)]
struct NotificationJobPayload {
    notification: Option<NotificationPayloadBody>,
}

#[derive(Debug, Deserialize)]
struct NotificationPayloadBody {
    title: String,
    body: String,
}

#[derive(Debug, Serialize)]
struct PushDeliveryRequest<'a> {
    device_token: &'a str,
    title: &'a str,
    body: &'a str,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "worker=debug".to_string()))
        .init();

    let config = match WorkerConfig::from_env() {
        Ok(cfg) => cfg,
        Err(err) => {
            error!("failed to read worker config: {err}");
            std::process::exit(1);
        }
    };

    let store = match Store::connect(
        &config.database_url,
        config.database_max_connections,
        &config.data_encryption_key,
    )
    .await
    {
        Ok(store) => store,
        Err(err) => {
            error!("failed to connect to postgres: {err}");
            std::process::exit(1);
        }
    };

    let push_sender = PushSender::new(
        config.apns_sandbox_endpoint.clone(),
        config.apns_production_endpoint.clone(),
        config.apns_auth_token.clone(),
    );

    let worker_id = Uuid::new_v4();
    info!(
        worker_id = %worker_id,
        tick_seconds = config.tick_seconds,
        batch_size = config.batch_size,
        lease_seconds = config.lease_seconds,
        per_user_concurrency_limit = config.per_user_concurrency_limit,
        apns_sandbox_endpoint_configured = config.apns_sandbox_endpoint.is_some(),
        apns_production_endpoint_configured = config.apns_production_endpoint.is_some(),
        "worker starting"
    );

    let mut ticker = time::interval(Duration::from_secs(config.tick_seconds));

    loop {
        tokio::select! {
            _ = signal::ctrl_c() => {
                info!(worker_id = %worker_id, "shutdown signal received");
                break;
            }
            _ = ticker.tick() => {
                process_due_jobs(&store, &config, &push_sender, worker_id).await;
            }
        }
    }
}

impl PushSender {
    fn new(
        sandbox_endpoint: Option<String>,
        production_endpoint: Option<String>,
        auth_token: Option<String>,
    ) -> Self {
        Self {
            client: reqwest::Client::new(),
            sandbox_endpoint,
            production_endpoint,
            auth_token,
        }
    }

    async fn send(
        &self,
        device: &DeviceRegistration,
        content: &NotificationContent,
    ) -> Result<(), PushSendError> {
        let endpoint = match device.environment {
            ApnsEnvironment::Sandbox => self.sandbox_endpoint.as_deref(),
            ApnsEnvironment::Production => self.production_endpoint.as_deref(),
        };

        let Some(endpoint) = endpoint else {
            info!(
                device_id = %device.device_id,
                environment = %apns_environment_label(&device.environment),
                "apns endpoint not configured for environment; simulated delivery"
            );
            return Ok(());
        };

        let request = PushDeliveryRequest {
            device_token: &device.apns_token,
            title: &content.title,
            body: &content.body,
        };

        let mut builder = self.client.post(endpoint).json(&request);
        if let Some(auth_token) = self.auth_token.as_deref() {
            builder = builder.bearer_auth(auth_token);
        }

        let response = builder
            .send()
            .await
            .map_err(|err| PushSendError::Transient {
                code: "APNS_NETWORK_ERROR".to_string(),
                message: format!("APNs request failed: {err}"),
            })?;

        let status = response.status();
        if status.is_success() {
            return Ok(());
        }

        let body = response.text().await.unwrap_or_default();
        let code = format!("APNS_HTTP_{}", status.as_u16());
        let message = if body.is_empty() {
            format!("APNs responded with status {status}")
        } else {
            format!("APNs responded with status {status}: {body}")
        };

        match classify_http_failure(status) {
            FailureClass::Transient => Err(PushSendError::Transient { code, message }),
            FailureClass::Permanent => Err(PushSendError::Permanent { code, message }),
        }
    }
}

async fn process_due_jobs(
    store: &Store,
    config: &WorkerConfig,
    push_sender: &PushSender,
    worker_id: Uuid,
) {
    let now = Utc::now();
    let claimed_jobs = match store
        .claim_due_jobs(
            now,
            worker_id,
            i64::from(config.batch_size),
            i64::try_from(config.lease_seconds).unwrap_or(i64::MAX),
            i32::try_from(config.per_user_concurrency_limit).unwrap_or(i32::MAX),
        )
        .await
    {
        Ok(jobs) => jobs,
        Err(err) => {
            error!(worker_id = %worker_id, "failed to claim due jobs: {err}");
            return;
        }
    };

    let mut metrics = WorkerTickMetrics {
        claimed_jobs: claimed_jobs.len(),
        ..WorkerTickMetrics::default()
    };

    for job in claimed_jobs {
        metrics.record_lag(job.due_at, now);
        process_claimed_job(store, config, push_sender, worker_id, job, &mut metrics).await;
    }

    let due_count = store.count_due_jobs(Utc::now()).await.unwrap_or(-1);

    info!(
        worker_id = %worker_id,
        pending_due_jobs = due_count,
        claimed_jobs = metrics.claimed_jobs,
        processed_jobs = metrics.processed_jobs,
        successful_jobs = metrics.successful_jobs,
        retryable_failures = metrics.retryable_failures,
        permanent_failures = metrics.permanent_failures,
        dead_lettered_jobs = metrics.dead_lettered_jobs,
        push_attempts = metrics.push_attempts,
        push_delivered = metrics.push_delivered,
        push_quiet_hours_suppressed = metrics.push_quiet_hours_suppressed,
        push_transient_failures = metrics.push_transient_failures,
        push_permanent_failures = metrics.push_permanent_failures,
        average_lag_seconds = metrics.average_lag_seconds(),
        max_lag_seconds = metrics.max_lag_seconds,
        success_rate = metrics.success_rate(),
        "worker tick metrics"
    );
}

async fn process_claimed_job(
    store: &Store,
    config: &WorkerConfig,
    push_sender: &PushSender,
    worker_id: Uuid,
    job: ClaimedJob,
    metrics: &mut WorkerTickMetrics,
) {
    metrics.processed_jobs += 1;

    match execute_job(store, push_sender, &job, metrics).await {
        Ok(()) => match store.mark_job_done(job.id, worker_id).await {
            Ok(true) => {
                metrics.successful_jobs += 1;
            }
            Ok(false) => {
                warn!(
                    worker_id = %worker_id,
                    job_id = %job.id,
                    "job completion skipped because lease ownership was lost"
                );
                metrics.permanent_failures += 1;
            }
            Err(err) => {
                error!(
                    worker_id = %worker_id,
                    job_id = %job.id,
                    "failed to persist job completion: {err}"
                );
                metrics.retryable_failures += 1;
            }
        },
        Err(err) => {
            let next_attempt = job.attempts.saturating_add(1);
            let can_retry =
                matches!(err.class, FailureClass::Transient) && next_attempt < job.max_attempts;

            if can_retry {
                let delay_seconds = retry_delay_seconds(
                    config.retry_base_delay_seconds,
                    config.retry_max_delay_seconds,
                    next_attempt,
                );
                let next_due_at = Utc::now()
                    + ChronoDuration::seconds(i64::try_from(delay_seconds).unwrap_or(i64::MAX));

                match store
                    .schedule_job_retry(
                        job.id,
                        worker_id,
                        next_attempt,
                        next_due_at,
                        &err.code,
                        &err.message,
                    )
                    .await
                {
                    Ok(true) => {
                        metrics.retryable_failures += 1;
                        info!(
                            worker_id = %worker_id,
                            job_id = %job.id,
                            next_attempt,
                            next_due_at = %next_due_at,
                            error_code = %err.code,
                            "job scheduled for retry"
                        );
                    }
                    Ok(false) => {
                        warn!(
                            worker_id = %worker_id,
                            job_id = %job.id,
                            "job retry update skipped because lease ownership was lost"
                        );
                        metrics.permanent_failures += 1;
                    }
                    Err(store_err) => {
                        error!(
                            worker_id = %worker_id,
                            job_id = %job.id,
                            "failed to schedule retry: {store_err}"
                        );
                        metrics.retryable_failures += 1;
                    }
                }
            } else {
                match store
                    .mark_job_failed(&job, worker_id, next_attempt, &err.code, &err.message)
                    .await
                {
                    Ok(true) => {
                        metrics.permanent_failures += 1;
                        metrics.dead_lettered_jobs += 1;
                        warn!(
                            worker_id = %worker_id,
                            job_id = %job.id,
                            attempts = next_attempt,
                            error_code = %err.code,
                            "job dead-lettered"
                        );
                    }
                    Ok(false) => {
                        warn!(
                            worker_id = %worker_id,
                            job_id = %job.id,
                            "job failure update skipped because lease ownership was lost"
                        );
                        metrics.permanent_failures += 1;
                    }
                    Err(store_err) => {
                        error!(
                            worker_id = %worker_id,
                            job_id = %job.id,
                            "failed to dead-letter job: {store_err}"
                        );
                        metrics.retryable_failures += 1;
                    }
                }
            }
        }
    }
}

async fn execute_job(
    store: &Store,
    push_sender: &PushSender,
    job: &ClaimedJob,
    metrics: &mut WorkerTickMetrics,
) -> Result<(), JobExecutionError> {
    let has_action_lease = store
        .record_outbound_action_idempotency(job.user_id, &job.idempotency_key, job.id)
        .await
        .map_err(|err| {
            JobExecutionError::transient(
                "IDEMPOTENCY_WRITE_FAILED",
                format!("failed to write idempotency record: {err}"),
            )
        })?;

    if !has_action_lease {
        info!(
            job_id = %job.id,
            user_id = %job.user_id,
            idempotency_key = %job.idempotency_key,
            "duplicate action prevented by idempotency key"
        );
        return Ok(());
    }

    if let Err(err) = dispatch_job_action(store, push_sender, job, metrics).await {
        if let Err(release_err) = store
            .release_outbound_action_idempotency(job.user_id, &job.idempotency_key, job.id)
            .await
        {
            return Err(JobExecutionError::permanent(
                "IDEMPOTENCY_RELEASE_FAILED",
                format!("failed to release idempotency reservation: {release_err}"),
            ));
        }

        return Err(err);
    }

    Ok(())
}

async fn dispatch_job_action(
    store: &Store,
    push_sender: &PushSender,
    job: &ClaimedJob,
    metrics: &mut WorkerTickMetrics,
) -> Result<(), JobExecutionError> {
    if let Some(simulated_failure) = parse_simulated_failure(job.payload_ciphertext.as_deref()) {
        return Err(simulated_failure);
    }

    let content = resolve_notification_content(job);

    let preferences = store
        .get_or_create_preferences(job.user_id)
        .await
        .map_err(|err| {
            JobExecutionError::transient(
                "PREFERENCES_READ_FAILED",
                format!("failed to read user preferences: {err}"),
            )
        })?;

    if is_within_quiet_hours(
        Utc::now().time(),
        &preferences.quiet_hours_start,
        &preferences.quiet_hours_end,
    )
    .map_err(|message| JobExecutionError::permanent("INVALID_QUIET_HOURS", message))?
    {
        metrics.push_quiet_hours_suppressed += 1;

        let mut metadata = HashMap::new();
        metadata.insert("job_id".to_string(), job.id.to_string());
        metadata.insert("job_type".to_string(), job.job_type.as_str().to_string());
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
            quiet_hours_start = %preferences.quiet_hours_start,
            quiet_hours_end = %preferences.quiet_hours_end,
            "notification suppressed by quiet hours"
        );

        return Ok(());
    }

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

        match push_sender.send(device, &content).await {
            Ok(()) => {
                delivered += 1;
                metrics.push_delivered += 1;

                let mut metadata = HashMap::new();
                metadata.insert("job_id".to_string(), job.id.to_string());
                metadata.insert("job_type".to_string(), job.job_type.as_str().to_string());
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

                let mut metadata = HashMap::new();
                metadata.insert("job_id".to_string(), job.id.to_string());
                metadata.insert("job_type".to_string(), job.job_type.as_str().to_string());
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
    user_id: Uuid,
    event_type: &str,
    result: AuditResult,
    metadata: HashMap<String, String>,
) {
    if let Err(err) = store
        .add_audit_event(user_id, event_type, None, result, &metadata)
        .await
    {
        warn!(
            user_id = %user_id,
            event_type = %event_type,
            "failed to persist notification audit event: {err}"
        );
    }
}

fn resolve_notification_content(job: &ClaimedJob) -> NotificationContent {
    if let Some(payload) = parse_notification_payload(job.payload_ciphertext.as_deref()) {
        let title = payload.title.trim();
        let body = payload.body.trim();

        if !title.is_empty() && !body.is_empty() {
            return NotificationContent {
                title: title.to_string(),
                body: body.to_string(),
            };
        }
    }

    match job.job_type {
        JobType::MeetingReminder => NotificationContent {
            title: "Meeting reminder".to_string(),
            body: "You have a meeting coming up soon.".to_string(),
        },
        JobType::MorningBrief => NotificationContent {
            title: "Morning brief".to_string(),
            body: "Your Alfred morning brief is ready.".to_string(),
        },
        JobType::UrgentEmailCheck => NotificationContent {
            title: "Urgent email alert".to_string(),
            body: "Alfred detected an urgent email that needs attention.".to_string(),
        },
    }
}

fn parse_notification_payload(payload: Option<&[u8]>) -> Option<NotificationPayloadBody> {
    let payload = payload?;
    let parsed: NotificationJobPayload = serde_json::from_slice(payload).ok()?;
    parsed.notification
}

fn parse_simulated_failure(payload: Option<&[u8]>) -> Option<JobExecutionError> {
    let payload = payload?;
    let text = std::str::from_utf8(payload).ok()?;

    let mut parts = text.splitn(4, ':');
    if parts.next()? != "simulate-failure" {
        return None;
    }

    let class = parts.next()?;
    let code = parts.next()?.trim();
    let message = parts.next()?.trim();

    match class {
        "transient" => Some(JobExecutionError::transient(code, message)),
        "permanent" => Some(JobExecutionError::permanent(code, message)),
        _ => None,
    }
}

fn apns_environment_label(environment: &ApnsEnvironment) -> &'static str {
    match environment {
        ApnsEnvironment::Sandbox => "sandbox",
        ApnsEnvironment::Production => "production",
    }
}

fn is_within_quiet_hours(now: NaiveTime, start: &str, end: &str) -> Result<bool, String> {
    let start = parse_hhmm(start)?;
    let end = parse_hhmm(end)?;

    if start == end {
        return Ok(true);
    }

    if start < end {
        Ok(now >= start && now < end)
    } else {
        Ok(now >= start || now < end)
    }
}

fn parse_hhmm(value: &str) -> Result<NaiveTime, String> {
    NaiveTime::parse_from_str(value, "%H:%M")
        .map_err(|_| format!("time must be in HH:MM format: {value}"))
}

fn classify_http_failure(status: StatusCode) -> FailureClass {
    match status.as_u16() {
        408 | 425 | 429 | 500 | 502 | 503 | 504 => FailureClass::Transient,
        _ => FailureClass::Permanent,
    }
}

fn retry_delay_seconds(base_seconds: u64, max_seconds: u64, attempt: i32) -> u64 {
    if attempt <= 1 {
        return base_seconds.min(max_seconds);
    }

    let exponent = u32::try_from(attempt.saturating_sub(1)).unwrap_or(u32::MAX);
    let capped_exponent = exponent.min(20);
    let multiplier = 1_u64 << capped_exponent;

    base_seconds.saturating_mul(multiplier).min(max_seconds)
}

#[cfg(test)]
mod tests {
    use chrono::NaiveTime;
    use reqwest::StatusCode;

    use super::{FailureClass, classify_http_failure, is_within_quiet_hours, retry_delay_seconds};

    #[test]
    fn retry_backoff_is_exponential_and_capped() {
        assert_eq!(retry_delay_seconds(30, 900, 1), 30);
        assert_eq!(retry_delay_seconds(30, 900, 2), 60);
        assert_eq!(retry_delay_seconds(30, 900, 3), 120);
        assert_eq!(retry_delay_seconds(30, 900, 10), 900);
    }

    #[test]
    fn quiet_hours_supports_wrapped_ranges() {
        let before_midnight = NaiveTime::from_hms_opt(23, 15, 0).expect("valid time");
        let after_midnight = NaiveTime::from_hms_opt(6, 45, 0).expect("valid time");
        let outside = NaiveTime::from_hms_opt(14, 0, 0).expect("valid time");

        assert!(is_within_quiet_hours(before_midnight, "22:00", "07:00").expect("valid range"));
        assert!(is_within_quiet_hours(after_midnight, "22:00", "07:00").expect("valid range"));
        assert!(!is_within_quiet_hours(outside, "22:00", "07:00").expect("valid range"));
    }

    #[test]
    fn quiet_hours_supports_non_wrapped_ranges() {
        let in_range = NaiveTime::from_hms_opt(13, 0, 0).expect("valid time");
        let out_of_range = NaiveTime::from_hms_opt(17, 0, 0).expect("valid time");

        assert!(is_within_quiet_hours(in_range, "12:00", "14:00").expect("valid range"));
        assert!(!is_within_quiet_hours(out_of_range, "12:00", "14:00").expect("valid range"));
    }

    #[test]
    fn quiet_hours_with_equal_bounds_suppresses_all_day() {
        let now = NaiveTime::from_hms_opt(9, 30, 0).expect("valid time");
        assert!(is_within_quiet_hours(now, "08:00", "08:00").expect("valid range"));
    }

    #[test]
    fn classifies_retryable_http_status_codes_as_transient() {
        assert!(matches!(
            classify_http_failure(StatusCode::TOO_MANY_REQUESTS),
            FailureClass::Transient
        ));
        assert!(matches!(
            classify_http_failure(StatusCode::SERVICE_UNAVAILABLE),
            FailureClass::Transient
        ));
    }

    #[test]
    fn classifies_client_errors_as_permanent() {
        assert!(matches!(
            classify_http_failure(StatusCode::BAD_REQUEST),
            FailureClass::Permanent
        ));
        assert!(matches!(
            classify_http_failure(StatusCode::GONE),
            FailureClass::Permanent
        ));
    }
}
