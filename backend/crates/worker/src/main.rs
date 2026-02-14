use chrono::{DateTime, Duration as ChronoDuration, Utc};
use reqwest::StatusCode;
use serde::Serialize;
use shared::config::WorkerConfig;
use shared::models::ApnsEnvironment;
use shared::repos::{ClaimedJob, DeviceRegistration, Store};
use shared::security::{KmsDecryptPolicy, SecretRuntime, TeeAttestationPolicy};
use tokio::signal;
use tokio::time::{self, Duration};
use tracing::{error, info, warn};
use uuid::Uuid;

mod job_actions;
mod privacy_delete;
mod privacy_delete_revoke;

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

struct JobRuntime<'a> {
    store: &'a Store,
    config: &'a WorkerConfig,
    secret_runtime: &'a SecretRuntime,
    oauth_client: &'a reqwest::Client,
    push_sender: &'a PushSender,
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
    let oauth_client = reqwest::Client::new();
    let secret_runtime = SecretRuntime::new(
        TeeAttestationPolicy {
            required: config.tee_attestation_required,
            expected_runtime: config.tee_expected_runtime.clone(),
            allowed_measurements: config.tee_allowed_measurements.clone(),
            attestation_public_key: config.tee_attestation_public_key.clone(),
            max_attestation_age_seconds: config.tee_attestation_max_age_seconds,
            allow_insecure_dev_attestation: config.tee_allow_insecure_dev_attestation,
        },
        KmsDecryptPolicy {
            key_id: config.kms_key_id.clone(),
            key_version: config.kms_key_version,
            allowed_measurements: config.kms_allowed_measurements.clone(),
        },
        config.tee_attestation_document.clone(),
        config.tee_attestation_document_path.clone(),
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
                privacy_delete::process_delete_requests(
                    &store,
                    &config,
                    &secret_runtime,
                    &oauth_client,
                    worker_id,
                ).await;
                process_due_jobs(
                    &store,
                    &config,
                    &secret_runtime,
                    &oauth_client,
                    &push_sender,
                    worker_id,
                )
                .await;
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
    secret_runtime: &SecretRuntime,
    oauth_client: &reqwest::Client,
    push_sender: &PushSender,
    worker_id: Uuid,
) {
    let runtime = JobRuntime {
        store,
        config,
        secret_runtime,
        oauth_client,
        push_sender,
    };

    let now = Utc::now();
    let claimed_jobs = match runtime
        .store
        .claim_due_jobs(
            now,
            worker_id,
            i64::from(runtime.config.batch_size),
            i64::try_from(runtime.config.lease_seconds).unwrap_or(i64::MAX),
            i32::try_from(runtime.config.per_user_concurrency_limit).unwrap_or(i32::MAX),
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
        process_claimed_job(&runtime, worker_id, job, &mut metrics).await;
    }

    let due_count = runtime.store.count_due_jobs(Utc::now()).await.unwrap_or(-1);

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
    runtime: &JobRuntime<'_>,
    worker_id: Uuid,
    job: ClaimedJob,
    metrics: &mut WorkerTickMetrics,
) {
    metrics.processed_jobs += 1;

    match execute_job(runtime, &job, metrics).await {
        Ok(()) => match runtime.store.mark_job_done(job.id, worker_id).await {
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
                    runtime.config.retry_base_delay_seconds,
                    runtime.config.retry_max_delay_seconds,
                    next_attempt,
                );
                let next_due_at = Utc::now()
                    + ChronoDuration::seconds(i64::try_from(delay_seconds).unwrap_or(i64::MAX));

                match runtime
                    .store
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
                match runtime
                    .store
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
    runtime: &JobRuntime<'_>,
    job: &ClaimedJob,
    metrics: &mut WorkerTickMetrics,
) -> Result<(), JobExecutionError> {
    let has_action_lease = runtime
        .store
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

    if let Err(err) = job_actions::dispatch_job_action(
        runtime.store,
        runtime.config,
        runtime.secret_runtime,
        runtime.oauth_client,
        runtime.push_sender,
        job,
        metrics,
    )
    .await
    {
        if let Err(release_err) = runtime
            .store
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

fn apns_environment_label(environment: &ApnsEnvironment) -> &'static str {
    match environment {
        ApnsEnvironment::Sandbox => "sandbox",
        ApnsEnvironment::Production => "production",
    }
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
    use reqwest::StatusCode;

    use super::{FailureClass, classify_http_failure, retry_delay_seconds};

    #[test]
    fn retry_backoff_is_exponential_and_capped() {
        assert_eq!(retry_delay_seconds(30, 900, 1), 30);
        assert_eq!(retry_delay_seconds(30, 900, 2), 60);
        assert_eq!(retry_delay_seconds(30, 900, 3), 120);
        assert_eq!(retry_delay_seconds(30, 900, 10), 900);
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
