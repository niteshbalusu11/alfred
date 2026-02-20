use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use shared::config::WorkerConfig;
use shared::repos::{JobType, Store};
use tracing::{error, info, warn};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AutomationRunJobPayload {
    pub(crate) automation_run_id: Uuid,
    pub(crate) automation_rule_id: Uuid,
    pub(crate) scheduled_for: DateTime<Utc>,
    pub(crate) prompt_sha256: String,
    pub(crate) prompt_envelope_ciphertext_b64: String,
}

impl AutomationRunJobPayload {
    pub(crate) fn parse(payload: Option<&[u8]>) -> Result<Self, &'static str> {
        let payload = payload.ok_or("automation payload is required")?;
        serde_json::from_slice(payload).map_err(|_| "automation payload must be valid JSON")
    }
}

#[derive(Debug, Default)]
pub(crate) struct AutomationSchedulerMetrics {
    pub(crate) claimed_rules: usize,
    pub(crate) materialized_runs: usize,
    pub(crate) enqueued_runs: usize,
    pub(crate) failed_runs: usize,
}

pub(crate) async fn enqueue_due_automation_runs(
    store: &Store,
    config: &WorkerConfig,
    worker_id: Uuid,
) -> AutomationSchedulerMetrics {
    let mut metrics = AutomationSchedulerMetrics::default();
    let now = Utc::now();
    let claimed_rules = match store
        .claim_due_automation_rules(
            now,
            worker_id,
            i64::from(config.batch_size),
            i64::try_from(config.lease_seconds).unwrap_or(i64::MAX),
        )
        .await
    {
        Ok(rules) => rules,
        Err(err) => {
            error!(worker_id = %worker_id, "failed to claim due automation rules: {err}");
            return metrics;
        }
    };
    metrics.claimed_rules = claimed_rules.len();

    for rule in claimed_rules {
        let scheduled_for = rule.next_run_at;
        let interval_seconds = i64::from(rule.interval_seconds.max(60));
        let next_run_at = scheduled_for + ChronoDuration::seconds(interval_seconds);
        let idempotency_key = format!("{}:{}", rule.id, scheduled_for.timestamp_micros());

        let run = match store
            .materialize_automation_run(
                rule.id,
                worker_id,
                scheduled_for,
                next_run_at,
                &idempotency_key,
            )
            .await
        {
            Ok(Some(run)) => run,
            Ok(None) => {
                warn!(
                    worker_id = %worker_id,
                    rule_id = %rule.id,
                    "automation run materialization skipped because lease ownership was lost"
                );
                continue;
            }
            Err(err) => {
                metrics.failed_runs += 1;
                error!(
                    worker_id = %worker_id,
                    rule_id = %rule.id,
                    "failed to materialize automation run: {err}"
                );
                continue;
            }
        };
        metrics.materialized_runs += 1;

        let payload = AutomationRunJobPayload {
            automation_run_id: run.id,
            automation_rule_id: rule.id,
            scheduled_for,
            prompt_sha256: rule.prompt_sha256,
            prompt_envelope_ciphertext_b64: STANDARD.encode(rule.prompt_ciphertext),
        };
        let payload_json = match serde_json::to_vec(&payload) {
            Ok(payload_json) => payload_json,
            Err(err) => {
                metrics.failed_runs += 1;
                error!(
                    worker_id = %worker_id,
                    run_id = %run.id,
                    "failed to serialize automation run payload: {err}"
                );
                let _ = store.mark_automation_run_failed(run.id, run.user_id).await;
                continue;
            }
        };

        let job_id = match store
            .enqueue_job_with_idempotency_key(
                run.user_id,
                JobType::AutomationRun,
                now,
                Some(&payload_json),
                &idempotency_key,
            )
            .await
        {
            Ok(job_id) => job_id,
            Err(err) => {
                metrics.failed_runs += 1;
                error!(
                    worker_id = %worker_id,
                    run_id = %run.id,
                    "failed to enqueue automation run job: {err}"
                );
                let _ = store.mark_automation_run_failed(run.id, run.user_id).await;
                continue;
            }
        };

        match store
            .mark_automation_run_enqueued(run.id, run.user_id, job_id)
            .await
        {
            Ok(true) => {
                metrics.enqueued_runs += 1;
            }
            Ok(false) => {
                metrics.failed_runs += 1;
                warn!(
                    worker_id = %worker_id,
                    run_id = %run.id,
                    "failed to mark automation run enqueued due to lease/user mismatch"
                );
                let _ = store.mark_automation_run_failed(run.id, run.user_id).await;
            }
            Err(err) => {
                metrics.failed_runs += 1;
                error!(
                    worker_id = %worker_id,
                    run_id = %run.id,
                    "failed to update automation run state: {err}"
                );
                let _ = store.mark_automation_run_failed(run.id, run.user_id).await;
            }
        }
    }

    info!(
        worker_id = %worker_id,
        claimed_automation_rules = metrics.claimed_rules,
        materialized_automation_runs = metrics.materialized_runs,
        enqueued_automation_runs = metrics.enqueued_runs,
        failed_automation_runs = metrics.failed_runs,
        "automation scheduler metrics"
    );

    metrics
}
