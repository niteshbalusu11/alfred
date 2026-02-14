use std::collections::HashMap;

use chrono::Utc;
use shared::config::WorkerConfig;
use shared::repos::{AuditResult, ClaimedDeleteRequest, Store};
use shared::security::SecretRuntime;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::privacy_delete_revoke::{DeleteRequestError, revoke_active_connectors};

#[derive(Default)]
pub(crate) struct PrivacyDeleteTickMetrics {
    pub claimed_requests: usize,
    pub completed_requests: usize,
    pub failed_requests: usize,
    pub revoked_connectors: usize,
    pub pending_requests: i64,
    pub overdue_requests: i64,
}

pub(crate) async fn process_delete_requests(
    store: &Store,
    config: &WorkerConfig,
    secret_runtime: &SecretRuntime,
    oauth_client: &reqwest::Client,
    worker_id: Uuid,
) -> PrivacyDeleteTickMetrics {
    let now = Utc::now();
    let claimed_requests = match store
        .claim_delete_requests(
            now,
            worker_id,
            i64::from(config.privacy_delete_batch_size),
            i64::try_from(config.privacy_delete_lease_seconds).unwrap_or(i64::MAX),
        )
        .await
    {
        Ok(claimed_requests) => claimed_requests,
        Err(err) => {
            error!(
                worker_id = %worker_id,
                "failed to claim privacy delete requests: {err}"
            );
            return PrivacyDeleteTickMetrics::default();
        }
    };

    let mut metrics = PrivacyDeleteTickMetrics {
        claimed_requests: claimed_requests.len(),
        ..PrivacyDeleteTickMetrics::default()
    };

    for request in claimed_requests {
        process_claimed_delete_request(
            store,
            config,
            secret_runtime,
            oauth_client,
            worker_id,
            request,
            &mut metrics,
        )
        .await;
    }

    metrics.pending_requests = store.count_pending_delete_requests().await.unwrap_or(-1);
    metrics.overdue_requests = store
        .count_delete_requests_sla_overdue(
            Utc::now(),
            i64::try_from(config.privacy_delete_sla_hours).unwrap_or(i64::MAX),
        )
        .await
        .unwrap_or(-1);

    if metrics.overdue_requests > 0 {
        warn!(
            worker_id = %worker_id,
            overdue_requests = metrics.overdue_requests,
            sla_hours = config.privacy_delete_sla_hours,
            "privacy delete SLA alert threshold reached"
        );
    }

    info!(
        worker_id = %worker_id,
        claimed_requests = metrics.claimed_requests,
        completed_requests = metrics.completed_requests,
        failed_requests = metrics.failed_requests,
        revoked_connectors = metrics.revoked_connectors,
        pending_requests = metrics.pending_requests,
        overdue_requests = metrics.overdue_requests,
        "privacy delete tick metrics"
    );

    metrics
}

async fn process_claimed_delete_request(
    store: &Store,
    config: &WorkerConfig,
    secret_runtime: &SecretRuntime,
    oauth_client: &reqwest::Client,
    worker_id: Uuid,
    request: ClaimedDeleteRequest,
    metrics: &mut PrivacyDeleteTickMetrics,
) {
    match execute_delete_request(store, config, secret_runtime, oauth_client, &request).await {
        Ok(revoked_connectors) => {
            let completed_at = Utc::now();
            match store
                .mark_delete_request_completed(request.id, worker_id, completed_at)
                .await
            {
                Ok(true) => {
                    metrics.completed_requests += 1;
                    metrics.revoked_connectors += revoked_connectors;
                    record_delete_completion_audit(
                        store,
                        request.user_id,
                        request.id,
                        completed_at,
                        revoked_connectors,
                        config.privacy_delete_sla_hours,
                    )
                    .await;
                }
                Ok(false) => {
                    warn!(
                        worker_id = %worker_id,
                        request_id = %request.id,
                        "delete request completion skipped because lease ownership was lost"
                    );
                    metrics.failed_requests += 1;
                }
                Err(err) => {
                    error!(
                        worker_id = %worker_id,
                        request_id = %request.id,
                        "failed to mark delete request completed: {err}"
                    );
                    metrics.failed_requests += 1;
                }
            }
        }
        Err(err) => {
            let failed_at = Utc::now();
            let failure_reason = format_failure_reason(&err);
            match store
                .mark_delete_request_failed(request.id, worker_id, failed_at, &failure_reason)
                .await
            {
                Ok(true) => {
                    metrics.failed_requests += 1;
                    record_delete_failure_audit(
                        store,
                        request.user_id,
                        request.id,
                        failed_at,
                        &failure_reason,
                    )
                    .await;
                }
                Ok(false) => {
                    warn!(
                        worker_id = %worker_id,
                        request_id = %request.id,
                        "delete request failure update skipped because lease ownership was lost"
                    );
                    metrics.failed_requests += 1;
                }
                Err(store_err) => {
                    error!(
                        worker_id = %worker_id,
                        request_id = %request.id,
                        "failed to mark delete request failed: {store_err}"
                    );
                    metrics.failed_requests += 1;
                }
            }
        }
    }
}

async fn execute_delete_request(
    store: &Store,
    config: &WorkerConfig,
    secret_runtime: &SecretRuntime,
    oauth_client: &reqwest::Client,
    request: &ClaimedDeleteRequest,
) -> Result<usize, DeleteRequestError> {
    let active_connectors = store
        .list_active_connector_metadata(request.user_id)
        .await
        .map_err(|err| {
            DeleteRequestError::new(
                "CONNECTOR_LOOKUP_FAILED",
                format!("failed to load connectors: {err}"),
            )
        })?;

    let revoked_connectors = revoke_active_connectors(
        store,
        config,
        secret_runtime,
        oauth_client,
        request.user_id,
        active_connectors,
    )
    .await?;

    store
        .purge_user_operational_data(request.user_id)
        .await
        .map_err(|err| {
            DeleteRequestError::new(
                "PURGE_FAILED",
                format!("failed to purge user operational data: {err}"),
            )
        })?;

    Ok(revoked_connectors)
}

async fn record_delete_completion_audit(
    store: &Store,
    user_id: Uuid,
    request_id: Uuid,
    completed_at: chrono::DateTime<Utc>,
    revoked_connectors: usize,
    sla_hours: u64,
) {
    let mut metadata = HashMap::new();
    metadata.insert("request_id".to_string(), request_id.to_string());
    metadata.insert("status".to_string(), "COMPLETED".to_string());
    metadata.insert("completed_at".to_string(), completed_at.to_rfc3339());
    metadata.insert(
        "revoked_connectors".to_string(),
        revoked_connectors.to_string(),
    );
    metadata.insert("sla_hours".to_string(), sla_hours.to_string());

    if let Err(err) = store
        .add_audit_event(
            user_id,
            "PRIVACY_DELETE_ALL_COMPLETED",
            None,
            AuditResult::Success,
            &metadata,
        )
        .await
    {
        warn!(
            user_id = %user_id,
            request_id = %request_id,
            "failed to persist delete completion audit event: {err}"
        );
    }
}

async fn record_delete_failure_audit(
    store: &Store,
    user_id: Uuid,
    request_id: Uuid,
    failed_at: chrono::DateTime<Utc>,
    failure_reason: &str,
) {
    let mut metadata = HashMap::new();
    metadata.insert("request_id".to_string(), request_id.to_string());
    metadata.insert("status".to_string(), "FAILED".to_string());
    metadata.insert("failed_at".to_string(), failed_at.to_rfc3339());
    metadata.insert("reason".to_string(), failure_reason.to_string());

    if let Err(err) = store
        .add_audit_event(
            user_id,
            "PRIVACY_DELETE_ALL_FAILED",
            None,
            AuditResult::Failure,
            &metadata,
        )
        .await
    {
        warn!(
            user_id = %user_id,
            request_id = %request_id,
            "failed to persist delete failure audit event: {err}"
        );
    }
}

fn format_failure_reason(err: &DeleteRequestError) -> String {
    let mut reason = format!("{}: {}", err.code, err.message);
    const MAX_REASON_LEN: usize = 350;
    if reason.len() > MAX_REASON_LEN {
        reason.truncate(MAX_REASON_LEN);
    }
    reason
}
