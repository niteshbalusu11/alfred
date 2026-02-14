use reqwest::StatusCode;
use serde::Deserialize;
use shared::config::WorkerConfig;
use shared::repos::{ActiveConnectorMetadata, LEGACY_CONNECTOR_TOKEN_KEY_ID, Store};
use shared::security::{ConnectorKeyMetadata, SecretRuntime};
use tracing::info;
use uuid::Uuid;

#[derive(Debug)]
pub(crate) struct DeleteRequestError {
    pub code: &'static str,
    pub message: String,
}

impl DeleteRequestError {
    pub(crate) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

pub(crate) async fn revoke_active_connectors(
    store: &Store,
    config: &WorkerConfig,
    secret_runtime: &SecretRuntime,
    oauth_client: &reqwest::Client,
    user_id: Uuid,
    connectors: Vec<ActiveConnectorMetadata>,
) -> Result<usize, DeleteRequestError> {
    let mut revoked_count = 0_usize;

    for connector in connectors {
        revoke_single_connector(
            store,
            config,
            secret_runtime,
            oauth_client,
            user_id,
            connector,
        )
        .await?;
        revoked_count += 1;
    }

    Ok(revoked_count)
}

async fn revoke_single_connector(
    store: &Store,
    config: &WorkerConfig,
    secret_runtime: &SecretRuntime,
    oauth_client: &reqwest::Client,
    user_id: Uuid,
    connector: ActiveConnectorMetadata,
) -> Result<(), DeleteRequestError> {
    if connector.provider != "google" {
        return Err(DeleteRequestError::new(
            "UNSUPPORTED_CONNECTOR_PROVIDER",
            format!("unsupported connector provider: {}", connector.provider),
        ));
    }

    let connector = normalize_connector_metadata(store, config, user_id, connector).await?;

    let attested_identity = secret_runtime
        .authorize_connector_decrypt(&ConnectorKeyMetadata {
            key_id: connector.token_key_id.clone(),
            key_version: connector.token_version,
        })
        .map_err(|err| {
            DeleteRequestError::new(
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
            DeleteRequestError::new(
                "CONNECTOR_TOKEN_DECRYPT_FAILED",
                format!("failed to decrypt refresh token: {err}"),
            )
        })?
        .ok_or_else(|| {
            DeleteRequestError::new(
                "CONNECTOR_TOKEN_MISSING",
                "refresh token was unavailable for active connector",
            )
        })?;

    revoke_google_token(oauth_client, &config.google_revoke_url, &refresh_token).await?;

    info!(
        user_id = %user_id,
        connector_id = %connector.connector_id,
        attested_measurement = %attested_identity.measurement,
        "revoked connector token for privacy delete request"
    );

    Ok(())
}

async fn normalize_connector_metadata(
    store: &Store,
    config: &WorkerConfig,
    user_id: Uuid,
    mut connector: ActiveConnectorMetadata,
) -> Result<ActiveConnectorMetadata, DeleteRequestError> {
    if connector.token_key_id != LEGACY_CONNECTOR_TOKEN_KEY_ID {
        return Ok(connector);
    }

    store
        .adopt_legacy_connector_token_key_id(
            user_id,
            connector.connector_id,
            &config.kms_key_id,
            config.kms_key_version,
        )
        .await
        .map_err(|err| {
            DeleteRequestError::new(
                "CONNECTOR_KEY_METADATA_UPDATE_FAILED",
                format!("failed to adopt connector key metadata: {err}"),
            )
        })?;

    let refreshed = store
        .get_active_connector_key_metadata(user_id, connector.connector_id)
        .await
        .map_err(|err| {
            DeleteRequestError::new(
                "CONNECTOR_KEY_METADATA_READ_FAILED",
                format!("failed to read connector key metadata: {err}"),
            )
        })?
        .ok_or_else(|| {
            DeleteRequestError::new(
                "CONNECTOR_KEY_METADATA_MISSING",
                "connector key metadata changed during delete workflow",
            )
        })?;

    connector.token_key_id = refreshed.token_key_id;
    connector.token_version = refreshed.token_version;

    Ok(connector)
}

async fn revoke_google_token(
    client: &reqwest::Client,
    revoke_url: &str,
    refresh_token: &str,
) -> Result<(), DeleteRequestError> {
    let response = client
        .post(revoke_url)
        .form(&[("token", refresh_token)])
        .send()
        .await
        .map_err(|err| {
            DeleteRequestError::new(
                "GOOGLE_REVOKE_UNAVAILABLE",
                format!("failed to call Google revoke endpoint: {err}"),
            )
        })?;

    if response.status().is_success() {
        return Ok(());
    }

    let status = response.status();
    let body = response.text().await.unwrap_or_default();

    if status == StatusCode::BAD_REQUEST
        && let Some(parsed) = parse_google_oauth_error(&body)
        && parsed.error == "invalid_token"
    {
        return Ok(());
    }

    Err(DeleteRequestError::new(
        "GOOGLE_REVOKE_FAILED",
        format!("Google revoke endpoint returned HTTP {}", status.as_u16()),
    ))
}

#[derive(Debug, Deserialize)]
struct GoogleOAuthErrorResponse {
    error: String,
}

fn parse_google_oauth_error(body: &str) -> Option<GoogleOAuthErrorResponse> {
    serde_json::from_str::<GoogleOAuthErrorResponse>(body).ok()
}
