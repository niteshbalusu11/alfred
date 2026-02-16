use shared::config::WorkerConfig;
use shared::enclave::{
    ConnectorSecretRequest, EnclaveRpcClient, EnclaveRpcError, GoogleEnclaveOauthConfig,
};
use shared::repos::{LEGACY_CONNECTOR_TOKEN_KEY_ID, Store};
use shared::security::SecretRuntime;

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
    let enclave_client = build_enclave_client(store, config, secret_runtime, oauth_client);
    let token_response = enclave_client
        .exchange_google_access_token(ConnectorSecretRequest {
            user_id,
            connector_id: connector.connector_id,
        })
        .await
        .map_err(map_exchange_enclave_error)?;

    Ok(GoogleSession {
        access_token: token_response.access_token,
        attested_measurement: token_response.attested_identity.measurement,
    })
}

#[derive(Clone)]
struct ActiveGoogleConnector {
    connector_id: uuid::Uuid,
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
    })
}

fn build_enclave_client(
    store: &Store,
    config: &WorkerConfig,
    secret_runtime: &SecretRuntime,
    oauth_client: &reqwest::Client,
) -> EnclaveRpcClient {
    EnclaveRpcClient::new(
        store.clone(),
        secret_runtime.clone(),
        oauth_client.clone(),
        GoogleEnclaveOauthConfig {
            client_id: config.google_client_id.clone(),
            client_secret: config.google_client_secret.clone(),
            token_url: config.google_token_url.clone(),
            revoke_url: config.google_revoke_url.clone(),
        },
    )
}

fn map_exchange_enclave_error(err: EnclaveRpcError) -> JobExecutionError {
    match err {
        EnclaveRpcError::DecryptNotAuthorized(err) => JobExecutionError::permanent(
            "CONNECTOR_DECRYPT_NOT_AUTHORIZED",
            format!("decrypt authorization failed: {err}"),
        ),
        EnclaveRpcError::ConnectorTokenDecryptFailed(err) => JobExecutionError::transient(
            "CONNECTOR_TOKEN_DECRYPT_FAILED",
            format!("failed to decrypt refresh token: {err}"),
        ),
        EnclaveRpcError::ConnectorTokenUnavailable => JobExecutionError::permanent(
            "CONNECTOR_TOKEN_MISSING",
            "refresh token was unavailable for active connector",
        ),
        EnclaveRpcError::ProviderRequestUnavailable { message, .. } => {
            JobExecutionError::transient(
                "GOOGLE_TOKEN_REFRESH_UNAVAILABLE",
                format!("google token refresh request failed: {message}"),
            )
        }
        EnclaveRpcError::ProviderRequestFailed {
            status,
            oauth_error,
            ..
        } => {
            let status =
                reqwest::StatusCode::from_u16(status).unwrap_or(reqwest::StatusCode::BAD_GATEWAY);
            let message = match oauth_error {
                Some(error) => format!("google token refresh rejected: {error}"),
                None => format!("google token refresh failed with HTTP {}", status.as_u16()),
            };
            classified_http_error(status, "GOOGLE_TOKEN_REFRESH_FAILED", message)
        }
        EnclaveRpcError::ProviderResponseInvalid { message, .. } => JobExecutionError::transient(
            "GOOGLE_TOKEN_REFRESH_PARSE_FAILED",
            format!("google token refresh response was invalid: {message}"),
        ),
    }
}
