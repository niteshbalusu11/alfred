use reqwest::StatusCode;
use serde::Deserialize;

use crate::repos::{ConnectorKeyMetadata as PersistedConnectorKeyMetadata, Store};
use crate::security::{ConnectorKeyMetadata as AuthorizedConnectorKeyMetadata, SecretRuntime};

use super::{
    AttestedIdentityPayload, ConnectorSecretRequest, EnclaveRpcError, ExchangeGoogleTokenResponse,
    GoogleEnclaveOauthConfig, ProviderOperation, RevokeGoogleTokenResponse,
};

#[derive(Clone)]
pub struct EnclaveOperationService {
    store: Store,
    secret_runtime: SecretRuntime,
    http_client: reqwest::Client,
    oauth: GoogleEnclaveOauthConfig,
}

impl EnclaveOperationService {
    pub fn new(
        store: Store,
        secret_runtime: SecretRuntime,
        http_client: reqwest::Client,
        oauth: GoogleEnclaveOauthConfig,
    ) -> Self {
        Self {
            store,
            secret_runtime,
            http_client,
            oauth,
        }
    }

    pub async fn exchange_google_access_token(
        &self,
        request: ConnectorSecretRequest,
    ) -> Result<ExchangeGoogleTokenResponse, EnclaveRpcError> {
        let (refresh_token, attested_identity) =
            self.load_authorized_refresh_token(&request).await?;

        let response = self
            .http_client
            .post(&self.oauth.token_url)
            .form(&[
                ("grant_type", "refresh_token"),
                ("client_id", self.oauth.client_id.as_str()),
                ("client_secret", self.oauth.client_secret.as_str()),
                ("refresh_token", refresh_token.as_str()),
            ])
            .send()
            .await
            .map_err(|err| EnclaveRpcError::ProviderRequestUnavailable {
                operation: ProviderOperation::TokenRefresh,
                message: err.to_string(),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            let oauth_error = parse_google_oauth_error(&body).and_then(|parsed| parsed.error);
            return Err(EnclaveRpcError::ProviderRequestFailed {
                operation: ProviderOperation::TokenRefresh,
                status: status.as_u16(),
                oauth_error,
            });
        }

        let payload = response
            .json::<GoogleRefreshTokenResponse>()
            .await
            .map_err(|err| EnclaveRpcError::ProviderResponseInvalid {
                operation: ProviderOperation::TokenRefresh,
                message: err.to_string(),
            })?;

        Ok(ExchangeGoogleTokenResponse {
            access_token: payload.access_token,
            attested_identity,
        })
    }

    pub async fn revoke_google_connector_token(
        &self,
        request: ConnectorSecretRequest,
    ) -> Result<RevokeGoogleTokenResponse, EnclaveRpcError> {
        let (refresh_token, attested_identity) =
            self.load_authorized_refresh_token(&request).await?;

        let response = self
            .http_client
            .post(&self.oauth.revoke_url)
            .form(&[("token", refresh_token.as_str())])
            .send()
            .await
            .map_err(|err| EnclaveRpcError::ProviderRequestUnavailable {
                operation: ProviderOperation::TokenRevoke,
                message: err.to_string(),
            })?;

        if response.status().is_success() {
            return Ok(RevokeGoogleTokenResponse { attested_identity });
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if status == StatusCode::BAD_REQUEST
            && let Some(error) = parse_google_oauth_error(&body).and_then(|parsed| parsed.error)
            && error == "invalid_token"
        {
            return Ok(RevokeGoogleTokenResponse { attested_identity });
        }

        let oauth_error = parse_google_oauth_error(&body).and_then(|parsed| parsed.error);
        Err(EnclaveRpcError::ProviderRequestFailed {
            operation: ProviderOperation::TokenRevoke,
            status: status.as_u16(),
            oauth_error,
        })
    }

    async fn load_authorized_refresh_token(
        &self,
        request: &ConnectorSecretRequest,
    ) -> Result<(String, AttestedIdentityPayload), EnclaveRpcError> {
        let connector_metadata = self
            .store
            .get_active_connector_key_metadata(request.user_id, request.connector_id)
            .await
            .map_err(|err| EnclaveRpcError::ConnectorTokenDecryptFailed {
                message: err.to_string(),
            })?
            .ok_or(EnclaveRpcError::ConnectorTokenUnavailable)?;

        let attested_identity = self
            .secret_runtime
            .authorize_connector_decrypt(&AuthorizedConnectorKeyMetadata {
                key_id: connector_metadata.token_key_id.clone(),
                key_version: connector_metadata.token_version,
            })
            .await
            .map_err(|err| EnclaveRpcError::DecryptNotAuthorized {
                message: err.to_string(),
            })?;

        let refresh_token = self
            .store
            .decrypt_active_connector_refresh_token(
                request.user_id,
                request.connector_id,
                &PersistedConnectorKeyMetadata {
                    provider: connector_metadata.provider,
                    token_key_id: connector_metadata.token_key_id,
                    token_version: connector_metadata.token_version,
                },
            )
            .await
            .map_err(|err| EnclaveRpcError::ConnectorTokenDecryptFailed {
                message: err.to_string(),
            })?
            .ok_or(EnclaveRpcError::ConnectorTokenUnavailable)?;

        Ok((
            refresh_token,
            AttestedIdentityPayload {
                runtime: attested_identity.runtime,
                measurement: attested_identity.measurement,
            },
        ))
    }
}

#[derive(Debug, Deserialize)]
struct GoogleRefreshTokenResponse {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct GoogleOAuthErrorResponse {
    error: Option<String>,
}

fn parse_google_oauth_error(body: &str) -> Option<GoogleOAuthErrorResponse> {
    serde_json::from_str::<GoogleOAuthErrorResponse>(body).ok()
}
