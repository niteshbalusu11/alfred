use shared::config::WorkerConfig;
use shared::enclave::{ConnectorSecretRequest, EnclaveRpcAuthConfig, EnclaveRpcClient};
use shared::repos::Store;
use shared::security::SecretRuntime;

use crate::JobExecutionError;

pub(super) struct GoogleSession {
    pub(super) connector_request: ConnectorSecretRequest,
}

pub(super) async fn build_google_session(
    store: &Store,
    config: &WorkerConfig,
    _secret_runtime: &SecretRuntime,
    _oauth_client: &reqwest::Client,
    user_id: uuid::Uuid,
) -> Result<GoogleSession, JobExecutionError> {
    let connector = load_active_google_connector(store, config, user_id).await?;

    Ok(GoogleSession {
        connector_request: ConnectorSecretRequest {
            user_id,
            connector_id: connector.connector_id,
        },
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
    let connector = store
        .list_active_connector_metadata(user_id)
        .await
        .map_err(|_err| {
            JobExecutionError::transient(
                "CONNECTOR_METADATA_READ_FAILED",
                "failed to read connector metadata",
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

    if connector.token_key_id != config.kms_key_id
        || connector.token_version != config.kms_key_version
    {
        match store
            .ensure_active_connector_key_metadata(
                user_id,
                connector.connector_id,
                &config.kms_key_id,
                config.kms_key_version,
            )
            .await
        {
            Ok(Some(_)) => {}
            Ok(None) => {
                return Err(JobExecutionError::permanent(
                    "CONNECTOR_KEY_METADATA_MISSING",
                    "connector key metadata changed; retry the job",
                ));
            }
            Err(_err) => {
                return Err(JobExecutionError::transient(
                    "CONNECTOR_KEY_METADATA_UPDATE_FAILED",
                    "failed to rotate connector key metadata",
                ));
            }
        }
    }

    Ok(ActiveGoogleConnector {
        connector_id: connector.connector_id,
    })
}

pub(super) fn build_enclave_client(
    config: &WorkerConfig,
    oauth_client: &reqwest::Client,
) -> EnclaveRpcClient {
    EnclaveRpcClient::new(
        config.enclave_runtime_base_url.clone(),
        EnclaveRpcAuthConfig {
            shared_secret: config.enclave_rpc_shared_secret.clone(),
            max_clock_skew_seconds: config.enclave_rpc_auth_max_skew_seconds,
        },
        oauth_client.clone(),
    )
}
