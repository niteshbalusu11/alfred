use chrono::{DateTime, Utc};
use sqlx::Row;
use uuid::Uuid;

use super::{AutomationRunRecord, AutomationRunState, Store, StoreError};

impl Store {
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

    pub async fn mark_automation_run_enqueued(
        &self,
        run_id: Uuid,
        user_id: Uuid,
        job_id: Uuid,
    ) -> Result<bool, StoreError> {
        let result = sqlx::query(
            "UPDATE automation_runs
             SET job_id = $3,
                 state = 'ENQUEUED',
                 updated_at = NOW()
             WHERE id = $1
               AND user_id = $2",
        )
        .bind(run_id)
        .bind(user_id)
        .bind(job_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn mark_automation_run_failed(
        &self,
        run_id: Uuid,
        user_id: Uuid,
    ) -> Result<bool, StoreError> {
        let result = sqlx::query(
            "UPDATE automation_runs
             SET state = 'FAILED',
                 updated_at = NOW()
             WHERE id = $1
               AND user_id = $2",
        )
        .bind(run_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
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
