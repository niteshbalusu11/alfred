use reqwest::StatusCode;
use serde::Deserialize;
use shared::config::WorkerConfig;
use shared::repos::{LEGACY_CONNECTOR_TOKEN_KEY_ID, Store};
use shared::security::{ConnectorKeyMetadata, SecretRuntime};

use super::fetch::classified_http_error;
use crate::JobExecutionError;

pub(super) struct GoogleSession {
    pub(super) access_token: String,
    pub(super) attested_measurement: String,
}

pub(super) async fn build_google_session(
    store: &Store,
    config: &WorkerConfig,
    secret_runtime: &SecretRuntime,
    oauth_client: &reqwest::Client,
    user_id: uuid::Uuid,
) -> Result<GoogleSession, JobExecutionError> {
    let connector = load_active_google_connector(store, config, user_id).await?;

    let attested_identity = secret_runtime
        .authorize_connector_decrypt(&ConnectorKeyMetadata {
            key_id: connector.token_key_id.clone(),
            key_version: connector.token_version,
        })
        .map_err(|err| {
            JobExecutionError::permanent(
                "CONNECTOR_DECRYPT_NOT_AUTHORIZED",
                format!("decrypt authorization failed: {err}"),
            )
        })?;

    let refresh_token = store
        .decrypt_active_connector_refresh_token(
            user_id,
            connector.connector_id,
            &connector.token_key_id,
            connector.token_version,
        )
        .await
        .map_err(|err| {
            JobExecutionError::transient(
                "CONNECTOR_TOKEN_DECRYPT_FAILED",
                format!("failed to decrypt refresh token: {err}"),
            )
        })?
        .ok_or_else(|| {
            JobExecutionError::permanent(
                "CONNECTOR_TOKEN_MISSING",
                "refresh token was unavailable for active connector",
            )
        })?;

    let access_token = exchange_refresh_token(oauth_client, config, &refresh_token).await?;

    Ok(GoogleSession {
        access_token,
        attested_measurement: attested_identity.measurement,
    })
}

#[derive(Clone)]
struct ActiveGoogleConnector {
    connector_id: uuid::Uuid,
    token_key_id: String,
    token_version: i32,
}

async fn load_active_google_connector(
    store: &Store,
    config: &WorkerConfig,
    user_id: uuid::Uuid,
) -> Result<ActiveGoogleConnector, JobExecutionError> {
    let mut connector = store
        .list_active_connector_metadata(user_id)
        .await
        .map_err(|err| {
            JobExecutionError::transient(
                "CONNECTOR_METADATA_READ_FAILED",
                format!("failed to read connector metadata: {err}"),
            )
        })?
        .into_iter()
        .find(|item| item.provider == "google")
        .ok_or_else(|| {
            JobExecutionError::permanent(
                "GOOGLE_CONNECTOR_NOT_ACTIVE",
                "no active Google connector found for user",
            )
        })?;

    if connector.token_key_id == LEGACY_CONNECTOR_TOKEN_KEY_ID {
        store
            .adopt_legacy_connector_token_key_id(
                user_id,
                connector.connector_id,
                &config.kms_key_id,
                config.kms_key_version,
            )
            .await
            .map_err(|err| {
                JobExecutionError::transient(
                    "CONNECTOR_KEY_METADATA_UPDATE_FAILED",
                    format!("failed to adopt connector key metadata: {err}"),
                )
            })?;

        let refreshed = store
            .get_active_connector_key_metadata(user_id, connector.connector_id)
            .await
            .map_err(|err| {
                JobExecutionError::transient(
                    "CONNECTOR_KEY_METADATA_READ_FAILED",
                    format!("failed to read connector key metadata: {err}"),
                )
            })?
            .ok_or_else(|| {
                JobExecutionError::permanent(
                    "CONNECTOR_KEY_METADATA_MISSING",
                    "connector key metadata changed; retry the job",
                )
            })?;

        connector.token_key_id = refreshed.token_key_id;
        connector.token_version = refreshed.token_version;
    }

    Ok(ActiveGoogleConnector {
        connector_id: connector.connector_id,
        token_key_id: connector.token_key_id,
        token_version: connector.token_version,
    })
}

async fn exchange_refresh_token(
    oauth_client: &reqwest::Client,
    config: &WorkerConfig,
    refresh_token: &str,
) -> Result<String, JobExecutionError> {
    let response = oauth_client
        .post(&config.google_token_url)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", config.google_client_id.as_str()),
            ("client_secret", config.google_client_secret.as_str()),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await
        .map_err(|err| {
            JobExecutionError::transient(
                "GOOGLE_TOKEN_REFRESH_UNAVAILABLE",
                format!("google token refresh request failed: {err}"),
            )
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        let mut message = format!("google token refresh failed with HTTP {}", status.as_u16());
        if status == StatusCode::BAD_REQUEST
            && let Ok(parsed) = serde_json::from_str::<GoogleOAuthErrorResponse>(&body)
            && let Some(error) = parsed.error
        {
            message = format!("google token refresh rejected: {error}");
        }

        return Err(classified_http_error(
            status,
            "GOOGLE_TOKEN_REFRESH_FAILED",
            message,
        ));
    }

    let payload = response
        .json::<GoogleRefreshTokenResponse>()
        .await
        .map_err(|err| {
            JobExecutionError::transient(
                "GOOGLE_TOKEN_REFRESH_PARSE_FAILED",
                format!("google token refresh response was invalid: {err}"),
            )
        })?;

    Ok(payload.access_token)
}

#[derive(Debug, Deserialize)]
struct GoogleRefreshTokenResponse {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct GoogleOAuthErrorResponse {
    error: Option<String>,
}
