mod support;

use std::collections::HashMap;

use chrono::{Duration, Utc};
use serial_test::serial;
use shared::models::{ApnsEnvironment, AssistantSessionStateEnvelope};
use shared::repos::{AuditResult, JobType};
use sqlx::Row;
use uuid::Uuid;

#[tokio::test]
#[serial]
async fn purge_user_operational_data_removes_sensitive_rows_and_marks_user_deleted() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let now = Utc::now();
    let user_id = Uuid::new_v4();
    store
        .ensure_user(user_id)
        .await
        .expect("ensure user should succeed");

    store
        .upsert_google_connector(
            user_id,
            "refresh-token",
            &["https://www.googleapis.com/auth/calendar.readonly".to_string()],
            "kms/local/alfred-refresh-token",
            1,
        )
        .await
        .expect("connector upsert should succeed");

    store
        .register_device(
            user_id,
            "device-1",
            "apns-token",
            &ApnsEnvironment::Sandbox,
            None,
            None,
        )
        .await
        .expect("device registration should succeed");

    store
        .enqueue_job(
            user_id,
            JobType::AutomationRun,
            now,
            Some(b"opaque-payload"),
        )
        .await
        .expect("job enqueue should succeed");

    store
        .store_oauth_state(
            user_id,
            b"oauth-state-to-purge",
            "alfred://oauth/google",
            now + Duration::minutes(5),
        )
        .await
        .expect("oauth state should store");

    let mut metadata = HashMap::new();
    metadata.insert("refresh_token".to_string(), "leak-me".to_string());
    store
        .add_audit_event(
            user_id,
            "TEST_EVENT",
            Some("google"),
            AuditResult::Failure,
            &metadata,
        )
        .await
        .expect("audit event should store");

    store
        .get_or_create_preferences(user_id)
        .await
        .expect("default preferences should be created");

    let session_state = AssistantSessionStateEnvelope {
        version: "v1".to_string(),
        algorithm: "x25519-chacha20poly1305".to_string(),
        key_id: "assistant-ingress-v1".to_string(),
        nonce: "nonce-session".to_string(),
        ciphertext: "ciphertext-session".to_string(),
        expires_at: now + Duration::minutes(10),
    };

    store
        .upsert_assistant_encrypted_session(user_id, Uuid::new_v4(), &session_state, now, 600)
        .await
        .expect("session upsert should succeed");

    store
        .purge_user_operational_data(user_id)
        .await
        .expect("purge should succeed");

    assert_eq!(row_count(store.pool(), "connectors", user_id).await, 0);
    assert_eq!(row_count(store.pool(), "devices", user_id).await, 0);
    assert_eq!(row_count(store.pool(), "jobs", user_id).await, 0);
    assert_eq!(row_count(store.pool(), "oauth_states", user_id).await, 0);
    assert_eq!(row_count(store.pool(), "audit_events", user_id).await, 0);
    assert_eq!(
        row_count(store.pool(), "user_preferences", user_id).await,
        0
    );
    assert_eq!(
        row_count(store.pool(), "assistant_encrypted_sessions", user_id).await,
        0
    );

    let user_row = sqlx::query("SELECT status FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(store.pool())
        .await
        .expect("user row should exist");
    let status: String = user_row
        .try_get("status")
        .expect("status column should decode");
    assert_eq!(status, "DELETED");
}

async fn row_count(pool: &sqlx::PgPool, table: &str, user_id: Uuid) -> i64 {
    let query = format!("SELECT COUNT(*)::bigint FROM {table} WHERE user_id = $1");
    sqlx::query_scalar(&query)
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("row count query should succeed")
}
