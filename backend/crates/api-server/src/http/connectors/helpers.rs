use axum::http::StatusCode;
use axum::response::Response;
use shared::enclave::{EnclaveRpcClient, EnclaveRpcError};
use tracing::warn;
use url::Url;

use super::super::errors::{
    bad_gateway_response, bad_request_response, decrypt_not_authorized_response,
};
use super::super::{AppState, OAuthConfig};
use super::types::{GoogleOAuthErrorResponse, GoogleTokenResponse};

pub(super) fn build_enclave_client(state: &AppState) -> EnclaveRpcClient {
    EnclaveRpcClient::new(
        state.enclave_rpc.base_url.clone(),
        state.enclave_rpc.auth.clone(),
        state.http_client.clone(),
    )
}

pub(super) fn map_revoke_enclave_error(err: EnclaveRpcError) -> Response {
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
        EnclaveRpcError::ProviderRequestUnavailable { message, .. } => {
            warn!("oauth revoke request failed: {message}");
            bad_gateway_response(
                "oauth_revoke_unavailable",
                "Unable to reach Google OAuth revoke endpoint",
            )
        }
        EnclaveRpcError::ProviderRequestFailed { status, .. } => {
            warn!("oauth revoke failed: status={status}");
            bad_gateway_response("oauth_revoke_failed", "Google token revoke failed")
        }
        EnclaveRpcError::ProviderResponseInvalid { .. } => {
            bad_gateway_response("oauth_revoke_failed", "Google token revoke failed")
        }
        EnclaveRpcError::RpcUnauthorized { .. }
        | EnclaveRpcError::RpcContractRejected { .. }
        | EnclaveRpcError::RpcTransportUnavailable { .. }
        | EnclaveRpcError::RpcResponseInvalid { .. } => {
            bad_gateway_response("enclave_rpc_failed", "Secure enclave RPC request failed")
        }
    }
}

pub(super) async fn exchange_google_code(
    client: &reqwest::Client,
    oauth: &OAuthConfig,
    code: &str,
    redirect_uri: &str,
) -> Result<GoogleTokenResponse, Response> {
    let response = match client
        .post(&oauth.token_url)
        .form(&[
            ("code", code),
            ("client_id", &oauth.client_id),
            ("client_secret", &oauth.client_secret),
            ("redirect_uri", redirect_uri),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
    {
        Ok(response) => response,
        Err(err) => {
            warn!("oauth token request failed: {err}");
            return Err(bad_gateway_response(
                "oauth_unavailable",
                "Unable to reach Google OAuth token endpoint",
            ));
        }
    };

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if status == StatusCode::BAD_REQUEST
            && let Some(error) = parse_google_oauth_error(&body)
        {
            if error.error == "invalid_grant" {
                return Err(bad_request_response(
                    "invalid_oauth_code",
                    "Authorization code is invalid or expired",
                ));
            }

            if error.error == "access_denied" {
                return Err(bad_request_response(
                    "oauth_consent_denied",
                    error
                        .error_description
                        .as_deref()
                        .unwrap_or("Google consent was denied"),
                ));
            }
        }

        warn!("oauth token exchange failed: status={status}");
        return Err(bad_gateway_response(
            "oauth_token_exchange_failed",
            "Google OAuth token exchange failed",
        ));
    }

    match response.json::<GoogleTokenResponse>().await {
        Ok(token_response) => Ok(token_response),
        Err(err) => {
            warn!("oauth token parse failed: {err}");
            Err(bad_gateway_response(
                "oauth_invalid_response",
                "Google OAuth token response was invalid",
            ))
        }
    }
}

fn parse_google_oauth_error(body: &str) -> Option<GoogleOAuthErrorResponse> {
    serde_json::from_str::<GoogleOAuthErrorResponse>(body).ok()
}

pub(super) fn build_google_auth_url(
    oauth: &OAuthConfig,
    state_token: &str,
) -> Result<String, url::ParseError> {
    let mut url = Url::parse(&oauth.auth_url)?;
    url.query_pairs_mut()
        .append_pair("client_id", &oauth.client_id)
        .append_pair("redirect_uri", &oauth.redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", &oauth.scopes.join(" "))
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent")
        .append_pair("state", state_token);

    Ok(url.to_string())
}
