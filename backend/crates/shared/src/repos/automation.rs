use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::{DateTime, Duration, Utc};
use sqlx::Row;
use uuid::Uuid;

use crate::timezone::normalize_time_zone;

use super::{
    AutomationRuleRecord, AutomationRuleStatus, AutomationRunRecord, AutomationRunState,
    AutomationScheduleType, ClaimedAutomationRule, Store, StoreError,
};

const MIN_INTERVAL_SECONDS: u32 = 60;
const MAX_INTERVAL_SECONDS: u32 = 604_800;

impl Store {
    pub async fn create_automation_rule(
        &self,
        user_id: Uuid,
        interval_seconds: u32,
        time_zone: &str,
        next_run_at: DateTime<Utc>,
        prompt_ciphertext: &[u8],
        prompt_sha256: &str,
    ) -> Result<AutomationRuleRecord, StoreError> {
        self.ensure_user(user_id).await?;
        let interval_seconds = validated_interval_seconds(interval_seconds)?;
        let time_zone = normalized_time_zone(time_zone)?;
        let prompt_sha256 = normalized_prompt_sha256(prompt_sha256)?;

        let row = sqlx::query(
            "INSERT INTO automation_rules (
                user_id,
                status,
                schedule_type,
                interval_seconds,
                time_zone,
                next_run_at,
                prompt_ciphertext,
                prompt_sha256
             ) VALUES (
                $1,
                'ACTIVE',
                'INTERVAL_SECONDS',
                $2,
                $3,
                $4,
                pgp_sym_encrypt(encode($5, 'base64'), $6),
                $7
             )
             RETURNING
                id,
                user_id,
                status,
                schedule_type,
                interval_seconds,
                time_zone,
                next_run_at,
                last_run_at,
                prompt_sha256,
                created_at,
                updated_at",
        )
        .bind(user_id)
        .bind(interval_seconds)
        .bind(time_zone)
        .bind(next_run_at)
        .bind(prompt_ciphertext)
        .bind(&self.data_encryption_key)
        .bind(prompt_sha256)
        .fetch_one(&self.pool)
        .await?;

        automation_rule_from_row(&row)
    }

    pub async fn get_automation_rule(
        &self,
        user_id: Uuid,
        rule_id: Uuid,
    ) -> Result<Option<AutomationRuleRecord>, StoreError> {
        let row = sqlx::query(
            "SELECT
                id,
                user_id,
                status,
                schedule_type,
                interval_seconds,
                time_zone,
                next_run_at,
                last_run_at,
                prompt_sha256,
                created_at,
                updated_at
             FROM automation_rules
             WHERE user_id = $1
               AND id = $2",
        )
        .bind(user_id)
        .bind(rule_id)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| automation_rule_from_row(&row)).transpose()
    }

    pub async fn list_automation_rules(
        &self,
        user_id: Uuid,
        limit: i64,
    ) -> Result<Vec<AutomationRuleRecord>, StoreError> {
        if limit <= 0 {
            return Err(StoreError::InvalidData(
                "automation list limit must be > 0".to_string(),
            ));
        }

        let rows = sqlx::query(
            "SELECT
                id,
                user_id,
                status,
                schedule_type,
                interval_seconds,
                time_zone,
                next_run_at,
                last_run_at,
                prompt_sha256,
                created_at,
                updated_at
             FROM automation_rules
             WHERE user_id = $1
             ORDER BY created_at DESC, id DESC
             LIMIT $2",
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| automation_rule_from_row(&row))
            .collect()
    }

    pub async fn update_automation_rule_schedule(
        &self,
        user_id: Uuid,
        rule_id: Uuid,
        interval_seconds: u32,
        time_zone: &str,
        next_run_at: DateTime<Utc>,
    ) -> Result<Option<AutomationRuleRecord>, StoreError> {
        let interval_seconds = validated_interval_seconds(interval_seconds)?;
        let time_zone = normalized_time_zone(time_zone)?;

        let row = sqlx::query(
            "UPDATE automation_rules
             SET interval_seconds = $3,
                 time_zone = $4,
                 next_run_at = $5,
                 updated_at = NOW()
             WHERE user_id = $1
               AND id = $2
             RETURNING
                id,
                user_id,
                status,
                schedule_type,
                interval_seconds,
                time_zone,
                next_run_at,
                last_run_at,
                prompt_sha256,
                created_at,
                updated_at",
        )
        .bind(user_id)
        .bind(rule_id)
        .bind(interval_seconds)
        .bind(time_zone)
        .bind(next_run_at)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| automation_rule_from_row(&row)).transpose()
    }

    pub async fn update_automation_rule_prompt(
        &self,
        user_id: Uuid,
        rule_id: Uuid,
        prompt_ciphertext: &[u8],
        prompt_sha256: &str,
    ) -> Result<Option<AutomationRuleRecord>, StoreError> {
        let prompt_sha256 = normalized_prompt_sha256(prompt_sha256)?;

        let row = sqlx::query(
            "UPDATE automation_rules
             SET prompt_ciphertext = pgp_sym_encrypt(encode($3, 'base64'), $5),
                 prompt_sha256 = $4,
                 updated_at = NOW()
             WHERE user_id = $1
               AND id = $2
             RETURNING
                id,
                user_id,
                status,
                schedule_type,
                interval_seconds,
                time_zone,
                next_run_at,
                last_run_at,
                prompt_sha256,
                created_at,
                updated_at",
        )
        .bind(user_id)
        .bind(rule_id)
        .bind(prompt_ciphertext)
        .bind(prompt_sha256)
        .bind(&self.data_encryption_key)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| automation_rule_from_row(&row)).transpose()
    }

    pub async fn pause_automation_rule(
        &self,
        user_id: Uuid,
        rule_id: Uuid,
    ) -> Result<bool, StoreError> {
        let result = sqlx::query(
            "UPDATE automation_rules
             SET status = 'PAUSED',
                 lease_owner = NULL,
                 lease_expires_at = NULL,
                 updated_at = NOW()
             WHERE user_id = $1
               AND id = $2",
        )
        .bind(user_id)
        .bind(rule_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn resume_automation_rule(
        &self,
        user_id: Uuid,
        rule_id: Uuid,
        next_run_at: DateTime<Utc>,
    ) -> Result<bool, StoreError> {
        let result = sqlx::query(
            "UPDATE automation_rules
             SET status = 'ACTIVE',
                 next_run_at = $3,
                 lease_owner = NULL,
                 lease_expires_at = NULL,
                 updated_at = NOW()
             WHERE user_id = $1
               AND id = $2",
        )
        .bind(user_id)
        .bind(rule_id)
        .bind(next_run_at)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_automation_rule(
        &self,
        user_id: Uuid,
        rule_id: Uuid,
    ) -> Result<bool, StoreError> {
        let result = sqlx::query(
            "DELETE FROM automation_rules
             WHERE user_id = $1
               AND id = $2",
        )
        .bind(user_id)
        .bind(rule_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn claim_due_automation_rules(
        &self,
        now: DateTime<Utc>,
        worker_id: Uuid,
        max_rules: i64,
        lease_seconds: i64,
    ) -> Result<Vec<ClaimedAutomationRule>, StoreError> {
        if max_rules <= 0 {
            return Ok(Vec::new());
        }
        if lease_seconds <= 0 {
            return Err(StoreError::InvalidData(
                "automation lease_seconds must be > 0".to_string(),
            ));
        }

        sqlx::query(
            "UPDATE automation_rules
             SET lease_owner = NULL,
                 lease_expires_at = NULL,
                 updated_at = NOW()
             WHERE status = 'ACTIVE'
               AND lease_expires_at IS NOT NULL
               AND lease_expires_at <= $1",
        )
        .bind(now)
        .execute(&self.pool)
        .await?;

        let lease_until = now + Duration::seconds(lease_seconds);
        let worker_id = worker_id.to_string();
        let rows = sqlx::query(
            "WITH candidate_ids AS (
                SELECT id
                FROM automation_rules
                WHERE status = 'ACTIVE'
                  AND next_run_at <= $1
                  AND (lease_expires_at IS NULL OR lease_expires_at <= $1)
                ORDER BY next_run_at ASC, id ASC
                LIMIT $2
                FOR UPDATE SKIP LOCKED
             ),
             claimed AS (
                UPDATE automation_rules r
                SET lease_owner = $3,
                    lease_expires_at = $4,
                    updated_at = NOW()
                FROM candidate_ids c
                WHERE r.id = c.id
                RETURNING
                    r.id,
                    r.user_id,
                    r.interval_seconds,
                    r.time_zone,
                    r.next_run_at,
                    r.prompt_sha256,
                    pgp_sym_decrypt(r.prompt_ciphertext, $5) AS prompt_encoded
             )
             SELECT
                id,
                user_id,
                interval_seconds,
                time_zone,
                next_run_at,
                prompt_sha256,
                prompt_encoded
             FROM claimed
             ORDER BY next_run_at ASC, id ASC",
        )
        .bind(now)
        .bind(max_rules)
        .bind(worker_id)
        .bind(lease_until)
        .bind(&self.data_encryption_key)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(claimed_automation_rule_from_row)
            .collect()
    }

    pub async fn materialize_automation_run(
        &self,
        rule_id: Uuid,
        worker_id: Uuid,
        scheduled_for: DateTime<Utc>,
        next_run_at: DateTime<Utc>,
        idempotency_key: &str,
    ) -> Result<Option<AutomationRunRecord>, StoreError> {
        if idempotency_key.trim().is_empty() {
            return Err(StoreError::InvalidData(
                "automation idempotency_key must not be empty".to_string(),
            ));
        }

        let mut tx = self.pool.begin().await?;
        let Some(user_id) = sqlx::query_scalar::<_, Uuid>(
            "SELECT user_id
             FROM automation_rules
             WHERE id = $1
               AND status = 'ACTIVE'
               AND lease_owner = $2
             FOR UPDATE",
        )
        .bind(rule_id)
        .bind(worker_id.to_string())
        .fetch_optional(&mut *tx)
        .await?
        else {
            tx.rollback().await?;
            return Ok(None);
        };

        let run_row = sqlx::query(
            "INSERT INTO automation_runs (
                rule_id,
                user_id,
                scheduled_for,
                idempotency_key,
                state
             ) VALUES (
                $1,
                $2,
                $3,
                $4,
                'MATERIALIZED'
             )
             ON CONFLICT (rule_id, scheduled_for)
             DO UPDATE SET
                idempotency_key = EXCLUDED.idempotency_key,
                updated_at = NOW()
             RETURNING
                id,
                rule_id,
                user_id,
                scheduled_for,
                job_id,
                idempotency_key,
                state,
                created_at,
                updated_at",
        )
        .bind(rule_id)
        .bind(user_id)
        .bind(scheduled_for)
        .bind(idempotency_key.trim())
        .fetch_one(&mut *tx)
        .await?;

        let update = sqlx::query(
            "UPDATE automation_rules
             SET last_run_at = CASE
                    WHEN last_run_at IS NULL OR last_run_at < $3 THEN $3
                    ELSE last_run_at
                 END,
                 next_run_at = CASE
                    WHEN next_run_at < $4 THEN $4
                    ELSE next_run_at
                 END,
                 lease_owner = NULL,
                 lease_expires_at = NULL,
                 updated_at = NOW()
             WHERE id = $1
               AND status = 'ACTIVE'
               AND lease_owner = $2",
        )
        .bind(rule_id)
        .bind(worker_id.to_string())
        .bind(scheduled_for)
        .bind(next_run_at)
        .execute(&mut *tx)
        .await?;

        if update.rows_affected() == 0 {
            tx.rollback().await?;
            return Ok(None);
        }

        tx.commit().await?;
        Ok(Some(automation_run_from_row(&run_row)?))
    }

    pub async fn list_automation_runs_for_rule(
        &self,
        user_id: Uuid,
        rule_id: Uuid,
        limit: i64,
    ) -> Result<Vec<AutomationRunRecord>, StoreError> {
        if limit <= 0 {
            return Err(StoreError::InvalidData(
                "automation run list limit must be > 0".to_string(),
            ));
        }

        let rows = sqlx::query(
            "SELECT
                id,
                rule_id,
                user_id,
                scheduled_for,
                job_id,
                idempotency_key,
                state,
                created_at,
                updated_at
             FROM automation_runs
             WHERE user_id = $1
               AND rule_id = $2
             ORDER BY scheduled_for DESC, id DESC
             LIMIT $3",
        )
        .bind(user_id)
        .bind(rule_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| automation_run_from_row(&row))
            .collect()
    }
}

fn automation_rule_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<AutomationRuleRecord, StoreError> {
    let status: String = row.try_get("status")?;
    let schedule_type: String = row.try_get("schedule_type")?;
    Ok(AutomationRuleRecord {
        id: row.try_get("id")?,
        user_id: row.try_get("user_id")?,
        status: AutomationRuleStatus::from_db(&status)?,
        schedule_type: AutomationScheduleType::from_db(&schedule_type)?,
        interval_seconds: row.try_get("interval_seconds")?,
        time_zone: row.try_get("time_zone")?,
        next_run_at: row.try_get("next_run_at")?,
        last_run_at: row.try_get("last_run_at")?,
        prompt_sha256: row.try_get("prompt_sha256")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

fn claimed_automation_rule_from_row(
    row: sqlx::postgres::PgRow,
) -> Result<ClaimedAutomationRule, StoreError> {
    let prompt_encoded: String = row.try_get("prompt_encoded")?;
    let prompt_ciphertext = STANDARD
        .decode(prompt_encoded.as_bytes())
        .map_err(|_| StoreError::InvalidData("automation prompt decode failed".to_string()))?;

    Ok(ClaimedAutomationRule {
        id: row.try_get("id")?,
        user_id: row.try_get("user_id")?,
        interval_seconds: row.try_get("interval_seconds")?,
        time_zone: row.try_get("time_zone")?,
        next_run_at: row.try_get("next_run_at")?,
        prompt_ciphertext,
        prompt_sha256: row.try_get("prompt_sha256")?,
    })
}

fn automation_run_from_row(row: &sqlx::postgres::PgRow) -> Result<AutomationRunRecord, StoreError> {
    let state: String = row.try_get("state")?;
    Ok(AutomationRunRecord {
        id: row.try_get("id")?,
        rule_id: row.try_get("rule_id")?,
        user_id: row.try_get("user_id")?,
        scheduled_for: row.try_get("scheduled_for")?,
        job_id: row.try_get("job_id")?,
        idempotency_key: row.try_get("idempotency_key")?,
        state: AutomationRunState::from_db(&state)?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

fn validated_interval_seconds(value: u32) -> Result<i32, StoreError> {
    if !(MIN_INTERVAL_SECONDS..=MAX_INTERVAL_SECONDS).contains(&value) {
        return Err(StoreError::InvalidData(format!(
            "interval_seconds must be between {MIN_INTERVAL_SECONDS} and {MAX_INTERVAL_SECONDS}"
        )));
    }

    i32::try_from(value)
        .map_err(|_| StoreError::InvalidData("interval_seconds exceeds i32 bounds".to_string()))
}

fn normalized_time_zone(value: &str) -> Result<String, StoreError> {
    normalize_time_zone(value).ok_or_else(|| {
        StoreError::InvalidData("time_zone is not a valid IANA timezone".to_string())
    })
}

fn normalized_prompt_sha256(value: &str) -> Result<String, StoreError> {
    let trimmed = value.trim();
    if trimmed.len() != 64 || !trimmed.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(StoreError::InvalidData(
            "prompt_sha256 must be a 64-character hex digest".to_string(),
        ));
    }

    Ok(trimmed.to_ascii_lowercase())
}
