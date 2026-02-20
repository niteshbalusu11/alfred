mod support;

use chrono::{Duration as ChronoDuration, Utc};
use serial_test::serial;
use tokio::join;
use uuid::Uuid;

const PROMPT_HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const PROMPT_HASH_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

#[tokio::test]
#[serial]
async fn automation_rule_crud_pause_resume_and_delete_flow() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let user_id = Uuid::new_v4();
    let now = Utc::now();
    let next_run_at = now + ChronoDuration::minutes(5);
    let prompt_ciphertext = b"sealed-automation-prompt-v1";

    let created = store
        .create_automation_rule(
            user_id,
            900,
            "America/Los_Angeles",
            next_run_at,
            prompt_ciphertext,
            PROMPT_HASH_A,
        )
        .await
        .expect("rule should be created");

    assert_eq!(created.user_id, user_id);
    assert_eq!(created.interval_seconds, 900);
    assert_eq!(created.prompt_sha256, PROMPT_HASH_A);
    assert_eq!(created.time_zone, "America/Los_Angeles");
    assert_eq!(created.status.as_str(), "ACTIVE");

    let fetched = store
        .get_automation_rule(user_id, created.id)
        .await
        .expect("rule fetch should succeed")
        .expect("rule should exist");
    assert_eq!(fetched.id, created.id);

    let updated_schedule = store
        .update_automation_rule_schedule(
            user_id,
            created.id,
            1_200,
            "America/New_York",
            next_run_at + ChronoDuration::minutes(10),
        )
        .await
        .expect("schedule update should succeed")
        .expect("rule should exist");
    assert_eq!(updated_schedule.interval_seconds, 1_200);
    assert_eq!(updated_schedule.time_zone, "America/New_York");

    let updated_prompt = store
        .update_automation_rule_prompt(
            user_id,
            created.id,
            b"sealed-automation-prompt-v2",
            PROMPT_HASH_B,
        )
        .await
        .expect("prompt update should succeed")
        .expect("rule should exist");
    assert_eq!(updated_prompt.prompt_sha256, PROMPT_HASH_B);

    let paused = store
        .pause_automation_rule(user_id, created.id)
        .await
        .expect("pause should succeed");
    assert!(paused);

    let resumed = store
        .resume_automation_rule(
            user_id,
            created.id,
            next_run_at + ChronoDuration::minutes(30),
        )
        .await
        .expect("resume should succeed");
    assert!(resumed);

    let listed = store
        .list_automation_rules(user_id, 10)
        .await
        .expect("list should succeed");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, created.id);

    let deleted = store
        .delete_automation_rule(user_id, created.id)
        .await
        .expect("delete should succeed");
    assert!(deleted);

    let missing = store
        .get_automation_rule(user_id, created.id)
        .await
        .expect("lookup should succeed");
    assert!(missing.is_none());
}

#[tokio::test]
#[serial]
async fn due_claims_are_lease_safe_and_split_across_workers() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let now = Utc::now();
    let rule_a = store
        .create_automation_rule(
            Uuid::new_v4(),
            600,
            "UTC",
            now - ChronoDuration::minutes(1),
            b"prompt-a",
            PROMPT_HASH_A,
        )
        .await
        .expect("rule a should be created");
    let rule_b = store
        .create_automation_rule(
            Uuid::new_v4(),
            600,
            "UTC",
            now - ChronoDuration::minutes(1),
            b"prompt-b",
            PROMPT_HASH_B,
        )
        .await
        .expect("rule b should be created");

    let worker_a = Uuid::new_v4();
    let worker_b = Uuid::new_v4();
    let (claims_a, claims_b) = join!(
        store.claim_due_automation_rules(now, worker_a, 1, 300),
        store.claim_due_automation_rules(now, worker_b, 1, 300),
    );
    let claims_a = claims_a.expect("worker a claim should succeed");
    let claims_b = claims_b.expect("worker b claim should succeed");

    assert_eq!(claims_a.len(), 1);
    assert_eq!(claims_b.len(), 1);

    let mut claimed_ids = vec![claims_a[0].id, claims_b[0].id];
    claimed_ids.sort();
    claimed_ids.dedup();
    assert_eq!(
        claimed_ids.len(),
        2,
        "workers should not claim the same rule"
    );
    assert!(claimed_ids.contains(&rule_a.id));
    assert!(claimed_ids.contains(&rule_b.id));

    let prompt_bytes = if claims_a[0].id == rule_a.id {
        &claims_a[0].prompt_ciphertext
    } else {
        &claims_b[0].prompt_ciphertext
    };
    assert_eq!(prompt_bytes, b"prompt-a");
}

#[tokio::test]
#[serial]
async fn run_materialization_is_idempotent_for_same_rule_and_scheduled_time() {
    let store = support::test_store().await;
    support::reset_database(store.pool()).await;

    let user_id = Uuid::new_v4();
    let now = Utc::now();
    let interval = 900_i32;
    let scheduled_for = now - ChronoDuration::minutes(1);
    let next_run_at = now + ChronoDuration::minutes(14);

    let rule = store
        .create_automation_rule(
            user_id,
            interval as u32,
            "UTC",
            scheduled_for,
            b"prompt-c",
            PROMPT_HASH_A,
        )
        .await
        .expect("rule should be created");

    let worker_a = Uuid::new_v4();
    let claims_a = store
        .claim_due_automation_rules(now, worker_a, 1, 300)
        .await
        .expect("claim should succeed");
    assert_eq!(claims_a.len(), 1);

    let run_first = store
        .materialize_automation_run(
            rule.id,
            worker_a,
            scheduled_for,
            next_run_at,
            "automation:run:001",
        )
        .await
        .expect("first materialization should succeed")
        .expect("lease owner should materialize run");

    store
        .update_automation_rule_schedule(user_id, rule.id, interval as u32, "UTC", scheduled_for)
        .await
        .expect("rule schedule reset should succeed")
        .expect("rule should still exist");

    let worker_b = Uuid::new_v4();
    let claims_b = store
        .claim_due_automation_rules(now, worker_b, 1, 300)
        .await
        .expect("second claim should succeed");
    assert_eq!(claims_b.len(), 1);

    let run_second = store
        .materialize_automation_run(
            rule.id,
            worker_b,
            scheduled_for,
            next_run_at + ChronoDuration::minutes(15),
            "automation:run:001",
        )
        .await
        .expect("second materialization should succeed")
        .expect("lease owner should materialize run");

    assert_eq!(run_first.id, run_second.id);

    let runs = store
        .list_automation_runs_for_rule(user_id, rule.id, 10)
        .await
        .expect("run list should succeed");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].scheduled_for, scheduled_for);
}
