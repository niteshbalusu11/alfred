use base64::{
    Engine as _, engine::general_purpose::STANDARD, engine::general_purpose::URL_SAFE_NO_PAD,
};
use chrono::{DateTime, Duration, Utc};
use sha2::{Digest, Sha256};
use sqlx::Row;
use uuid::Uuid;

use super::{ClaimedJob, JobType, Store, StoreError};

impl Store {
    pub async fn enqueue_job(
        &self,
        user_id: Uuid,
        job_type: JobType,
        due_at: DateTime<Utc>,
        payload_ciphertext: Option<&[u8]>,
    ) -> Result<Uuid, StoreError> {
        let idempotency_key =
            default_job_idempotency_key(user_id, &job_type, due_at, payload_ciphertext);
        self.enqueue_job_with_idempotency_key(
            user_id,
            job_type,
            due_at,
            payload_ciphertext,
            &idempotency_key,
        )
        .await
    }

    pub async fn enqueue_job_with_idempotency_key(
        &self,
        user_id: Uuid,
        job_type: JobType,
        due_at: DateTime<Utc>,
        payload_ciphertext: Option<&[u8]>,
        idempotency_key: &str,
    ) -> Result<Uuid, StoreError> {
        self.ensure_user(user_id).await?;

        let job_id: Uuid = sqlx::query_scalar(
            "INSERT INTO jobs (user_id, type, due_at, state, payload_ciphertext, idempotency_key)
             VALUES (
               $1,
               $2,
               $3,
               'PENDING',
               CASE
                 WHEN $4::bytea IS NULL THEN NULL
                 ELSE pgp_sym_encrypt(encode($4, 'base64'), $6)
               END,
               $5
             )
             ON CONFLICT (user_id, type, idempotency_key)
             DO UPDATE SET
               due_at = LEAST(jobs.due_at, EXCLUDED.due_at),
               payload_ciphertext = COALESCE(EXCLUDED.payload_ciphertext, jobs.payload_ciphertext),
               updated_at = NOW()
             RETURNING id",
        )
        .bind(user_id)
        .bind(job_type.as_str())
        .bind(due_at)
        .bind(payload_ciphertext)
        .bind(idempotency_key)
        .bind(&self.data_encryption_key)
        .fetch_one(&self.pool)
        .await?;

        Ok(job_id)
    }

    pub async fn claim_due_jobs(
        &self,
        now: DateTime<Utc>,
        worker_id: Uuid,
        max_jobs: i64,
        lease_seconds: i64,
        per_user_concurrency_limit: i32,
    ) -> Result<Vec<ClaimedJob>, StoreError> {
        if max_jobs <= 0 {
            return Ok(Vec::new());
        }
        if lease_seconds <= 0 {
            return Err(StoreError::InvalidData(
                "lease_seconds must be > 0".to_string(),
            ));
        }
        if per_user_concurrency_limit <= 0 {
            return Err(StoreError::InvalidData(
                "per_user_concurrency_limit must be > 0".to_string(),
            ));
        }

        sqlx::query(
            "WITH expired AS (
                UPDATE jobs
                SET attempts = attempts + 1,
                    state = CASE
                      WHEN attempts + 1 >= max_attempts THEN 'FAILED'
                      ELSE 'PENDING'
                    END,
                    due_at = CASE
                      WHEN attempts + 1 >= max_attempts THEN due_at
                      ELSE $1
                    END,
                    next_run_at = CASE
                      WHEN attempts + 1 >= max_attempts THEN NULL
                      ELSE $1
                    END,
                    lease_owner = NULL,
                    lease_expires_at = NULL,
                    last_error_code = 'LEASE_EXPIRED',
                    last_error_message = 'lease expired before completion',
                    updated_at = NOW()
                WHERE state = 'RUNNING'
                  AND lease_expires_at IS NOT NULL
                  AND lease_expires_at <= $1
                RETURNING
                  id,
                  user_id,
                  type,
                  idempotency_key,
                  attempts,
                  payload_ciphertext,
                  state
             )
             INSERT INTO dead_letter_jobs (
               job_id,
               user_id,
               type,
               idempotency_key,
               attempts,
               reason_code,
               reason_message,
               payload_ciphertext
             )
             SELECT
               id,
               user_id,
               type,
               idempotency_key,
               attempts,
               'LEASE_EXPIRED_MAX_ATTEMPTS',
               'job lease expired and retry limit was reached',
               payload_ciphertext
             FROM expired
             WHERE state = 'FAILED'
             ON CONFLICT (job_id)
             DO UPDATE SET
               attempts = EXCLUDED.attempts,
               reason_code = EXCLUDED.reason_code,
               reason_message = EXCLUDED.reason_message,
               failed_at = NOW()",
        )
        .bind(now)
        .execute(&self.pool)
        .await?;

        let lease_until = now + Duration::seconds(lease_seconds);
        let worker_id = worker_id.to_string();

        let rows = sqlx::query(
            "WITH running_counts AS (
                SELECT user_id, COUNT(*)::int AS running_count
                FROM jobs
                WHERE state = 'RUNNING'
                  AND lease_expires_at IS NOT NULL
                  AND lease_expires_at > $1
                GROUP BY user_id
             ),
             eligible AS (
                SELECT
                  j.id,
                  j.user_id,
                  j.due_at,
                  COALESCE(r.running_count, 0) AS running_count,
                  ROW_NUMBER() OVER (
                    PARTITION BY j.user_id
                    ORDER BY j.due_at ASC, j.id ASC
                  ) AS user_rank
                FROM jobs j
                LEFT JOIN running_counts r ON r.user_id = j.user_id
                WHERE j.state = 'PENDING'
                  AND j.due_at <= $1
             ),
             candidate_ids AS (
                SELECT j.id
                FROM jobs j
                INNER JOIN eligible e ON e.id = j.id
                WHERE e.user_rank <= GREATEST($2 - e.running_count, 0)
                ORDER BY e.due_at ASC, j.id ASC
                LIMIT $3
                FOR UPDATE OF j SKIP LOCKED
             ),
             claimed AS (
                UPDATE jobs j
                SET state = 'RUNNING',
                    lease_owner = $4,
                    lease_expires_at = $5,
                    last_run_at = $1,
                    next_run_at = NULL,
                    updated_at = NOW()
                FROM candidate_ids c
                WHERE j.id = c.id
                RETURNING
                  j.id,
                  j.user_id,
                  j.type,
                  j.due_at,
                  CASE
                    WHEN j.payload_ciphertext IS NULL THEN NULL
                    ELSE pgp_sym_decrypt(j.payload_ciphertext, $6)
                  END AS payload_encoded,
                  j.attempts,
                  j.max_attempts,
                  j.idempotency_key
             )
             SELECT
               id,
               user_id,
               type,
               due_at,
               payload_encoded,
               attempts,
               max_attempts,
               idempotency_key
             FROM claimed
             ORDER BY due_at ASC, id ASC",
        )
        .bind(now)
        .bind(per_user_concurrency_limit)
        .bind(max_jobs)
        .bind(worker_id)
        .bind(lease_until)
        .bind(&self.data_encryption_key)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(claimed_job_from_row).collect()
    }

    pub async fn mark_job_done(&self, job_id: Uuid, worker_id: Uuid) -> Result<bool, StoreError> {
        let result = sqlx::query(
            "UPDATE jobs
             SET state = 'DONE',
                 lease_owner = NULL,
                 lease_expires_at = NULL,
                 next_run_at = NULL,
                 last_error_code = NULL,
                 last_error_message = NULL,
                 updated_at = NOW()
             WHERE id = $1
               AND state = 'RUNNING'
               AND lease_owner = $2",
        )
        .bind(job_id)
        .bind(worker_id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn schedule_job_retry(
        &self,
        job_id: Uuid,
        worker_id: Uuid,
        attempts: i32,
        next_due_at: DateTime<Utc>,
        error_code: &str,
        error_message: &str,
    ) -> Result<bool, StoreError> {
        let result = sqlx::query(
            "UPDATE jobs
             SET state = 'PENDING',
                 attempts = $3,
                 due_at = $4,
                 next_run_at = $4,
                 lease_owner = NULL,
                 lease_expires_at = NULL,
                 last_error_code = $5,
                 last_error_message = $6,
                 updated_at = NOW()
             WHERE id = $1
               AND state = 'RUNNING'
               AND lease_owner = $2",
        )
        .bind(job_id)
        .bind(worker_id.to_string())
        .bind(attempts)
        .bind(next_due_at)
        .bind(error_code)
        .bind(error_message)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn mark_job_failed(
        &self,
        job: &ClaimedJob,
        worker_id: Uuid,
        attempts: i32,
        reason_code: &str,
        reason_message: &str,
    ) -> Result<bool, StoreError> {
        let mut tx = self.pool.begin().await?;

        let result = sqlx::query(
            "UPDATE jobs
             SET state = 'FAILED',
                 attempts = $3,
                 lease_owner = NULL,
                 lease_expires_at = NULL,
                 next_run_at = NULL,
                 last_error_code = $4,
                 last_error_message = $5,
                 updated_at = NOW()
             WHERE id = $1
               AND state = 'RUNNING'
               AND lease_owner = $2",
        )
        .bind(job.id)
        .bind(worker_id.to_string())
        .bind(attempts)
        .bind(reason_code)
        .bind(reason_message)
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() == 0 {
            tx.rollback().await?;
            return Ok(false);
        }

        sqlx::query(
            "INSERT INTO dead_letter_jobs (
                job_id,
                user_id,
                type,
                idempotency_key,
                attempts,
                reason_code,
                reason_message,
                payload_ciphertext
             ) VALUES (
                $1,
                $2,
                $3,
                $4,
                $5,
                $6,
                $7,
                CASE
                  WHEN $8::bytea IS NULL THEN NULL
                  ELSE pgp_sym_encrypt(encode($8, 'base64'), $9)
                END
             )
             ON CONFLICT (job_id)
             DO UPDATE SET
               attempts = EXCLUDED.attempts,
               reason_code = EXCLUDED.reason_code,
               reason_message = EXCLUDED.reason_message,
               failed_at = NOW()",
        )
        .bind(job.id)
        .bind(job.user_id)
        .bind(job.job_type.as_str())
        .bind(&job.idempotency_key)
        .bind(attempts)
        .bind(reason_code)
        .bind(reason_message)
        .bind(job.payload_ciphertext.as_deref())
        .bind(&self.data_encryption_key)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(true)
    }

    pub async fn record_outbound_action_idempotency(
        &self,
        user_id: Uuid,
        action_key: &str,
        job_id: Uuid,
    ) -> Result<bool, StoreError> {
        let result = sqlx::query(
            "INSERT INTO outbound_action_idempotency (user_id, action_key, job_id)
             VALUES ($1, $2, $3)
             ON CONFLICT (user_id, action_key)
             DO NOTHING",
        )
        .bind(user_id)
        .bind(action_key)
        .bind(job_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn release_outbound_action_idempotency(
        &self,
        user_id: Uuid,
        action_key: &str,
        job_id: Uuid,
    ) -> Result<(), StoreError> {
        sqlx::query(
            "DELETE FROM outbound_action_idempotency
             WHERE user_id = $1
               AND action_key = $2
               AND job_id = $3",
        )
        .bind(user_id)
        .bind(action_key)
        .bind(job_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn count_due_jobs(&self, now: DateTime<Utc>) -> Result<i64, StoreError> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint
             FROM jobs
             WHERE state = 'PENDING' AND due_at <= $1",
        )
        .bind(now)
        .fetch_one(&self.pool)
        .await?;

        Ok(count)
    }
}

fn claimed_job_from_row(row: sqlx::postgres::PgRow) -> Result<ClaimedJob, StoreError> {
    let job_type: String = row.try_get("type")?;
    let payload_encoded: Option<String> = row.try_get("payload_encoded")?;
    let payload_ciphertext = payload_encoded
        .map(|encoded| decode_base64_payload(encoded.as_str()))
        .transpose()?;

    Ok(ClaimedJob {
        id: row.try_get("id")?,
        user_id: row.try_get("user_id")?,
        job_type: JobType::from_db(&job_type)?,
        due_at: row.try_get("due_at")?,
        payload_ciphertext,
        attempts: row.try_get("attempts")?,
        max_attempts: row.try_get("max_attempts")?,
        idempotency_key: row.try_get("idempotency_key")?,
    })
}

fn decode_base64_payload(encoded: &str) -> Result<Vec<u8>, StoreError> {
    let compact: String = encoded
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace())
        .collect();
    STANDARD
        .decode(compact.as_bytes())
        .map_err(|_| StoreError::InvalidData("job payload decode failed".to_string()))
}

fn default_job_idempotency_key(
    user_id: Uuid,
    job_type: &JobType,
    due_at: DateTime<Utc>,
    payload_ciphertext: Option<&[u8]>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(user_id.as_bytes());
    hasher.update([0x1f]);
    hasher.update(job_type.as_str().as_bytes());
    hasher.update([0x1f]);
    hasher.update(due_at.timestamp_micros().to_be_bytes());
    hasher.update([0x1f]);
    if let Some(payload) = payload_ciphertext {
        hasher.update(payload);
    }

    let digest = hasher.finalize();
    let suffix = URL_SAFE_NO_PAD.encode(digest);
    format!("{}:{suffix}", job_type.as_str())
}
