use axum::response::Response;
use shared::enclave::{ConnectorSecretRequest, EnclaveRpcClient};
use uuid::Uuid;

use super::super::AppState;
use super::super::errors::{bad_request_response, store_error_response};

pub(super) struct GoogleSession {
    pub(super) connector_request: ConnectorSecretRequest,
}

pub(super) async fn build_google_session(
    state: &AppState,
    user_id: Uuid,
) -> Result<GoogleSession, Response> {
    let active_connector = load_active_google_connector(state, user_id).await?;

    Ok(GoogleSession {
        connector_request: ConnectorSecretRequest {
            user_id,
            connector_id: active_connector.connector_id,
        },
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
    let connector = match state.store.list_active_connector_metadata(user_id).await {
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

    if connector.token_key_id != state.secret_runtime.kms_key_id()
        || connector.token_version != state.secret_runtime.kms_key_version()
    {
        match state
            .store
            .ensure_active_connector_key_metadata(
                user_id,
                connector.connector_id,
                state.secret_runtime.kms_key_id(),
                state.secret_runtime.kms_key_version(),
            )
            .await
        {
            Ok(Some(_)) => {}
            Ok(None) => {
                return Err(bad_request_response(
                    "connector_token_unavailable",
                    "Connector token metadata changed; retry the request",
                ));
            }
            Err(err) => return Err(store_error_response(err)),
        }
    }

    Ok(ActiveGoogleConnector {
        connector_id: connector.connector_id,
    })
}

pub(super) fn build_enclave_client(state: &AppState) -> EnclaveRpcClient {
    EnclaveRpcClient::new(
        state.enclave_rpc.base_url.clone(),
        state.enclave_rpc.auth.clone(),
        state.http_client.clone(),
    )
}
