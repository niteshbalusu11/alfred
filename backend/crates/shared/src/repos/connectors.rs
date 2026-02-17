use sqlx::Row;
use uuid::Uuid;

use super::{
    ActiveConnectorMetadata, ConnectorKeyMetadata, ConnectorStateRecord,
    LEGACY_CONNECTOR_TOKEN_KEY_ID, Store, StoreError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectorKeyRotationOutcome {
    Rotated,
    AlreadyCurrent,
    Missing,
    RetryRequired,
}

fn classify_connector_key_rotation_outcome(
    rows_affected: u64,
    refreshed: Option<&ConnectorKeyMetadata>,
    target_key_id: &str,
    target_version: i32,
) -> ConnectorKeyRotationOutcome {
    if rows_affected > 0 {
        return ConnectorKeyRotationOutcome::Rotated;
    }

    let Some(refreshed) = refreshed else {
        return ConnectorKeyRotationOutcome::Missing;
    };

    if refreshed.token_key_id == target_key_id && refreshed.token_version == target_version {
        return ConnectorKeyRotationOutcome::AlreadyCurrent;
    }

    ConnectorKeyRotationOutcome::RetryRequired
}

impl Store {
    pub async fn list_connector_states(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<ConnectorStateRecord>, StoreError> {
        let rows = sqlx::query(
            "SELECT id, provider, status
             FROM connectors
             WHERE user_id = $1
             ORDER BY created_at ASC, id ASC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                let connector_id: Uuid = row.try_get("id")?;
                let provider: String = row.try_get("provider")?;
                let status: String = row.try_get("status")?;
                Ok(ConnectorStateRecord {
                    connector_id,
                    provider,
                    status,
                })
            })
            .collect()
    }

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

    pub async fn ensure_active_connector_key_metadata(
        &self,
        user_id: Uuid,
        connector_id: Uuid,
        target_key_id: &str,
        target_version: i32,
    ) -> Result<Option<ConnectorKeyMetadata>, StoreError> {
        let Some(current) = self
            .get_active_connector_key_metadata(user_id, connector_id)
            .await?
        else {
            return Ok(None);
        };

        if current.token_key_id == target_key_id && current.token_version == target_version {
            return Ok(Some(current));
        }

        let rows_affected = sqlx::query(
            "UPDATE connectors
             SET token_key_id = $3,
                 token_version = $4,
                 token_rotated_at = NOW()
             WHERE id = $1
               AND user_id = $2
               AND status = 'ACTIVE'
               AND token_key_id = $5
               AND token_version = $6",
        )
        .bind(connector_id)
        .bind(user_id)
        .bind(target_key_id)
        .bind(target_version)
        .bind(&current.token_key_id)
        .bind(current.token_version)
        .execute(&self.pool)
        .await?
        .rows_affected();

        let refreshed = self
            .get_active_connector_key_metadata(user_id, connector_id)
            .await?;

        match classify_connector_key_rotation_outcome(
            rows_affected,
            refreshed.as_ref(),
            target_key_id,
            target_version,
        ) {
            ConnectorKeyRotationOutcome::Rotated | ConnectorKeyRotationOutcome::AlreadyCurrent => {
                Ok(refreshed)
            }
            ConnectorKeyRotationOutcome::Missing => Ok(None),
            ConnectorKeyRotationOutcome::RetryRequired => Err(StoreError::InvalidData(format!(
                "connector key metadata rotation conflict for connector_id={connector_id}: expected target key_id={target_key_id}, version={target_version}"
            ))),
        }
    }

    pub(crate) async fn decrypt_active_connector_refresh_token(
        &self,
        user_id: Uuid,
        connector_id: Uuid,
        metadata: &ConnectorKeyMetadata,
    ) -> Result<Option<String>, StoreError> {
        let refresh_token = sqlx::query_scalar(
            "SELECT pgp_sym_decrypt(refresh_token_ciphertext, $6) AS refresh_token
             FROM connectors
             WHERE id = $1
               AND user_id = $2
               AND provider = $3
               AND status = 'ACTIVE'
               AND token_key_id = $4
               AND token_version = $5",
        )
        .bind(connector_id)
        .bind(user_id)
        .bind(&metadata.provider)
        .bind(&metadata.token_key_id)
        .bind(metadata.token_version)
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

#[cfg(test)]
mod tests {
    use super::{ConnectorKeyRotationOutcome, classify_connector_key_rotation_outcome};
    use crate::repos::ConnectorKeyMetadata;

    fn metadata(token_key_id: &str, token_version: i32) -> ConnectorKeyMetadata {
        ConnectorKeyMetadata {
            provider: "google".to_string(),
            token_key_id: token_key_id.to_string(),
            token_version,
        }
    }

    #[test]
    fn classify_rotation_success_when_update_applied() {
        let outcome = classify_connector_key_rotation_outcome(
            1,
            Some(&metadata("kms/alfred/token", 2)),
            "kms/alfred/token",
            2,
        );
        assert_eq!(outcome, ConnectorKeyRotationOutcome::Rotated);
    }

    #[test]
    fn classify_rotation_retry_safe_when_concurrent_writer_already_rotated() {
        let outcome = classify_connector_key_rotation_outcome(
            0,
            Some(&metadata("kms/alfred/token", 2)),
            "kms/alfred/token",
            2,
        );
        assert_eq!(outcome, ConnectorKeyRotationOutcome::AlreadyCurrent);
    }

    #[test]
    fn classify_rotation_partial_failure_requires_retry() {
        let outcome = classify_connector_key_rotation_outcome(
            0,
            Some(&metadata("__legacy__", 1)),
            "kms/alfred/token",
            2,
        );
        assert_eq!(outcome, ConnectorKeyRotationOutcome::RetryRequired);
    }

    #[test]
    fn classify_rotation_missing_row_is_deterministic_noop() {
        let outcome = classify_connector_key_rotation_outcome(0, None, "kms/alfred/token", 2);
        assert_eq!(outcome, ConnectorKeyRotationOutcome::Missing);
    }
}
