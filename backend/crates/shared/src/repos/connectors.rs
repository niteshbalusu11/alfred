use sqlx::Row;
use uuid::Uuid;

use super::{
    ActiveConnectorMetadata, ConnectorKeyMetadata, LEGACY_CONNECTOR_TOKEN_KEY_ID, Store, StoreError,
};

impl Store {
    pub async fn list_active_connector_metadata(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<ActiveConnectorMetadata>, StoreError> {
        let rows = sqlx::query(
            "SELECT id, provider, token_key_id, token_version
             FROM connectors
             WHERE user_id = $1
               AND status = 'ACTIVE'
             ORDER BY created_at ASC, id ASC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                let connector_id: Uuid = row.try_get("id")?;
                let provider: String = row.try_get("provider")?;
                let token_key_id: String = row.try_get("token_key_id")?;
                let token_version: i32 = row.try_get("token_version")?;
                Ok(ActiveConnectorMetadata {
                    connector_id,
                    provider,
                    token_key_id,
                    token_version,
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

    pub(crate) async fn decrypt_active_connector_refresh_token(
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
}
