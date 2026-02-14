use std::collections::HashMap;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Duration, Utc};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use thiserror::Error;
use uuid::Uuid;

use crate::models::{ApnsEnvironment, AuditEvent, Preferences};

const DEFAULT_MEETING_REMINDER_MINUTES: i32 = 15;
const DEFAULT_MORNING_BRIEF_LOCAL_TIME: &str = "08:00";
const DEFAULT_QUIET_HOURS_START: &str = "22:00";
const DEFAULT_QUIET_HOURS_END: &str = "07:00";
pub const LEGACY_CONNECTOR_TOKEN_KEY_ID: &str = "__legacy__";

#[derive(Debug, Clone)]
pub enum AuditResult {
    Success,
    Failure,
}

impl AuditResult {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Success => "SUCCESS",
            Self::Failure => "FAILURE",
        }
    }
}

#[derive(Debug, Clone)]
pub enum JobType {
    MeetingReminder,
    MorningBrief,
    UrgentEmailCheck,
}

impl JobType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MeetingReminder => "MEETING_REMINDER",
            Self::MorningBrief => "MORNING_BRIEF",
            Self::UrgentEmailCheck => "URGENT_EMAIL_CHECK",
        }
    }

    fn from_db(value: &str) -> Result<Self, StoreError> {
        match value {
            "MEETING_REMINDER" => Ok(Self::MeetingReminder),
            "MORNING_BRIEF" => Ok(Self::MorningBrief),
            "URGENT_EMAIL_CHECK" => Ok(Self::UrgentEmailCheck),
            _ => Err(StoreError::InvalidData(format!(
                "unknown job type persisted: {value}"
            ))),
        }
    }
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("invalid cursor")]
    InvalidCursor,
    #[error("invalid persisted data: {0}")]
    InvalidData(String),
}

#[derive(Clone)]
pub struct Store {
    pool: PgPool,
    data_encryption_key: String,
}

#[derive(Debug, Clone)]
pub struct ConnectorKeyMetadata {
    pub provider: String,
    pub token_key_id: String,
    pub token_version: i32,
}

#[derive(Debug, Clone)]
pub struct ClaimedJob {
    pub id: Uuid,
    pub user_id: Uuid,
    pub job_type: JobType,
    pub due_at: DateTime<Utc>,
    pub payload_ciphertext: Option<Vec<u8>>,
    pub attempts: i32,
    pub max_attempts: i32,
    pub idempotency_key: String,
}

#[derive(Debug, Clone)]
pub struct DeviceRegistration {
    pub device_id: String,
    pub apns_token: String,
    pub environment: ApnsEnvironment,
}

impl Store {
    pub async fn connect(
        database_url: &str,
        max_connections: u32,
        data_encryption_key: &str,
    ) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .connect(database_url)
            .await?;

        Ok(Self {
            pool,
            data_encryption_key: data_encryption_key.to_string(),
        })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn ping(&self) -> Result<(), StoreError> {
        let _: i32 = sqlx::query_scalar("SELECT 1").fetch_one(&self.pool).await?;
        Ok(())
    }

    pub async fn create_user(&self) -> Result<Uuid, StoreError> {
        let user_id: Uuid = sqlx::query_scalar("INSERT INTO users DEFAULT VALUES RETURNING id")
            .fetch_one(&self.pool)
            .await?;
        Ok(user_id)
    }

    pub async fn ensure_user(&self, user_id: Uuid) -> Result<(), StoreError> {
        sqlx::query("INSERT INTO users (id) VALUES ($1) ON CONFLICT (id) DO NOTHING")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn create_session(
        &self,
        user_id: Uuid,
        access_token_hash: &[u8],
        refresh_token_hash: &[u8],
        expires_at: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.ensure_user(user_id).await?;

        sqlx::query(
            "INSERT INTO auth_sessions (user_id, access_token_hash, refresh_token_hash, expires_at)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(user_id)
        .bind(access_token_hash)
        .bind(refresh_token_hash)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn resolve_session_user(
        &self,
        access_token_hash: &[u8],
        now: DateTime<Utc>,
    ) -> Result<Option<Uuid>, StoreError> {
        let user_id = sqlx::query_scalar(
            "SELECT user_id
             FROM auth_sessions
             WHERE access_token_hash = $1
               AND revoked_at IS NULL
               AND expires_at > $2",
        )
        .bind(access_token_hash)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user_id)
    }

    pub async fn store_oauth_state(
        &self,
        user_id: Uuid,
        state_hash: &[u8],
        redirect_uri: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.ensure_user(user_id).await?;

        sqlx::query(
            "INSERT INTO oauth_states (user_id, state_hash, redirect_uri, expires_at)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (state_hash)
             DO UPDATE SET
               user_id = EXCLUDED.user_id,
               redirect_uri = EXCLUDED.redirect_uri,
               expires_at = EXCLUDED.expires_at,
               consumed_at = NULL",
        )
        .bind(user_id)
        .bind(state_hash)
        .bind(redirect_uri)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn consume_oauth_state(
        &self,
        user_id: Uuid,
        state_hash: &[u8],
        now: DateTime<Utc>,
    ) -> Result<Option<String>, StoreError> {
        let redirect_uri = sqlx::query_scalar(
            "UPDATE oauth_states
             SET consumed_at = NOW()
             WHERE user_id = $1
               AND state_hash = $2
               AND consumed_at IS NULL
               AND expires_at > $3
             RETURNING redirect_uri",
        )
        .bind(user_id)
        .bind(state_hash)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;

        Ok(redirect_uri)
    }

    pub async fn register_device(
        &self,
        user_id: Uuid,
        device_id: &str,
        apns_token: &str,
        environment: &ApnsEnvironment,
    ) -> Result<(), StoreError> {
        self.ensure_user(user_id).await?;

        sqlx::query(
            "INSERT INTO devices (user_id, device_identifier, apns_token_ciphertext, environment)
             VALUES ($1, $2, pgp_sym_encrypt($3, $5), $4)
             ON CONFLICT (user_id, device_identifier)
             DO UPDATE SET
               apns_token_ciphertext = pgp_sym_encrypt($3, $5),
               environment = EXCLUDED.environment,
               updated_at = NOW()",
        )
        .bind(user_id)
        .bind(device_id)
        .bind(apns_token)
        .bind(apns_environment_str(environment))
        .bind(&self.data_encryption_key)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn has_registered_device(&self, user_id: Uuid) -> Result<bool, StoreError> {
        self.ensure_user(user_id).await?;

        let has_device: bool = sqlx::query_scalar(
            "SELECT EXISTS (
                SELECT 1
                FROM devices
                WHERE user_id = $1
            )",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(has_device)
    }

    pub async fn list_registered_devices(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<DeviceRegistration>, StoreError> {
        self.ensure_user(user_id).await?;

        let rows = sqlx::query(
            "SELECT
                device_identifier,
                pgp_sym_decrypt(apns_token_ciphertext, $2) AS apns_token,
                environment
             FROM devices
             WHERE user_id = $1",
        )
        .bind(user_id)
        .bind(&self.data_encryption_key)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                let device_id: String = row.try_get("device_identifier")?;
                let apns_token: String = row.try_get("apns_token")?;
                let environment: String = row.try_get("environment")?;

                Ok(DeviceRegistration {
                    device_id,
                    apns_token,
                    environment: parse_apns_environment(&environment)?,
                })
            })
            .collect()
    }

    pub async fn upsert_google_connector(
        &self,
        user_id: Uuid,
        refresh_token: &str,
        scopes: &[String],
        token_key_id: &str,
        token_version: i32,
    ) -> Result<Uuid, StoreError> {
        self.ensure_user(user_id).await?;

        let connector_id: Uuid = sqlx::query_scalar(
            "INSERT INTO connectors (
                user_id,
                provider,
                scopes,
                refresh_token_ciphertext,
                token_key_id,
                token_version,
                token_rotated_at,
                status
             )
             VALUES ($1, 'google', $2, pgp_sym_encrypt($3, $6), $4, $5, NOW(), 'ACTIVE')
             ON CONFLICT (user_id, provider)
             DO UPDATE SET
               scopes = EXCLUDED.scopes,
               refresh_token_ciphertext = pgp_sym_encrypt($3, $6),
               token_key_id = EXCLUDED.token_key_id,
               token_version = EXCLUDED.token_version,
               token_rotated_at = CASE
                 WHEN connectors.token_key_id <> EXCLUDED.token_key_id
                   OR connectors.token_version <> EXCLUDED.token_version
                 THEN NOW()
                 ELSE connectors.token_rotated_at
               END,
               status = 'ACTIVE',
               revoked_at = NULL
             RETURNING id",
        )
        .bind(user_id)
        .bind(scopes)
        .bind(refresh_token)
        .bind(token_key_id)
        .bind(token_version)
        .bind(&self.data_encryption_key)
        .fetch_one(&self.pool)
        .await?;

        Ok(connector_id)
    }

    pub async fn revoke_connector(
        &self,
        user_id: Uuid,
        connector_id: Uuid,
    ) -> Result<bool, StoreError> {
        let result = sqlx::query(
            "UPDATE connectors
             SET status = 'REVOKED', revoked_at = NOW()
             WHERE id = $1 AND user_id = $2 AND status <> 'REVOKED'",
        )
        .bind(connector_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn get_active_connector_key_metadata(
        &self,
        user_id: Uuid,
        connector_id: Uuid,
    ) -> Result<Option<ConnectorKeyMetadata>, StoreError> {
        let row = sqlx::query(
            "SELECT provider, token_key_id, token_version
             FROM connectors
             WHERE id = $1
               AND user_id = $2
               AND status = 'ACTIVE'",
        )
        .bind(connector_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| {
            let provider: String = row.try_get("provider")?;
            let token_key_id: String = row.try_get("token_key_id")?;
            let token_version: i32 = row.try_get("token_version")?;
            Ok(ConnectorKeyMetadata {
                provider,
                token_key_id,
                token_version,
            })
        })
        .transpose()
    }

    pub async fn decrypt_active_connector_refresh_token(
        &self,
        user_id: Uuid,
        connector_id: Uuid,
        token_key_id: &str,
        token_version: i32,
    ) -> Result<Option<String>, StoreError> {
        let refresh_token = sqlx::query_scalar(
            "SELECT pgp_sym_decrypt(refresh_token_ciphertext, $5) AS refresh_token
             FROM connectors
             WHERE id = $1
               AND user_id = $2
               AND status = 'ACTIVE'
               AND token_key_id = $3
               AND token_version = $4",
        )
        .bind(connector_id)
        .bind(user_id)
        .bind(token_key_id)
        .bind(token_version)
        .bind(&self.data_encryption_key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(refresh_token)
    }

    pub async fn adopt_legacy_connector_token_key_id(
        &self,
        user_id: Uuid,
        connector_id: Uuid,
        token_key_id: &str,
        token_version: i32,
    ) -> Result<bool, StoreError> {
        let result = sqlx::query(
            "UPDATE connectors
             SET token_key_id = $3,
                 token_version = $4,
                 token_rotated_at = NOW()
             WHERE id = $1
               AND user_id = $2
               AND status = 'ACTIVE'
               AND token_key_id = $5",
        )
        .bind(connector_id)
        .bind(user_id)
        .bind(token_key_id)
        .bind(token_version)
        .bind(LEGACY_CONNECTOR_TOKEN_KEY_ID)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn get_or_create_preferences(
        &self,
        user_id: Uuid,
    ) -> Result<Preferences, StoreError> {
        self.ensure_user(user_id).await?;

        if let Some(row) = sqlx::query(
            "SELECT meeting_reminder_minutes, morning_brief_local_time, quiet_hours_start,
                    quiet_hours_end, high_risk_requires_confirm
             FROM user_preferences
             WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?
        {
            return row_to_preferences(&row);
        }

        sqlx::query(
            "INSERT INTO user_preferences (
                user_id,
                meeting_reminder_minutes,
                morning_brief_local_time,
                quiet_hours_start,
                quiet_hours_end,
                high_risk_requires_confirm
             ) VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(user_id)
        .bind(DEFAULT_MEETING_REMINDER_MINUTES)
        .bind(DEFAULT_MORNING_BRIEF_LOCAL_TIME)
        .bind(DEFAULT_QUIET_HOURS_START)
        .bind(DEFAULT_QUIET_HOURS_END)
        .bind(true)
        .execute(&self.pool)
        .await?;

        Ok(Preferences {
            meeting_reminder_minutes: DEFAULT_MEETING_REMINDER_MINUTES as u32,
            morning_brief_local_time: DEFAULT_MORNING_BRIEF_LOCAL_TIME.to_string(),
            quiet_hours_start: DEFAULT_QUIET_HOURS_START.to_string(),
            quiet_hours_end: DEFAULT_QUIET_HOURS_END.to_string(),
            high_risk_requires_confirm: true,
        })
    }

    pub async fn upsert_preferences(
        &self,
        user_id: Uuid,
        preferences: &Preferences,
    ) -> Result<(), StoreError> {
        self.ensure_user(user_id).await?;

        sqlx::query(
            "INSERT INTO user_preferences (
                user_id,
                meeting_reminder_minutes,
                morning_brief_local_time,
                quiet_hours_start,
                quiet_hours_end,
                high_risk_requires_confirm
             ) VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (user_id)
             DO UPDATE SET
               meeting_reminder_minutes = EXCLUDED.meeting_reminder_minutes,
               morning_brief_local_time = EXCLUDED.morning_brief_local_time,
               quiet_hours_start = EXCLUDED.quiet_hours_start,
               quiet_hours_end = EXCLUDED.quiet_hours_end,
               high_risk_requires_confirm = EXCLUDED.high_risk_requires_confirm,
               updated_at = NOW()",
        )
        .bind(user_id)
        .bind(preferences.meeting_reminder_minutes as i32)
        .bind(&preferences.morning_brief_local_time)
        .bind(&preferences.quiet_hours_start)
        .bind(&preferences.quiet_hours_end)
        .bind(preferences.high_risk_requires_confirm)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn add_audit_event(
        &self,
        user_id: Uuid,
        event_type: &str,
        connector: Option<&str>,
        result: AuditResult,
        metadata: &HashMap<String, String>,
    ) -> Result<(), StoreError> {
        self.ensure_user(user_id).await?;

        let redacted_metadata = Value::Object(
            metadata
                .iter()
                .map(|(key, value)| {
                    if is_sensitive_metadata_key(key) {
                        (key.clone(), Value::String("[REDACTED]".to_string()))
                    } else {
                        (key.clone(), Value::String(value.clone()))
                    }
                })
                .collect(),
        );

        sqlx::query(
            "INSERT INTO audit_events (user_id, event_type, connector, result, redacted_metadata)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(user_id)
        .bind(event_type)
        .bind(connector)
        .bind(result.as_str())
        .bind(redacted_metadata)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn list_audit_events(
        &self,
        user_id: Uuid,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<(Vec<AuditEvent>, Option<String>), StoreError> {
        let cursor = parse_cursor(cursor)?;

        let rows = sqlx::query(
            "SELECT id, created_at, event_type, connector, result, redacted_metadata
             FROM audit_events
             WHERE user_id = $1
               AND (
                 $2::timestamptz IS NULL
                 OR created_at < $2
                 OR (created_at = $2 AND id < $3)
               )
             ORDER BY created_at DESC, id DESC
             LIMIT $4",
        )
        .bind(user_id)
        .bind(cursor.as_ref().map(|(ts, _)| *ts))
        .bind(cursor.as_ref().map(|(_, id)| *id))
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        let mut items = Vec::with_capacity(rows.len());
        let mut last_key: Option<(DateTime<Utc>, Uuid)> = None;

        for row in rows {
            let id: Uuid = row.try_get("id")?;
            let created_at: DateTime<Utc> = row.try_get("created_at")?;
            let event_type: String = row.try_get("event_type")?;
            let connector: Option<String> = row.try_get("connector")?;
            let result: String = row.try_get("result")?;
            let metadata_value: Value = row.try_get("redacted_metadata")?;

            last_key = Some((created_at, id));

            items.push(AuditEvent {
                id: id.to_string(),
                timestamp: created_at,
                event_type,
                connector,
                result,
                metadata: json_value_to_string_map(metadata_value),
            });
        }

        let next_cursor = if items.len() == limit {
            last_key.map(|(ts, id)| encode_cursor(ts, id))
        } else {
            None
        };

        Ok((items, next_cursor))
    }

    pub async fn queue_delete_all(&self, user_id: Uuid) -> Result<Uuid, StoreError> {
        self.ensure_user(user_id).await?;

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
             VALUES ($1, $2, $3, 'PENDING', $4, $5)
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
                  j.payload_ciphertext,
                  j.attempts,
                  j.max_attempts,
                  j.idempotency_key
             )
             SELECT
               id,
               user_id,
               type,
               due_at,
               payload_ciphertext,
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
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
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
        .bind(&job.payload_ciphertext)
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
    Ok(ClaimedJob {
        id: row.try_get("id")?,
        user_id: row.try_get("user_id")?,
        job_type: JobType::from_db(&job_type)?,
        due_at: row.try_get("due_at")?,
        payload_ciphertext: row.try_get("payload_ciphertext")?,
        attempts: row.try_get("attempts")?,
        max_attempts: row.try_get("max_attempts")?,
        idempotency_key: row.try_get("idempotency_key")?,
    })
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

fn row_to_preferences(row: &sqlx::postgres::PgRow) -> Result<Preferences, StoreError> {
    let meeting_minutes: i32 = row.try_get("meeting_reminder_minutes")?;
    let meeting_minutes = u32::try_from(meeting_minutes).map_err(|_| {
        StoreError::InvalidData("meeting_reminder_minutes out of range".to_string())
    })?;

    Ok(Preferences {
        meeting_reminder_minutes: meeting_minutes,
        morning_brief_local_time: row.try_get("morning_brief_local_time")?,
        quiet_hours_start: row.try_get("quiet_hours_start")?,
        quiet_hours_end: row.try_get("quiet_hours_end")?,
        high_risk_requires_confirm: row.try_get("high_risk_requires_confirm")?,
    })
}

fn apns_environment_str(value: &ApnsEnvironment) -> &'static str {
    match value {
        ApnsEnvironment::Sandbox => "sandbox",
        ApnsEnvironment::Production => "production",
    }
}

fn parse_apns_environment(value: &str) -> Result<ApnsEnvironment, StoreError> {
    match value {
        "sandbox" => Ok(ApnsEnvironment::Sandbox),
        "production" => Ok(ApnsEnvironment::Production),
        _ => Err(StoreError::InvalidData(format!(
            "unknown apns environment persisted: {value}"
        ))),
    }
}

fn parse_cursor(cursor: Option<&str>) -> Result<Option<(DateTime<Utc>, Uuid)>, StoreError> {
    let Some(cursor) = cursor else {
        return Ok(None);
    };

    let (timestamp_micros, id) = cursor.split_once('|').ok_or(StoreError::InvalidCursor)?;
    let timestamp_micros = timestamp_micros
        .parse::<i64>()
        .map_err(|_| StoreError::InvalidCursor)?;
    let timestamp =
        DateTime::from_timestamp_micros(timestamp_micros).ok_or(StoreError::InvalidCursor)?;
    let id = Uuid::parse_str(id).map_err(|_| StoreError::InvalidCursor)?;

    Ok(Some((timestamp, id)))
}

fn encode_cursor(timestamp: DateTime<Utc>, id: Uuid) -> String {
    format!("{}|{}", timestamp.timestamp_micros(), id)
}

fn json_value_to_string_map(value: Value) -> HashMap<String, String> {
    match value {
        Value::Object(map) => map
            .into_iter()
            .map(|(key, value)| {
                let stringified = match value {
                    Value::String(string) => string,
                    other => other.to_string(),
                };
                (key, stringified)
            })
            .collect(),
        _ => HashMap::new(),
    }
}

fn is_sensitive_metadata_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.contains("token")
        || key.contains("secret")
        || key.contains("password")
        || key.contains("authorization")
        || key.contains("code")
}
