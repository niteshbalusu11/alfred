use chrono::{DateTime, Duration as ChronoDuration, Utc};
use shared::config::WorkerConfig;
use shared::repos::{ClaimedJob, JobType, Store};
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

    let worker_id = Uuid::new_v4();
    info!(
        worker_id = %worker_id,
        tick_seconds = config.tick_seconds,
        batch_size = config.batch_size,
        lease_seconds = config.lease_seconds,
        per_user_concurrency_limit = config.per_user_concurrency_limit,
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
                process_due_jobs(&store, &config, worker_id).await;
            }
        }
    }
}

async fn process_due_jobs(store: &Store, config: &WorkerConfig, worker_id: Uuid) {
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
        process_claimed_job(store, config, worker_id, job, &mut metrics).await;
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
        average_lag_seconds = metrics.average_lag_seconds(),
        max_lag_seconds = metrics.max_lag_seconds,
        success_rate = metrics.success_rate(),
        "worker tick metrics"
    );
}

async fn process_claimed_job(
    store: &Store,
    config: &WorkerConfig,
    worker_id: Uuid,
    job: ClaimedJob,
    metrics: &mut WorkerTickMetrics,
) {
    metrics.processed_jobs += 1;

    match execute_job(store, &job).await {
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

async fn execute_job(store: &Store, job: &ClaimedJob) -> Result<(), JobExecutionError> {
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

    if let Err(err) = dispatch_job_action(job) {
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

fn dispatch_job_action(job: &ClaimedJob) -> Result<(), JobExecutionError> {
    if let Some(simulated_failure) = parse_simulated_failure(job.payload_ciphertext.as_deref()) {
        return Err(simulated_failure);
    }

    match job.job_type {
        JobType::MeetingReminder | JobType::MorningBrief | JobType::UrgentEmailCheck => Ok(()),
    }
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
    use super::retry_delay_seconds;

    #[test]
    fn retry_backoff_is_exponential_and_capped() {
        assert_eq!(retry_delay_seconds(30, 900, 1), 30);
        assert_eq!(retry_delay_seconds(30, 900, 2), 60);
        assert_eq!(retry_delay_seconds(30, 900, 3), 120);
        assert_eq!(retry_delay_seconds(30, 900, 10), 900);
    }
}
