use axum::response::Response;
use shared::enclave::{ConnectorSecretRequest, EnclaveRpcClient, EnclaveRpcError};
use shared::repos::LEGACY_CONNECTOR_TOKEN_KEY_ID;
use uuid::Uuid;

use super::super::AppState;
use super::super::errors::{
    bad_gateway_response, bad_request_response, decrypt_not_authorized_response,
    store_error_response,
};

pub(super) struct GoogleSession {
    pub(super) access_token: String,
}

pub(super) async fn build_google_session(
    state: &AppState,
    user_id: Uuid,
) -> Result<GoogleSession, Response> {
    let active_connector = load_active_google_connector(state, user_id).await?;
    let enclave_client = build_enclave_client(state);

    let token_response = match enclave_client
        .exchange_google_access_token(ConnectorSecretRequest {
            user_id,
            connector_id: active_connector.connector_id,
        })
        .await
    {
        Ok(token_response) => token_response,
        Err(err) => return Err(map_token_exchange_error(err)),
    };

    Ok(GoogleSession {
        access_token: token_response.access_token,
    })
}

#[derive(Clone)]
struct ActiveGoogleConnector {
    connector_id: Uuid,
}

async fn load_active_google_connector(
    state: &AppState,
    user_id: Uuid,
) -> Result<ActiveGoogleConnector, Response> {
    let mut connector = match state.store.list_active_connector_metadata(user_id).await {
        Ok(connectors) => connectors
            .into_iter()
            .find(|connector| connector.provider == "google"),
        Err(err) => return Err(store_error_response(err)),
    }
    .ok_or_else(|| {
        bad_request_response(
            "google_connector_not_active",
            "No active Google connector found for this user",
        )
    })?;

    if connector.token_key_id == LEGACY_CONNECTOR_TOKEN_KEY_ID {
        if let Err(err) = state
            .store
            .adopt_legacy_connector_token_key_id(
                user_id,
                connector.connector_id,
                state.secret_runtime.kms_key_id(),
                state.secret_runtime.kms_key_version(),
            )
            .await
        {
            return Err(store_error_response(err));
        }

        let refreshed_connector = match state
            .store
            .get_active_connector_key_metadata(user_id, connector.connector_id)
            .await
        {
            Ok(Some(metadata)) => metadata,
            Ok(None) => {
                return Err(bad_request_response(
                    "connector_token_unavailable",
                    "Connector token metadata changed; retry the request",
                ));
            }
            Err(err) => return Err(store_error_response(err)),
        };

        connector.token_key_id = refreshed_connector.token_key_id;
        connector.token_version = refreshed_connector.token_version;
    }

    Ok(ActiveGoogleConnector {
        connector_id: connector.connector_id,
    })
}

fn build_enclave_client(state: &AppState) -> EnclaveRpcClient {
    EnclaveRpcClient::new(
        state.enclave_rpc.base_url.clone(),
        state.enclave_rpc.auth.clone(),
        state.http_client.clone(),
    )
}

fn map_token_exchange_error(err: EnclaveRpcError) -> Response {
    match err {
        EnclaveRpcError::DecryptNotAuthorized { .. } => decrypt_not_authorized_response(),
        EnclaveRpcError::ConnectorTokenDecryptFailed { .. } => bad_gateway_response(
            "connector_token_decrypt_failed",
            "Connector token decrypt failed",
        ),
        EnclaveRpcError::ConnectorTokenUnavailable => bad_request_response(
            "connector_token_unavailable",
            "Connector token metadata changed; retry the request",
        ),
        EnclaveRpcError::ProviderRequestUnavailable { .. } => bad_gateway_response(
            "google_token_refresh_unavailable",
            "Unable to reach Google OAuth token endpoint",
        ),
        EnclaveRpcError::ProviderRequestFailed { .. }
        | EnclaveRpcError::ProviderResponseInvalid { .. } => bad_gateway_response(
            "google_token_refresh_failed",
            "Google OAuth token refresh failed",
        ),
        EnclaveRpcError::RpcUnauthorized { .. }
        | EnclaveRpcError::RpcContractRejected { .. }
        | EnclaveRpcError::RpcTransportUnavailable { .. }
        | EnclaveRpcError::RpcResponseInvalid { .. } => {
            bad_gateway_response("enclave_rpc_failed", "Secure enclave RPC request failed")
        }
    }
}
