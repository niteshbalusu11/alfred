use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use thiserror::Error;
use uuid::Uuid;

use crate::models::{ApnsEnvironment, AuditEvent, Preferences};

const DEFAULT_MEETING_REMINDER_MINUTES: i32 = 15;
const DEFAULT_MORNING_BRIEF_LOCAL_TIME: &str = "08:00";
const DEFAULT_QUIET_HOURS_START: &str = "22:00";
const DEFAULT_QUIET_HOURS_END: &str = "07:00";

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
    fn as_str(&self) -> &'static str {
        match self {
            Self::MeetingReminder => "MEETING_REMINDER",
            Self::MorningBrief => "MORNING_BRIEF",
            Self::UrgentEmailCheck => "URGENT_EMAIL_CHECK",
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
}

impl Store {
    pub async fn connect(database_url: &str, max_connections: u32) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .connect(database_url)
            .await?;

        Ok(Self { pool })
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
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (user_id, device_identifier)
             DO UPDATE SET
               apns_token_ciphertext = EXCLUDED.apns_token_ciphertext,
               environment = EXCLUDED.environment,
               updated_at = NOW()",
        )
        .bind(user_id)
        .bind(device_id)
        .bind(apns_token.as_bytes())
        .bind(apns_environment_str(environment))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn upsert_google_connector(
        &self,
        user_id: Uuid,
        token_ciphertext: &[u8],
        scopes: &[String],
    ) -> Result<Uuid, StoreError> {
        self.ensure_user(user_id).await?;

        let connector_id: Uuid = sqlx::query_scalar(
            "INSERT INTO connectors (user_id, provider, scopes, refresh_token_ciphertext, status)
             VALUES ($1, 'google', $2, $3, 'ACTIVE')
             ON CONFLICT (user_id, provider)
             DO UPDATE SET
               scopes = EXCLUDED.scopes,
               refresh_token_ciphertext = EXCLUDED.refresh_token_ciphertext,
               status = 'ACTIVE',
               revoked_at = NULL
             RETURNING id",
        )
        .bind(user_id)
        .bind(scopes)
        .bind(token_ciphertext)
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
                .map(|(k, v)| (k.clone(), Value::String(v.clone())))
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
        self.ensure_user(user_id).await?;

        let job_id: Uuid = sqlx::query_scalar(
            "INSERT INTO jobs (user_id, type, due_at, state, payload_ciphertext)
             VALUES ($1, $2, $3, 'PENDING', $4)
             RETURNING id",
        )
        .bind(user_id)
        .bind(job_type.as_str())
        .bind(due_at)
        .bind(payload_ciphertext)
        .fetch_one(&self.pool)
        .await?;

        Ok(job_id)
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
