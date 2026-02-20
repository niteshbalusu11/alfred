use chrono::{DateTime, Duration, Utc};
use sqlx::Row;
use uuid::Uuid;

use crate::models::AssistantSessionStateEnvelope;

use super::{Store, StoreError};

#[derive(Debug, Clone)]
pub struct AssistantEncryptedSessionRecord {
    pub session_id: Uuid,
    pub state: AssistantSessionStateEnvelope,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct AssistantEncryptedSessionMetadataRecord {
    pub session_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl Store {
    pub async fn list_assistant_encrypted_sessions(
        &self,
        user_id: Uuid,
        now: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<AssistantEncryptedSessionMetadataRecord>, StoreError> {
        if limit <= 0 {
            return Err(StoreError::InvalidData(
                "assistant encrypted session list limit must be > 0".to_string(),
            ));
        }

        self.purge_expired_assistant_encrypted_sessions(user_id, now)
            .await?;

        let rows = sqlx::query(
            "SELECT session_id, created_at, updated_at, expires_at
             FROM assistant_encrypted_sessions
             WHERE user_id = $1
               AND expires_at > $2
             ORDER BY updated_at DESC, session_id DESC
             LIMIT $3",
        )
        .bind(user_id)
        .bind(now)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(AssistantEncryptedSessionMetadataRecord {
                    session_id: row.try_get("session_id")?,
                    created_at: row.try_get("created_at")?,
                    updated_at: row.try_get("updated_at")?,
                    expires_at: row.try_get("expires_at")?,
                })
            })
            .collect()
    }

    pub async fn load_assistant_encrypted_session(
        &self,
        user_id: Uuid,
        session_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<Option<AssistantEncryptedSessionRecord>, StoreError> {
        self.purge_expired_assistant_encrypted_sessions(user_id, now)
            .await?;

        let row = sqlx::query(
            "SELECT session_id, expires_at, state_json
             FROM assistant_encrypted_sessions
             WHERE user_id = $1
               AND session_id = $2
               AND expires_at > $3",
        )
        .bind(user_id)
        .bind(session_id)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| {
            let state_json: String = row.try_get("state_json")?;
            let state = serde_json::from_str::<AssistantSessionStateEnvelope>(&state_json)
                .map_err(|err| {
                    StoreError::InvalidData(format!("assistant encrypted session invalid: {err}"))
                })?;

            Ok(AssistantEncryptedSessionRecord {
                session_id: row.try_get("session_id")?,
                state,
                expires_at: row.try_get("expires_at")?,
            })
        })
        .transpose()
    }

    pub async fn upsert_assistant_encrypted_session(
        &self,
        user_id: Uuid,
        session_id: Uuid,
        state: &AssistantSessionStateEnvelope,
        now: DateTime<Utc>,
        ttl_seconds: i64,
    ) -> Result<(), StoreError> {
        if ttl_seconds <= 0 {
            return Err(StoreError::InvalidData(
                "assistant encrypted session ttl_seconds must be > 0".to_string(),
            ));
        }

        self.ensure_user(user_id).await?;
        self.purge_expired_assistant_encrypted_sessions(user_id, now)
            .await?;

        let state_json = serde_json::to_string(state).map_err(|err| {
            StoreError::InvalidData(format!("assistant encrypted session invalid: {err}"))
        })?;
        let expires_at = now + Duration::seconds(ttl_seconds);

        sqlx::query(
            "INSERT INTO assistant_encrypted_sessions (
                user_id,
                session_id,
                state_json,
                created_at,
                updated_at,
                expires_at
             ) VALUES ($1, $2, $3, $4, $4, $5)
             ON CONFLICT (user_id, session_id)
             DO UPDATE SET
               state_json = EXCLUDED.state_json,
               updated_at = $4,
               expires_at = $5",
        )
        .bind(user_id)
        .bind(session_id)
        .bind(state_json)
        .bind(now)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn delete_assistant_encrypted_session(
        &self,
        user_id: Uuid,
        session_id: Uuid,
    ) -> Result<bool, StoreError> {
        let result = sqlx::query(
            "DELETE FROM assistant_encrypted_sessions
             WHERE user_id = $1
               AND session_id = $2",
        )
        .bind(user_id)
        .bind(session_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_all_assistant_encrypted_sessions(
        &self,
        user_id: Uuid,
    ) -> Result<u64, StoreError> {
        let result = sqlx::query(
            "DELETE FROM assistant_encrypted_sessions
             WHERE user_id = $1",
        )
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    async fn purge_expired_assistant_encrypted_sessions(
        &self,
        user_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        sqlx::query(
            "DELETE FROM assistant_encrypted_sessions
             WHERE user_id = $1
               AND expires_at <= $2",
        )
        .bind(user_id)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
