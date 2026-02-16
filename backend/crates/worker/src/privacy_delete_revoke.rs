use shared::config::WorkerConfig;
use shared::enclave::{ConnectorSecretRequest, EnclaveRpcClient, EnclaveRpcError};
use shared::repos::{ActiveConnectorMetadata, LEGACY_CONNECTOR_TOKEN_KEY_ID, Store};
use shared::security::SecretRuntime;
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
    _secret_runtime: &SecretRuntime,
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
    let enclave_client = build_enclave_client(config, oauth_client);
    let revoke_response = enclave_client
        .revoke_google_connector_token(ConnectorSecretRequest {
            user_id,
            connector_id: connector.connector_id,
        })
        .await
        .map_err(map_revoke_enclave_error)?;

    info!(
        user_id = %user_id,
        connector_id = %connector.connector_id,
        attested_measurement = %revoke_response.attested_identity.measurement,
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

fn build_enclave_client(config: &WorkerConfig, oauth_client: &reqwest::Client) -> EnclaveRpcClient {
    EnclaveRpcClient::new(
        config.enclave_runtime_base_url.clone(),
        shared::enclave::EnclaveRpcAuthConfig {
            shared_secret: config.enclave_rpc_shared_secret.clone(),
            max_clock_skew_seconds: config.enclave_rpc_auth_max_skew_seconds,
        },
        oauth_client.clone(),
    )
}

fn map_revoke_enclave_error(err: EnclaveRpcError) -> DeleteRequestError {
    match err {
        EnclaveRpcError::DecryptNotAuthorized { message } => DeleteRequestError::new(
            "CONNECTOR_DECRYPT_NOT_AUTHORIZED",
            format!("decrypt authorization failed: {message}"),
        ),
        EnclaveRpcError::ConnectorTokenDecryptFailed { message } => DeleteRequestError::new(
            "CONNECTOR_TOKEN_DECRYPT_FAILED",
            format!("failed to decrypt refresh token: {message}"),
        ),
        EnclaveRpcError::ConnectorTokenUnavailable => DeleteRequestError::new(
            "CONNECTOR_TOKEN_MISSING",
            "refresh token was unavailable for active connector",
        ),
        EnclaveRpcError::ProviderRequestUnavailable { message, .. } => DeleteRequestError::new(
            "GOOGLE_REVOKE_UNAVAILABLE",
            format!("failed to call Google revoke endpoint: {message}"),
        ),
        EnclaveRpcError::ProviderRequestFailed { status, .. } => DeleteRequestError::new(
            "GOOGLE_REVOKE_FAILED",
            format!("Google revoke endpoint returned HTTP {status}"),
        ),
        EnclaveRpcError::ProviderResponseInvalid { .. } => DeleteRequestError::new(
            "GOOGLE_REVOKE_FAILED",
            "Google revoke endpoint returned an invalid response",
        ),
        EnclaveRpcError::RpcUnauthorized { code }
        | EnclaveRpcError::RpcContractRejected { code } => DeleteRequestError::new(
            "ENCLAVE_RPC_REJECTED",
            format!("secure enclave rpc request rejected: {code}"),
        ),
        EnclaveRpcError::RpcTransportUnavailable { message }
        | EnclaveRpcError::RpcResponseInvalid { message } => DeleteRequestError::new(
            "ENCLAVE_RPC_UNAVAILABLE",
            format!("secure enclave rpc unavailable: {message}"),
        ),
    }
}
