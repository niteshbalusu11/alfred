mod support;

use std::collections::HashMap;

use chrono::{Duration as ChronoDuration, Utc};
use serial_test::serial;
use shared::repos::{AuditResult, JobType, PrivacyDeleteStatus, StoreError};
use sqlx::Row;
use tokio::time::{Duration, sleep};
use uuid::Uuid;

#[tokio::test]
#[serial]
async fn privacy_delete_execution_flow_completes_and_marks_user_deleted() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let now = Utc::now();
    let user_id = Uuid::new_v4();
    let worker_id = Uuid::new_v4();

    let connector_id = store
        .upsert_google_connector(
            user_id,
            "refresh-token",
            &["https://www.googleapis.com/auth/calendar.readonly".to_string()],
            "kms/local/alfred-refresh-token",
            1,
        )
        .await
        .expect("connector should be created");

    let delete_request_id = store
        .queue_delete_all(user_id)
        .await
        .expect("delete request should queue");

    let claims = store
        .claim_delete_requests(now, worker_id, 1, 300)
        .await
        .expect("delete request should be claimed");
    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0].id, delete_request_id);

    let revoked = store
        .revoke_connector(user_id, connector_id)
        .await
        .expect("connector revoke should succeed");
    assert!(revoked);

    store
        .purge_user_operational_data(user_id)
        .await
        .expect("operational data purge should succeed");

    let marked_completed = store
        .mark_delete_request_completed(delete_request_id, worker_id, now)
        .await
        .expect("mark delete request completed should succeed");
    assert!(marked_completed);

    let status = store
        .get_delete_request_status(user_id, delete_request_id)
        .await
        .expect("delete status lookup should succeed")
        .expect("delete status should exist");
    assert!(matches!(status.status, PrivacyDeleteStatus::Completed));

    let user_row = sqlx::query("SELECT status FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(store.pool())
        .await
        .expect("user row should exist");
    let user_status: String = user_row.try_get("status").expect("status should decode");
    assert_eq!(user_status, "DELETED");
}

#[tokio::test]
#[serial]
async fn outbound_action_idempotency_prevents_duplicate_effects() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let user_id = Uuid::new_v4();
    let job_id = store
        .enqueue_job(user_id, JobType::AutomationRun, Utc::now(), None)
        .await
        .expect("job enqueue should succeed");

    let first = store
        .record_outbound_action_idempotency(user_id, "notify:meeting-123", job_id)
        .await
        .expect("first idempotency record should succeed");
    assert!(first);

    let second = store
        .record_outbound_action_idempotency(user_id, "notify:meeting-123", job_id)
        .await
        .expect("duplicate idempotency record should succeed");
    assert!(!second);

    store
        .release_outbound_action_idempotency(user_id, "notify:meeting-123", job_id)
        .await
        .expect("idempotency key release should succeed");

    let third = store
        .record_outbound_action_idempotency(user_id, "notify:meeting-123", job_id)
        .await
        .expect("idempotency key should be reusable after release");
    assert!(third);
}

#[tokio::test]
#[serial]
async fn audit_metadata_redaction_masks_token_bearing_values() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let user_id = Uuid::new_v4();

    let mut metadata = HashMap::new();
    metadata.insert(
        "error_detail".to_string(),
        "authorization=Bearer secret".to_string(),
    );
    metadata.insert("status".to_string(), "failed".to_string());

    store
        .add_audit_event(
            user_id,
            "TEST_REDACTION",
            Some("google"),
            AuditResult::Failure,
            &metadata,
        )
        .await
        .expect("audit event insert should succeed");

    let (events, _cursor) = store
        .list_audit_events(user_id, None, 10)
        .await
        .expect("audit list should succeed");
    assert_eq!(events.len(), 1);

    let event = &events[0];
    assert_eq!(event.event_type, "TEST_REDACTION");
    assert_eq!(
        event.metadata.get("error_detail").map(String::as_str),
        Some("[REDACTED]")
    );
    assert_eq!(
        event.metadata.get("status").map(String::as_str),
        Some("failed")
    );
    assert!(
        !event
            .metadata
            .values()
            .any(|value| value.to_ascii_lowercase().contains("bearer")),
        "redacted metadata should never include token-bearing markers"
    );
}

#[tokio::test]
#[serial]
async fn connector_key_metadata_drift_conflict_fails_closed() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let user_id = Uuid::new_v4();
    let connector_id = store
        .upsert_google_connector(
            user_id,
            "refresh-token",
            &["https://www.googleapis.com/auth/calendar.readonly".to_string()],
            "__legacy__",
            1,
        )
        .await
        .expect("connector should be created");

    let mut tx = store
        .pool()
        .begin()
        .await
        .expect("transaction should start");
    sqlx::query(
        "UPDATE connectors
         SET token_key_id = $3, token_version = $4
         WHERE id = $1 AND user_id = $2",
    )
    .bind(connector_id)
    .bind(user_id)
    .bind("kms/drifted/other")
    .bind(9_i32)
    .execute(&mut *tx)
    .await
    .expect("concurrent drift update should apply inside transaction");

    let commit_task = tokio::spawn(async move {
        sleep(Duration::from_millis(200)).await;
        tx.commit()
            .await
            .expect("transaction commit should succeed");
    });

    let rotation_result = store
        .ensure_active_connector_key_metadata(
            user_id,
            connector_id,
            "kms/local/alfred-refresh-token",
            1,
        )
        .await;

    commit_task
        .await
        .expect("commit task should not panic or cancel");

    assert!(
        matches!(
            &rotation_result,
            Err(StoreError::InvalidData(message)) if message.contains("rotation conflict")
        ),
        "expected drift conflict invalid data error, got: {rotation_result:?}"
    );
}

#[tokio::test]
#[serial]
async fn lease_expiry_requeues_then_dead_letters_automation_run_jobs() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let user_id = Uuid::new_v4();
    let now = Utc::now();
    let job_id = store
        .enqueue_job(user_id, JobType::AutomationRun, now, None)
        .await
        .expect("job enqueue should succeed");
    sqlx::query("UPDATE jobs SET max_attempts = 2 WHERE id = $1")
        .bind(job_id)
        .execute(store.pool())
        .await
        .expect("max attempts update should succeed");

    let first_claim = store
        .claim_due_jobs(now, Uuid::new_v4(), 1, 1, 1)
        .await
        .expect("first claim should succeed");
    assert_eq!(first_claim.len(), 1);
    assert_eq!(first_claim[0].id, job_id);
    assert_eq!(first_claim[0].attempts, 0);

    let second_claim = store
        .claim_due_jobs(now + ChronoDuration::seconds(2), Uuid::new_v4(), 1, 1, 1)
        .await
        .expect("second claim should succeed after lease expiry");
    assert_eq!(second_claim.len(), 1);
    assert_eq!(second_claim[0].id, job_id);
    assert_eq!(second_claim[0].attempts, 1);

    let exhausted_claim = store
        .claim_due_jobs(now + ChronoDuration::seconds(4), Uuid::new_v4(), 1, 1, 1)
        .await
        .expect("third claim should succeed after second lease expiry");
    assert!(exhausted_claim.is_empty());

    let dead_letter =
        sqlx::query("SELECT attempts, reason_code FROM dead_letter_jobs WHERE job_id = $1")
            .bind(job_id)
            .fetch_one(store.pool())
            .await
            .expect("dead letter row should exist");
    let dead_letter_attempts: i32 = dead_letter
        .try_get("attempts")
        .expect("dead letter attempts should decode");
    let dead_letter_reason: String = dead_letter
        .try_get("reason_code")
        .expect("dead letter reason should decode");
    assert_eq!(dead_letter_attempts, 2);
    assert_eq!(dead_letter_reason, "LEASE_EXPIRED_MAX_ATTEMPTS");

    let job_row = sqlx::query("SELECT state, attempts FROM jobs WHERE id = $1")
        .bind(job_id)
        .fetch_one(store.pool())
        .await
        .expect("job row should exist");
    let state: String = job_row.try_get("state").expect("state should decode");
    let attempts: i32 = job_row.try_get("attempts").expect("attempts should decode");
    assert_eq!(state, "FAILED");
    assert_eq!(attempts, 2);
}
