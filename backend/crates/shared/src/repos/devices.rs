use sqlx::Row;
use uuid::Uuid;

use crate::models::ApnsEnvironment;

use super::{DeviceRegistration, Store, StoreError};

impl Store {
    pub async fn register_device(
        &self,
        user_id: Uuid,
        device_id: &str,
        apns_token: &str,
        environment: &ApnsEnvironment,
        notification_key_algorithm: Option<&str>,
        notification_public_key: Option<&str>,
    ) -> Result<(), StoreError> {
        self.ensure_user(user_id).await?;

        sqlx::query(
            "INSERT INTO devices (
                user_id,
                device_identifier,
                apns_token_ciphertext,
                environment,
                notification_key_algorithm,
                notification_public_key_ciphertext
             )
             VALUES ($1, $2, pgp_sym_encrypt($3, $7), $4, $5, pgp_sym_encrypt($6, $7))
             ON CONFLICT (user_id, device_identifier)
             DO UPDATE SET
               apns_token_ciphertext = pgp_sym_encrypt($3, $7),
               environment = EXCLUDED.environment,
               notification_key_algorithm = EXCLUDED.notification_key_algorithm,
               notification_public_key_ciphertext = EXCLUDED.notification_public_key_ciphertext,
               updated_at = NOW()",
        )
        .bind(user_id)
        .bind(device_id)
        .bind(apns_token)
        .bind(apns_environment_str(environment))
        .bind(notification_key_algorithm)
        .bind(notification_public_key)
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
                environment,
                notification_key_algorithm,
                pgp_sym_decrypt(notification_public_key_ciphertext, $2) AS notification_public_key
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
                let notification_key_algorithm: Option<String> =
                    row.try_get("notification_key_algorithm")?;
                let notification_public_key: Option<String> =
                    row.try_get("notification_public_key")?;

                Ok(DeviceRegistration {
                    device_id,
                    apns_token,
                    environment: parse_apns_environment(&environment)?,
                    notification_key_algorithm,
                    notification_public_key,
                })
            })
            .collect()
    }
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
