use chrono::{DateTime, Duration, Utc};
use sqlx::Row;
use uuid::Uuid;

use super::{
    ClaimedDeleteRequest, PrivacyDeleteRequestStatus, PrivacyDeleteStatus, Store, StoreError,
};

impl Store {
    pub async fn queue_delete_all(&self, user_id: Uuid) -> Result<Uuid, StoreError> {
        self.ensure_user(user_id).await?;

        let existing_request_id = sqlx::query_scalar(
            "SELECT id
             FROM privacy_delete_requests
             WHERE user_id = $1
               AND status IN ('QUEUED', 'RUNNING')
             ORDER BY created_at ASC, id ASC
             LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(existing_request_id) = existing_request_id {
            return Ok(existing_request_id);
        }

        let request_id: Uuid = sqlx::query_scalar(
            "INSERT INTO privacy_delete_requests (user_id, status)
             VALUES ($1, 'QUEUED')
             RETURNING id",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(request_id)
    }

    pub async fn claim_delete_requests(
        &self,
        now: DateTime<Utc>,
        worker_id: Uuid,
        max_requests: i64,
        lease_seconds: i64,
    ) -> Result<Vec<ClaimedDeleteRequest>, StoreError> {
        if max_requests <= 0 {
            return Ok(Vec::new());
        }
        if lease_seconds <= 0 {
            return Err(StoreError::InvalidData(
                "privacy delete lease_seconds must be > 0".to_string(),
            ));
        }

        sqlx::query(
            "UPDATE privacy_delete_requests
             SET status = 'QUEUED',
                 lease_owner = NULL,
                 lease_expires_at = NULL,
                 updated_at = NOW()
             WHERE status = 'RUNNING'
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
                FROM privacy_delete_requests
                WHERE status = 'QUEUED'
                ORDER BY created_at ASC, id ASC
                LIMIT $1
                FOR UPDATE SKIP LOCKED
             ),
             claimed AS (
                UPDATE privacy_delete_requests p
                SET status = 'RUNNING',
                    started_at = COALESCE(p.started_at, $2),
                    failed_at = NULL,
                    failure_reason = NULL,
                    lease_owner = $3,
                    lease_expires_at = $4,
                    updated_at = NOW()
                FROM candidate_ids c
                WHERE p.id = c.id
                RETURNING p.id, p.user_id, p.created_at
             )
             SELECT id, user_id, created_at
             FROM claimed
             ORDER BY created_at ASC, id ASC",
        )
        .bind(max_requests)
        .bind(now)
        .bind(worker_id)
        .bind(lease_until)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(ClaimedDeleteRequest {
                    id: row.try_get("id")?,
                    user_id: row.try_get("user_id")?,
                    created_at: row.try_get("created_at")?,
                })
            })
            .collect()
    }

    pub async fn mark_delete_request_completed(
        &self,
        request_id: Uuid,
        worker_id: Uuid,
        completed_at: DateTime<Utc>,
    ) -> Result<bool, StoreError> {
        let result = sqlx::query(
            "UPDATE privacy_delete_requests
             SET status = 'COMPLETED',
                 completed_at = $3,
                 failed_at = NULL,
                 failure_reason = NULL,
                 lease_owner = NULL,
                 lease_expires_at = NULL,
                 updated_at = NOW()
             WHERE id = $1
               AND status = 'RUNNING'
               AND lease_owner = $2",
        )
        .bind(request_id)
        .bind(worker_id.to_string())
        .bind(completed_at)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn mark_delete_request_failed(
        &self,
        request_id: Uuid,
        worker_id: Uuid,
        failed_at: DateTime<Utc>,
        failure_reason: &str,
    ) -> Result<bool, StoreError> {
        let result = sqlx::query(
            "UPDATE privacy_delete_requests
             SET status = 'FAILED',
                 failed_at = $3,
                 failure_reason = $4,
                 lease_owner = NULL,
                 lease_expires_at = NULL,
                 updated_at = NOW()
             WHERE id = $1
               AND status = 'RUNNING'
               AND lease_owner = $2",
        )
        .bind(request_id)
        .bind(worker_id.to_string())
        .bind(failed_at)
        .bind(failure_reason)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn get_delete_request_status(
        &self,
        user_id: Uuid,
        request_id: Uuid,
    ) -> Result<Option<PrivacyDeleteRequestStatus>, StoreError> {
        let row = sqlx::query(
            "SELECT id, status, created_at, started_at, completed_at, failed_at
             FROM privacy_delete_requests
             WHERE user_id = $1
               AND id = $2",
        )
        .bind(user_id)
        .bind(request_id)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| {
            let status: String = row.try_get("status")?;
            Ok(PrivacyDeleteRequestStatus {
                id: row.try_get("id")?,
                status: PrivacyDeleteStatus::from_db(&status)?,
                created_at: row.try_get("created_at")?,
                started_at: row.try_get("started_at")?,
                completed_at: row.try_get("completed_at")?,
                failed_at: row.try_get("failed_at")?,
            })
        })
        .transpose()
    }

    pub async fn count_pending_delete_requests(&self) -> Result<i64, StoreError> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint
             FROM privacy_delete_requests
             WHERE status IN ('QUEUED', 'RUNNING')",
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(count)
    }

    pub async fn count_delete_requests_sla_overdue(
        &self,
        now: DateTime<Utc>,
        sla_hours: i64,
    ) -> Result<i64, StoreError> {
        if sla_hours <= 0 {
            return Err(StoreError::InvalidData(
                "privacy delete sla_hours must be > 0".to_string(),
            ));
        }

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint
             FROM privacy_delete_requests
             WHERE status <> 'COMPLETED'
               AND created_at <= ($1 - ($2 * INTERVAL '1 hour'))",
        )
        .bind(now)
        .bind(sla_hours)
        .fetch_one(&self.pool)
        .await?;

        Ok(count)
    }

    pub async fn purge_user_operational_data(&self, user_id: Uuid) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await?;

        sqlx::query("DELETE FROM audit_events WHERE user_id = $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM oauth_states WHERE user_id = $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM auth_sessions WHERE user_id = $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM connectors WHERE user_id = $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM devices WHERE user_id = $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM jobs WHERE user_id = $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM user_preferences WHERE user_id = $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "UPDATE users
             SET status = 'DELETED'
             WHERE id = $1",
        )
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }
}
