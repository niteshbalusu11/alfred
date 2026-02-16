use chrono::{Duration as ChronoDuration, Utc};
use shared::config::WorkerConfig;
use shared::repos::{ClaimedJob, Store};
use shared::security::SecretRuntime;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::{FailureClass, JobExecutionError, PushSender, WorkerTickMetrics, retry_delay_seconds};

struct JobRuntime<'a> {
    store: &'a Store,
    config: &'a WorkerConfig,
    secret_runtime: &'a SecretRuntime,
    oauth_client: &'a reqwest::Client,
    push_sender: &'a PushSender,
}

pub(crate) async fn process_due_jobs(
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

    if let Err(err) = crate::job_actions::dispatch_job_action(
        crate::job_actions::JobActionContext {
            store: runtime.store,
            config: runtime.config,
            secret_runtime: runtime.secret_runtime,
            oauth_client: runtime.oauth_client,
            push_sender: runtime.push_sender,
        },
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
