mod support;

use chrono::{Duration, Utc};
use serial_test::serial;
use shared::models::AssistantSessionStateEnvelope;
use shared::repos::PrivacyDeleteStatus;
use uuid::Uuid;

#[tokio::test]
#[serial]
async fn oauth_state_is_user_scoped_single_use_and_ttl_bound() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let user_a = Uuid::new_v4();
    let user_b = Uuid::new_v4();
    let state_hash = b"state-hash-a";
    let now = Utc::now();

    store
        .store_oauth_state(
            user_a,
            state_hash,
            "alfred://oauth/google",
            now + Duration::minutes(5),
        )
        .await
        .expect("oauth state should store");

    let cross_user_redirect = store
        .consume_oauth_state(user_b, state_hash, now)
        .await
        .expect("cross-user consume should not fail");
    assert!(cross_user_redirect.is_none());

    let first_consume = store
        .consume_oauth_state(user_a, state_hash, now)
        .await
        .expect("first consume should succeed");
    assert_eq!(first_consume.as_deref(), Some("alfred://oauth/google"));

    let second_consume = store
        .consume_oauth_state(user_a, state_hash, now)
        .await
        .expect("second consume should not fail");
    assert!(second_consume.is_none());

    store
        .store_oauth_state(
            user_a,
            b"state-hash-expired",
            "alfred://oauth/google",
            now - Duration::seconds(1),
        )
        .await
        .expect("expired oauth state should store");

    let expired = store
        .consume_oauth_state(user_a, b"state-hash-expired", now)
        .await
        .expect("expired consume should not fail");
    assert!(expired.is_none());
}

#[tokio::test]
#[serial]
async fn queue_delete_all_deduplicates_and_hides_cross_user_status() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let user_a = Uuid::new_v4();
    let user_b = Uuid::new_v4();

    let first_request = store
        .queue_delete_all(user_a)
        .await
        .expect("initial delete-all queue should succeed");
    let second_request = store
        .queue_delete_all(user_a)
        .await
        .expect("deduped delete-all queue should succeed");

    assert_eq!(first_request, second_request);

    let status = store
        .get_delete_request_status(user_a, first_request)
        .await
        .expect("status lookup should succeed")
        .expect("request status should exist");
    assert!(matches!(status.status, PrivacyDeleteStatus::Queued));

    let cross_user_status = store
        .get_delete_request_status(user_b, first_request)
        .await
        .expect("cross-user status lookup should succeed");
    assert!(cross_user_status.is_none());

    let pending = store
        .count_pending_delete_requests()
        .await
        .expect("pending count should succeed");
    assert_eq!(pending, 1);
}

#[tokio::test]
#[serial]
async fn delete_request_claim_and_completion_require_correct_worker_lease() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let user_id = Uuid::new_v4();
    let request_id = store
        .queue_delete_all(user_id)
        .await
        .expect("delete-all queue should succeed");

    let now = Utc::now();
    let worker_id = Uuid::new_v4();
    let claims = store
        .claim_delete_requests(now, worker_id, 10, 120)
        .await
        .expect("claim should succeed");

    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0].id, request_id);
    assert_eq!(claims[0].user_id, user_id);

    let wrong_worker = store
        .mark_delete_request_completed(request_id, Uuid::new_v4(), now)
        .await
        .expect("wrong worker completion should not fail");
    assert!(!wrong_worker);

    let completed = store
        .mark_delete_request_completed(request_id, worker_id, now)
        .await
        .expect("correct worker completion should not fail");
    assert!(completed);

    let status = store
        .get_delete_request_status(user_id, request_id)
        .await
        .expect("status lookup should succeed")
        .expect("request status should exist");
    assert!(matches!(status.status, PrivacyDeleteStatus::Completed));
    assert!(status.completed_at.is_some());

    let pending = store
        .count_pending_delete_requests()
        .await
        .expect("pending count should succeed");
    assert_eq!(pending, 0);
}

#[tokio::test]
#[serial]
async fn assistant_encrypted_session_is_user_scoped_and_expires() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let now = Utc::now();
    let user_a = Uuid::new_v4();
    let user_b = Uuid::new_v4();
    let session_id = Uuid::new_v4();

    let state = AssistantSessionStateEnvelope {
        version: "v1".to_string(),
        algorithm: "x25519-chacha20poly1305".to_string(),
        key_id: "assistant-ingress-v1".to_string(),
        nonce: "nonce-a".to_string(),
        ciphertext: "ciphertext-a".to_string(),
        expires_at: now + Duration::minutes(10),
    };

    store
        .upsert_assistant_encrypted_session(user_a, session_id, &state, now, 1)
        .await
        .expect("session upsert should succeed");

    let cross_user = store
        .load_assistant_encrypted_session(user_b, session_id, now)
        .await
        .expect("cross-user lookup should succeed");
    assert!(cross_user.is_none());

    let loaded = store
        .load_assistant_encrypted_session(user_a, session_id, now)
        .await
        .expect("session lookup should succeed")
        .expect("session should exist before ttl expiry");
    assert_eq!(loaded.session_id, session_id);
    assert_eq!(loaded.state.ciphertext, "ciphertext-a");

    let expired_lookup = store
        .load_assistant_encrypted_session(user_a, session_id, now + Duration::seconds(2))
        .await
        .expect("expired lookup should succeed");
    assert!(expired_lookup.is_none());

    let remaining_rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint
         FROM assistant_encrypted_sessions
         WHERE user_id = $1",
    )
    .bind(user_a)
    .fetch_one(store.pool())
    .await
    .expect("session count query should succeed");
    assert_eq!(remaining_rows, 0);
}

#[tokio::test]
#[serial]
async fn assistant_encrypted_session_global_purge_is_bounded_and_non_traffic_dependent() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let now = Utc::now();
    let expired_now = now - Duration::days(61);
    let user_a = Uuid::new_v4();
    let user_b = Uuid::new_v4();
    let user_c = Uuid::new_v4();
    let session_a = Uuid::new_v4();
    let session_b = Uuid::new_v4();
    let session_c = Uuid::new_v4();

    let expired_state = AssistantSessionStateEnvelope {
        version: "v1".to_string(),
        algorithm: "x25519-chacha20poly1305".to_string(),
        key_id: "assistant-ingress-v1".to_string(),
        nonce: "nonce-expired".to_string(),
        ciphertext: "ciphertext-expired".to_string(),
        expires_at: expired_now,
    };
    let active_state = AssistantSessionStateEnvelope {
        version: "v1".to_string(),
        algorithm: "x25519-chacha20poly1305".to_string(),
        key_id: "assistant-ingress-v1".to_string(),
        nonce: "nonce-active".to_string(),
        ciphertext: "ciphertext-active".to_string(),
        expires_at: now + Duration::days(30),
    };

    store
        .upsert_assistant_encrypted_session(user_a, session_a, &expired_state, expired_now, 1)
        .await
        .expect("user-a expired session insert should succeed");
    store
        .upsert_assistant_encrypted_session(user_b, session_b, &expired_state, expired_now, 1)
        .await
        .expect("user-b expired session insert should succeed");
    store
        .upsert_assistant_encrypted_session(
            user_c,
            session_c,
            &active_state,
            now,
            60 * 24 * 60 * 60,
        )
        .await
        .expect("user-c active session insert should succeed");

    let expired_before: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint
         FROM assistant_encrypted_sessions
         WHERE expires_at <= $1",
    )
    .bind(now)
    .fetch_one(store.pool())
    .await
    .expect("expired session pre-count query should succeed");
    assert_eq!(expired_before, 2);

    let first_batch = store
        .purge_expired_assistant_encrypted_sessions_batch(now, 1)
        .await
        .expect("first global purge batch should succeed");
    assert_eq!(first_batch, 1);

    let expired_after_first_batch: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint
         FROM assistant_encrypted_sessions
         WHERE expires_at <= $1",
    )
    .bind(now)
    .fetch_one(store.pool())
    .await
    .expect("expired session count after first batch should succeed");
    assert_eq!(expired_after_first_batch, 1);

    let second_batch = store
        .purge_expired_assistant_encrypted_sessions_batch(now, 10)
        .await
        .expect("second global purge batch should succeed");
    assert_eq!(second_batch, 1);

    let expired_after_second_batch: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint
         FROM assistant_encrypted_sessions
         WHERE expires_at <= $1",
    )
    .bind(now)
    .fetch_one(store.pool())
    .await
    .expect("expired session count after second batch should succeed");
    assert_eq!(expired_after_second_batch, 0);

    let active_remaining: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint
         FROM assistant_encrypted_sessions
         WHERE user_id = $1
           AND session_id = $2
           AND expires_at > $3",
    )
    .bind(user_c)
    .bind(session_c)
    .bind(now)
    .fetch_one(store.pool())
    .await
    .expect("active session remaining query should succeed");
    assert_eq!(active_remaining, 1);
}

#[tokio::test]
#[serial]
async fn assistant_encrypted_session_user_scoped_purge_is_bounded_per_call() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let now = Utc::now();
    let expired_now = now - Duration::days(61);
    let user_id = Uuid::new_v4();

    let expired_state = AssistantSessionStateEnvelope {
        version: "v1".to_string(),
        algorithm: "x25519-chacha20poly1305".to_string(),
        key_id: "assistant-ingress-v1".to_string(),
        nonce: "nonce-expired-user-purge".to_string(),
        ciphertext: "ciphertext-expired-user-purge".to_string(),
        expires_at: expired_now,
    };

    for _ in 0..201 {
        store
            .upsert_assistant_encrypted_session(
                user_id,
                Uuid::new_v4(),
                &expired_state,
                expired_now,
                1,
            )
            .await
            .expect("expired session insert should succeed");
    }

    let first_expired_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint
         FROM assistant_encrypted_sessions
         WHERE user_id = $1
           AND expires_at <= $2",
    )
    .bind(user_id)
    .bind(now)
    .fetch_one(store.pool())
    .await
    .expect("expired count query before purge should succeed");
    assert_eq!(first_expired_count, 201);

    let _ = store
        .load_assistant_encrypted_session(user_id, Uuid::new_v4(), now)
        .await
        .expect("user-scoped lookup should succeed");

    let after_single_lookup_purge: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint
         FROM assistant_encrypted_sessions
         WHERE user_id = $1
           AND expires_at <= $2",
    )
    .bind(user_id)
    .bind(now)
    .fetch_one(store.pool())
    .await
    .expect("expired count query after purge should succeed");
    assert_eq!(after_single_lookup_purge, 1);
}
