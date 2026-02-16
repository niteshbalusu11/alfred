use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::models::AssistantQueryCapability;

use super::{Store, StoreError};

pub const ASSISTANT_SESSION_MEMORY_VERSION_V1: &str = "2026-02-16";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssistantSessionMemory {
    pub version: String,
    pub turns: Vec<AssistantSessionTurn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssistantSessionTurn {
    pub user_query_snippet: String,
    pub assistant_summary_snippet: String,
    pub capability: AssistantQueryCapability,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct AssistantSessionRecord {
    pub session_id: Uuid,
    pub last_capability: AssistantQueryCapability,
    pub turn_count: usize,
    pub memory: AssistantSessionMemory,
    pub expires_at: DateTime<Utc>,
}

impl Store {
    pub async fn load_assistant_session(
        &self,
        user_id: Uuid,
        session_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<Option<AssistantSessionRecord>, StoreError> {
        self.purge_expired_assistant_sessions(user_id, now).await?;

        let row = sqlx::query(
            "SELECT session_id, last_capability, turn_count, expires_at,
                    pgp_sym_decrypt(memory_ciphertext, $4)::text AS memory_json
             FROM assistant_sessions
             WHERE user_id = $1
               AND session_id = $2
               AND expires_at > $3",
        )
        .bind(user_id)
        .bind(session_id)
        .bind(now)
        .bind(&self.data_encryption_key)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| {
            let memory_json: String = row.try_get("memory_json")?;
            let memory =
                serde_json::from_str::<AssistantSessionMemory>(&memory_json).map_err(|err| {
                    StoreError::InvalidData(format!("assistant memory invalid: {err}"))
                })?;
            let turn_count_raw: i32 = row.try_get("turn_count")?;
            let turn_count = usize::try_from(turn_count_raw).map_err(|_| {
                StoreError::InvalidData("assistant session turn_count out of range".to_string())
            })?;
            let last_capability_raw: String = row.try_get("last_capability")?;

            Ok(AssistantSessionRecord {
                session_id: row.try_get("session_id")?,
                last_capability: capability_from_db(&last_capability_raw)?,
                turn_count,
                memory,
                expires_at: row.try_get("expires_at")?,
            })
        })
        .transpose()
    }

    pub async fn upsert_assistant_session(
        &self,
        user_id: Uuid,
        session_id: Uuid,
        last_capability: AssistantQueryCapability,
        memory: &AssistantSessionMemory,
        now: DateTime<Utc>,
        ttl_seconds: i64,
    ) -> Result<(), StoreError> {
        if ttl_seconds <= 0 {
            return Err(StoreError::InvalidData(
                "assistant session ttl_seconds must be > 0".to_string(),
            ));
        }

        self.ensure_user(user_id).await?;
        self.purge_expired_assistant_sessions(user_id, now).await?;

        let memory_json = serde_json::to_string(memory)
            .map_err(|err| StoreError::InvalidData(format!("assistant memory invalid: {err}")))?;
        let turn_count = i32::try_from(memory.turns.len()).map_err(|_| {
            StoreError::InvalidData("assistant session turn_count exceeds i32".to_string())
        })?;
        let expires_at = now + Duration::seconds(ttl_seconds);

        sqlx::query(
            "INSERT INTO assistant_sessions (
                user_id,
                session_id,
                last_capability,
                turn_count,
                memory_ciphertext,
                created_at,
                updated_at,
                expires_at
             ) VALUES ($1, $2, $3, $4, pgp_sym_encrypt($5, $8), $6, $6, $7)
             ON CONFLICT (user_id, session_id)
             DO UPDATE SET
               last_capability = EXCLUDED.last_capability,
               turn_count = EXCLUDED.turn_count,
               memory_ciphertext = pgp_sym_encrypt($5, $8),
               updated_at = $6,
               expires_at = $7",
        )
        .bind(user_id)
        .bind(session_id)
        .bind(capability_to_db(last_capability))
        .bind(turn_count)
        .bind(memory_json)
        .bind(now)
        .bind(expires_at)
        .bind(&self.data_encryption_key)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn purge_expired_assistant_sessions(
        &self,
        user_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        sqlx::query(
            "DELETE FROM assistant_sessions
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

fn capability_to_db(value: AssistantQueryCapability) -> &'static str {
    match value {
        AssistantQueryCapability::MeetingsToday => "MEETINGS_TODAY",
    }
}

fn capability_from_db(value: &str) -> Result<AssistantQueryCapability, StoreError> {
    match value {
        "MEETINGS_TODAY" => Ok(AssistantQueryCapability::MeetingsToday),
        _ => Err(StoreError::InvalidData(format!(
            "unknown assistant capability persisted: {value}"
        ))),
    }
}
