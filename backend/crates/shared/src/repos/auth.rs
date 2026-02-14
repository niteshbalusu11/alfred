use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::{Store, StoreError};

impl Store {
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
}
