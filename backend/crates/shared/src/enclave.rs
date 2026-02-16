use std::fmt;

use reqwest::StatusCode;
use serde::Deserialize;
use thiserror::Error;
use uuid::Uuid;

use crate::repos::{ConnectorKeyMetadata as PersistedConnectorKeyMetadata, Store, StoreError};
use crate::security::{
    AttestedIdentity, ConnectorKeyMetadata as AuthorizedConnectorKeyMetadata, SecretRuntime,
    SecurityError,
};

#[derive(Debug, Clone)]
pub struct GoogleEnclaveOauthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub token_url: String,
    pub revoke_url: String,
}

#[derive(Debug, Clone)]
pub struct ConnectorSecretRequest {
    pub user_id: Uuid,
    pub connector_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct ExchangeGoogleTokenResponse {
    pub access_token: String,
    pub attested_identity: AttestedIdentity,
}

#[derive(Debug, Clone)]
pub struct RevokeGoogleTokenResponse {
    pub attested_identity: AttestedIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderOperation {
    TokenRefresh,
    TokenRevoke,
}

impl fmt::Display for ProviderOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TokenRefresh => write!(f, "token_refresh"),
            Self::TokenRevoke => write!(f, "token_revoke"),
        }
    }
}

#[derive(Debug, Error)]
pub enum EnclaveRpcError {
    #[error("connector decrypt authorization failed: {0}")]
    DecryptNotAuthorized(#[source] SecurityError),
    #[error("connector token decrypt failed: {0}")]
    ConnectorTokenDecryptFailed(#[source] StoreError),
    #[error("connector token is unavailable for active connector")]
    ConnectorTokenUnavailable,
    #[error("provider request unavailable for {operation}: {message}")]
    ProviderRequestUnavailable {
        operation: ProviderOperation,
        message: String,
    },
    #[error("provider request failed for {operation}: status={status}")]
    ProviderRequestFailed {
        operation: ProviderOperation,
        status: u16,
        oauth_error: Option<String>,
    },
    #[error("provider response invalid for {operation}: {message}")]
    ProviderResponseInvalid {
        operation: ProviderOperation,
        message: String,
    },
}

#[derive(Clone)]
pub struct EnclaveRpcClient {
    store: Store,
    secret_runtime: SecretRuntime,
    http_client: reqwest::Client,
    oauth: GoogleEnclaveOauthConfig,
}

impl EnclaveRpcClient {
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
    ) -> Result<(String, AttestedIdentity), EnclaveRpcError> {
        let connector_metadata = self
            .store
            .get_active_connector_key_metadata(request.user_id, request.connector_id)
            .await
            .map_err(EnclaveRpcError::ConnectorTokenDecryptFailed)?
            .ok_or(EnclaveRpcError::ConnectorTokenUnavailable)?;

        let attested_identity = self
            .secret_runtime
            .authorize_connector_decrypt(&AuthorizedConnectorKeyMetadata {
                key_id: connector_metadata.token_key_id.clone(),
                key_version: connector_metadata.token_version,
            })
            .await
            .map_err(EnclaveRpcError::DecryptNotAuthorized)?;

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
            .map_err(EnclaveRpcError::ConnectorTokenDecryptFailed)?
            .ok_or(EnclaveRpcError::ConnectorTokenUnavailable)?;

        Ok((refresh_token, attested_identity))
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::{EnclaveRpcError, ProviderOperation};

    #[test]
    fn enclave_error_messages_do_not_include_refresh_tokens() {
        let err = EnclaveRpcError::ProviderRequestFailed {
            operation: ProviderOperation::TokenRefresh,
            status: 400,
            oauth_error: Some("invalid_grant".to_string()),
        };

        let rendered = err.to_string().to_ascii_lowercase();
        assert!(!rendered.contains("refresh_token"));
        assert!(!rendered.contains("client_secret"));
    }

    #[test]
    fn sensitive_worker_api_paths_do_not_log_secret_token_fields() {
        let shared_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let files = [
            shared_root.join("../api-server/src/http/assistant/session.rs"),
            shared_root.join("../api-server/src/http/connectors.rs"),
            shared_root.join("../api-server/src/http/connectors/revoke.rs"),
            shared_root.join("../worker/src/job_actions/google/session.rs"),
            shared_root.join("../worker/src/privacy_delete_revoke.rs"),
        ];

        for file in files {
            let content = fs::read_to_string(&file)
                .expect("failed to read source file for secret logging guard test");
            assert_no_sensitive_tracing_args(file.display().to_string().as_str(), &content);
        }
    }

    #[test]
    fn host_paths_do_not_call_store_decrypt_directly() {
        let shared_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let files = [
            shared_root.join("../api-server/src/http/assistant/session.rs"),
            shared_root.join("../api-server/src/http/connectors/revoke.rs"),
            shared_root.join("../worker/src/job_actions/google/session.rs"),
            shared_root.join("../worker/src/privacy_delete_revoke.rs"),
        ];

        for file in files {
            let content = fs::read_to_string(&file)
                .expect("failed to read source file for decrypt boundary guard test");
            assert!(
                !content.contains("decrypt_active_connector_refresh_token("),
                "{} must not call connector decrypt repository API directly",
                file.display()
            );
        }
    }

    fn assert_no_sensitive_tracing_args(path: &str, content: &str) {
        const TRACING_MACROS: [&str; 5] = ["trace!(", "debug!(", "info!(", "warn!(", "error!("];
        const SENSITIVE_TERMS: [&str; 4] = [
            "refresh_token",
            "access_token",
            "client_secret",
            "apns_token",
        ];

        for macro_call in TRACING_MACROS {
            let mut from = 0;
            while let Some(start_offset) = content[from..].find(macro_call) {
                let start = from + start_offset;
                let Some(end_offset) = content[start..].find(");") else {
                    break;
                };
                let end = start + end_offset + 2;
                let snippet = content[start..end].to_ascii_lowercase();

                for term in SENSITIVE_TERMS {
                    assert!(
                        !snippet.contains(term),
                        "{path} contains sensitive term `{term}` in tracing macro: {snippet}"
                    );
                }

                from = end;
            }
        }
    }
}
