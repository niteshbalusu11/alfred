use shared::config::WorkerConfig;
use shared::enclave::{ConnectorSecretRequest, EnclaveRpcClient, EnclaveRpcError};
use shared::repos::{ActiveConnectorMetadata, Store};
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
    connector: ActiveConnectorMetadata,
) -> Result<ActiveConnectorMetadata, DeleteRequestError> {
    if connector.token_key_id == config.kms_key_id
        && connector.token_version == config.kms_key_version
    {
        return Ok(connector);
    }

    match store
        .ensure_active_connector_key_metadata(
            user_id,
            connector.connector_id,
            &config.kms_key_id,
            config.kms_key_version,
        )
        .await
    {
        Ok(Some(_)) => Ok(connector),
        Ok(None) => Err(DeleteRequestError::new(
            "CONNECTOR_KEY_METADATA_MISSING",
            "connector key metadata changed during delete workflow",
        )),
        Err(_err) => Err(DeleteRequestError::new(
            "CONNECTOR_KEY_METADATA_UPDATE_FAILED",
            "failed to rotate connector key metadata",
        )),
    }
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
        EnclaveRpcError::DecryptNotAuthorized { .. } => DeleteRequestError::new(
            "CONNECTOR_DECRYPT_NOT_AUTHORIZED",
            "decrypt authorization failed",
        ),
        EnclaveRpcError::ConnectorTokenDecryptFailed { .. } => DeleteRequestError::new(
            "CONNECTOR_TOKEN_DECRYPT_FAILED",
            "failed to decrypt connector token",
        ),
        EnclaveRpcError::ConnectorTokenUnavailable => DeleteRequestError::new(
            "CONNECTOR_TOKEN_MISSING",
            "refresh token was unavailable for active connector",
        ),
        EnclaveRpcError::ProviderRequestUnavailable { .. } => DeleteRequestError::new(
            "GOOGLE_REVOKE_UNAVAILABLE",
            "failed to call Google revoke endpoint",
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
        EnclaveRpcError::RpcTransportUnavailable { .. }
        | EnclaveRpcError::RpcResponseInvalid { .. } => {
            DeleteRequestError::new("ENCLAVE_RPC_UNAVAILABLE", "secure enclave rpc unavailable")
        }
    }
}

#[cfg(test)]
mod tests {
    use shared::enclave::ProviderOperation;

    use super::*;

    #[test]
    fn provider_unavailable_error_message_is_sanitized() {
        let err = map_revoke_enclave_error(EnclaveRpcError::ProviderRequestUnavailable {
            operation: ProviderOperation::TokenRevoke,
            message: "timeout with refresh_token=abcd".to_string(),
        });

        assert_eq!(err.code, "GOOGLE_REVOKE_UNAVAILABLE");
        assert_eq!(err.message, "failed to call Google revoke endpoint");
    }

    #[test]
    fn rpc_unavailable_error_message_is_sanitized() {
        let err = map_revoke_enclave_error(EnclaveRpcError::RpcTransportUnavailable {
            message: "authorization header leak".to_string(),
        });

        assert_eq!(err.code, "ENCLAVE_RPC_UNAVAILABLE");
        assert_eq!(err.message, "secure enclave rpc unavailable");
    }
}
