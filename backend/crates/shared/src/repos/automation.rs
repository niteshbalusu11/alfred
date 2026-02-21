use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::{DateTime, Duration, Utc};
use sqlx::Row;
use uuid::Uuid;

use crate::automation_schedule::{
    AutomationScheduleSpec, interval_seconds_hint, validate_schedule_spec,
};
use crate::timezone::normalize_time_zone;

use super::{
    AutomationPromptMaterial, AutomationRuleRecord, AutomationRuleStatus, AutomationScheduleType,
    ClaimedAutomationRule, Store, StoreError,
};

impl Store {
    pub async fn create_automation_rule(
        &self,
        user_id: Uuid,
        schedule: &AutomationScheduleSpec,
        next_run_at: DateTime<Utc>,
        prompt_ciphertext: &[u8],
        prompt_sha256: &str,
    ) -> Result<AutomationRuleRecord, StoreError> {
        self.ensure_user(user_id).await?;
        let schedule = normalized_schedule_spec(schedule)?;
        let prompt_sha256 = normalized_prompt_sha256(prompt_sha256)?;

        let row = sqlx::query(
            "INSERT INTO automation_rules (
                user_id,
                status,
                schedule_type,
                interval_seconds,
                time_zone,
                local_time_minutes,
                anchor_day_of_week,
                anchor_day_of_month,
                anchor_month,
                next_run_at,
                prompt_ciphertext,
                prompt_sha256
             ) VALUES (
                $1,
                'ACTIVE',
                $2,
                $3,
                $4,
                $5,
                $6,
                $7,
                $8,
                $9,
                pgp_sym_encrypt(encode($10, 'base64'), $11),
                $12
             )
             RETURNING
                id,
                user_id,
                status,
                schedule_type,
                local_time_minutes,
                anchor_day_of_week,
                anchor_day_of_month,
                anchor_month,
                time_zone,
                next_run_at,
                last_run_at,
                prompt_sha256,
                created_at,
                updated_at",
        )
        .bind(user_id)
        .bind(schedule.schedule_type.as_str())
        .bind(interval_seconds_hint(schedule.schedule_type))
        .bind(schedule.time_zone.as_str())
        .bind(i32::from(schedule.local_time_minutes))
        .bind(schedule.anchor_day_of_week.map(i16::from))
        .bind(schedule.anchor_day_of_month.map(i16::from))
        .bind(schedule.anchor_month.map(i16::from))
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
                local_time_minutes,
                anchor_day_of_week,
                anchor_day_of_month,
                anchor_month,
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

    pub async fn get_automation_rule_prompt_material(
        &self,
        user_id: Uuid,
        rule_id: Uuid,
    ) -> Result<Option<AutomationPromptMaterial>, StoreError> {
        let row = sqlx::query(
            "SELECT
                prompt_sha256,
                pgp_sym_decrypt(prompt_ciphertext, $3) AS prompt_encoded
             FROM automation_rules
             WHERE user_id = $1
               AND id = $2",
        )
        .bind(user_id)
        .bind(rule_id)
        .bind(&self.data_encryption_key)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| automation_prompt_material_from_row(&row))
            .transpose()
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
                local_time_minutes,
                anchor_day_of_week,
                anchor_day_of_month,
                anchor_month,
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
        schedule: &AutomationScheduleSpec,
        next_run_at: DateTime<Utc>,
    ) -> Result<Option<AutomationRuleRecord>, StoreError> {
        let schedule = normalized_schedule_spec(schedule)?;

        let row = sqlx::query(
            "UPDATE automation_rules
             SET schedule_type = $3,
                 interval_seconds = $4,
                 time_zone = $5,
                 local_time_minutes = $6,
                 anchor_day_of_week = $7,
                 anchor_day_of_month = $8,
                 anchor_month = $9,
                 next_run_at = $10,
                 updated_at = NOW()
             WHERE user_id = $1
               AND id = $2
             RETURNING
                id,
                user_id,
                status,
                schedule_type,
                local_time_minutes,
                anchor_day_of_week,
                anchor_day_of_month,
                anchor_month,
                time_zone,
                next_run_at,
                last_run_at,
                prompt_sha256,
                created_at,
                updated_at",
        )
        .bind(user_id)
        .bind(rule_id)
        .bind(schedule.schedule_type.as_str())
        .bind(interval_seconds_hint(schedule.schedule_type))
        .bind(schedule.time_zone.as_str())
        .bind(i32::from(schedule.local_time_minutes))
        .bind(schedule.anchor_day_of_week.map(i16::from))
        .bind(schedule.anchor_day_of_month.map(i16::from))
        .bind(schedule.anchor_month.map(i16::from))
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
                local_time_minutes,
                anchor_day_of_week,
                anchor_day_of_month,
                anchor_month,
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
                    r.schedule_type,
                    r.local_time_minutes,
                    r.anchor_day_of_week,
                    r.anchor_day_of_month,
                    r.anchor_month,
                    r.time_zone,
                    r.next_run_at,
                    r.prompt_sha256,
                    pgp_sym_decrypt(r.prompt_ciphertext, $5) AS prompt_encoded
             )
             SELECT
                id,
                user_id,
                schedule_type,
                local_time_minutes,
                anchor_day_of_week,
                anchor_day_of_month,
                anchor_month,
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
        local_time_minutes: row.try_get("local_time_minutes")?,
        anchor_day_of_week: row.try_get("anchor_day_of_week")?,
        anchor_day_of_month: row.try_get("anchor_day_of_month")?,
        anchor_month: row.try_get("anchor_month")?,
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
    let prompt_ciphertext = decode_base64_payload(prompt_encoded.as_str())?;
    let schedule_type: String = row.try_get("schedule_type")?;

    Ok(ClaimedAutomationRule {
        id: row.try_get("id")?,
        user_id: row.try_get("user_id")?,
        schedule_type: AutomationScheduleType::from_db(&schedule_type)?,
        local_time_minutes: row.try_get("local_time_minutes")?,
        anchor_day_of_week: row.try_get("anchor_day_of_week")?,
        anchor_day_of_month: row.try_get("anchor_day_of_month")?,
        anchor_month: row.try_get("anchor_month")?,
        time_zone: row.try_get("time_zone")?,
        next_run_at: row.try_get("next_run_at")?,
        prompt_ciphertext,
        prompt_sha256: row.try_get("prompt_sha256")?,
    })
}

fn automation_prompt_material_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<AutomationPromptMaterial, StoreError> {
    let prompt_encoded: String = row.try_get("prompt_encoded")?;
    let prompt_ciphertext = decode_base64_payload(prompt_encoded.as_str())?;

    Ok(AutomationPromptMaterial {
        prompt_ciphertext,
        prompt_sha256: row.try_get("prompt_sha256")?,
    })
}

fn decode_base64_payload(encoded: &str) -> Result<Vec<u8>, StoreError> {
    let compact: String = encoded
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace())
        .collect();
    STANDARD
        .decode(compact.as_bytes())
        .map_err(|_| StoreError::InvalidData("automation prompt decode failed".to_string()))
}

fn normalized_schedule_spec(
    schedule: &AutomationScheduleSpec,
) -> Result<AutomationScheduleSpec, StoreError> {
    let mut normalized = schedule.clone();
    normalized.time_zone = normalized_time_zone(schedule.time_zone.as_str())?;
    validate_schedule_spec(&normalized).map_err(StoreError::InvalidData)?;
    Ok(normalized)
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
