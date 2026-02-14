use std::collections::HashMap;

use axum::Json;
use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::{Duration, Utc};
use serde::Deserialize;
use shared::models::{
    CompleteGoogleConnectRequest, CompleteGoogleConnectResponse, ConnectorStatus, ErrorBody,
    ErrorResponse, RevokeConnectorResponse, StartGoogleConnectRequest, StartGoogleConnectResponse,
};
use shared::repos::{AuditResult, JobType, LEGACY_CONNECTOR_TOKEN_KEY_ID};
use shared::security::ConnectorKeyMetadata;
use tracing::warn;
use url::Url;
use uuid::Uuid;

use super::errors::{
    bad_gateway_response, bad_request_response, security_error_response, store_error_response,
};
use super::tokens::{generate_secure_token, hash_token};
use super::{AppState, AuthUser, OAuthConfig};

pub(super) async fn start_google_connect(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Json(req): Json<StartGoogleConnectRequest>,
) -> Response {
    if req.redirect_uri != state.oauth.redirect_uri {
        return bad_request_response(
            "invalid_redirect_uri",
            "Provided redirect URI does not match configured redirect URI",
        );
    }

    let state_token = generate_secure_token("st");

    if let Err(err) = state
        .store
        .store_oauth_state(
            user.user_id,
            &hash_token(&state_token),
            &state.oauth.redirect_uri,
            Utc::now() + Duration::seconds(state.oauth_state_ttl_seconds as i64),
        )
        .await
    {
        return store_error_response(err);
    }

    let auth_url = match build_google_auth_url(&state.oauth, &state_token) {
        Ok(auth_url) => auth_url,
        Err(err) => {
            warn!("failed to construct oauth url: {err}");
            return bad_request_response(
                "oauth_config_error",
                "Google OAuth configuration is invalid",
            );
        }
    };

    let response = StartGoogleConnectResponse {
        auth_url,
        state: state_token,
    };

    let mut metadata = HashMap::new();
    metadata.insert("redirect_uri".to_string(), req.redirect_uri);

    if let Err(err) = state
        .store
        .add_audit_event(
            user.user_id,
            "GOOGLE_CONNECT_STARTED",
            Some("google"),
            AuditResult::Success,
            &metadata,
        )
        .await
    {
        return store_error_response(err);
    }

    (StatusCode::OK, Json(response)).into_response()
}

pub(super) async fn complete_google_connect(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Json(req): Json<CompleteGoogleConnectRequest>,
) -> Response {
    let Some(redirect_uri) = (match state
        .store
        .consume_oauth_state(user.user_id, &hash_token(&req.state), Utc::now())
        .await
    {
        Ok(redirect_uri) => redirect_uri,
        Err(err) => return store_error_response(err),
    }) else {
        return bad_request_response("invalid_state", "OAuth state is invalid or expired");
    };

    if let Some(error) = req.error.as_deref() {
        if error == "access_denied" {
            return bad_request_response(
                "oauth_consent_denied",
                req.error_description
                    .as_deref()
                    .unwrap_or("Google consent was denied"),
            );
        }

        return bad_request_response(
            "oauth_callback_error",
            "Google OAuth callback contained an error",
        );
    }

    let code = match req
        .code
        .as_deref()
        .map(str::trim)
        .filter(|code| !code.is_empty())
    {
        Some(code) => code,
        None => {
            return bad_request_response(
                "invalid_oauth_code",
                "Authorization code is missing or invalid",
            );
        }
    };

    let token_response =
        match exchange_google_code(&state.http_client, &state.oauth, code, &redirect_uri).await {
            Ok(token_response) => token_response,
            Err(response) => return response,
        };

    let Some(refresh_token) = token_response.refresh_token else {
        return bad_request_response(
            "missing_refresh_token",
            "Google did not return a refresh token",
        );
    };

    let granted_scopes = token_response
        .scope
        .map(|scope| {
            scope
                .split_whitespace()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| state.oauth.scopes.clone());

    let connector_id = match state
        .store
        .upsert_google_connector(
            user.user_id,
            &refresh_token,
            &granted_scopes,
            state.secret_runtime.kms_key_id(),
            state.secret_runtime.kms_key_version(),
        )
        .await
    {
        Ok(connector_id) => connector_id,
        Err(err) => return store_error_response(err),
    };

    if let Err(err) = state
        .store
        .enqueue_job(user.user_id, JobType::UrgentEmailCheck, Utc::now(), None)
        .await
    {
        return store_error_response(err);
    }

    let mut metadata = HashMap::new();
    metadata.insert("connector_id".to_string(), connector_id.to_string());

    if let Err(err) = state
        .store
        .add_audit_event(
            user.user_id,
            "GOOGLE_CONNECT_COMPLETED",
            Some("google"),
            AuditResult::Success,
            &metadata,
        )
        .await
    {
        return store_error_response(err);
    }

    let response = CompleteGoogleConnectResponse {
        connector_id: connector_id.to_string(),
        status: ConnectorStatus::Active,
        granted_scopes,
    };

    (StatusCode::OK, Json(response)).into_response()
}

pub(super) async fn revoke_connector(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(connector_id): Path<String>,
) -> Response {
    let connector_id = match Uuid::parse_str(&connector_id) {
        Ok(connector_id) => connector_id,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: ErrorBody {
                        code: "not_found".to_string(),
                        message: "Connector not found".to_string(),
                    },
                }),
            )
                .into_response();
        }
    };

    let mut connector_metadata = match state
        .store
        .get_active_connector_key_metadata(user.user_id, connector_id)
        .await
    {
        Ok(Some(connector_metadata)) => connector_metadata,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: ErrorBody {
                        code: "not_found".to_string(),
                        message: "Connector not found".to_string(),
                    },
                }),
            )
                .into_response();
        }
        Err(err) => return store_error_response(err),
    };

    if connector_metadata.provider != "google" {
        return bad_request_response(
            "unsupported_provider",
            "Connector provider is not supported",
        );
    }

    if connector_metadata.token_key_id == LEGACY_CONNECTOR_TOKEN_KEY_ID {
        if let Err(err) = state
            .store
            .adopt_legacy_connector_token_key_id(
                user.user_id,
                connector_id,
                state.secret_runtime.kms_key_id(),
                state.secret_runtime.kms_key_version(),
            )
            .await
        {
            return store_error_response(err);
        }

        connector_metadata = match state
            .store
            .get_active_connector_key_metadata(user.user_id, connector_id)
            .await
        {
            Ok(Some(connector_metadata)) => connector_metadata,
            Ok(None) => {
                return bad_request_response(
                    "connector_token_unavailable",
                    "Connector token metadata changed; retry the request",
                );
            }
            Err(err) => return store_error_response(err),
        };
    }

    let attested_identity =
        match state
            .secret_runtime
            .authorize_connector_decrypt(&ConnectorKeyMetadata {
                key_id: connector_metadata.token_key_id.clone(),
                key_version: connector_metadata.token_version,
            }) {
            Ok(attested_identity) => attested_identity,
            Err(err) => return security_error_response(err),
        };

    let refresh_token = match state
        .store
        .decrypt_active_connector_refresh_token(
            user.user_id,
            connector_id,
            &connector_metadata.token_key_id,
            connector_metadata.token_version,
        )
        .await
    {
        Ok(Some(refresh_token)) => refresh_token,
        Ok(None) => {
            return bad_request_response(
                "connector_token_unavailable",
                "Connector token metadata changed; retry the request",
            );
        }
        Err(err) => return store_error_response(err),
    };

    if let Err(response) =
        revoke_google_token(&state.http_client, &state.oauth, &refresh_token).await
    {
        return response;
    }

    match state
        .store
        .revoke_connector(user.user_id, connector_id)
        .await
    {
        Ok(true) => {
            let mut metadata = HashMap::new();
            metadata.insert("connector_id".to_string(), connector_id.to_string());
            metadata.insert(
                "attested_measurement".to_string(),
                attested_identity.measurement,
            );

            if let Err(err) = state
                .store
                .add_audit_event(
                    user.user_id,
                    "CONNECTOR_REVOKED",
                    Some("google"),
                    AuditResult::Success,
                    &metadata,
                )
                .await
            {
                return store_error_response(err);
            }

            (
                StatusCode::OK,
                Json(RevokeConnectorResponse {
                    status: ConnectorStatus::Revoked,
                }),
            )
                .into_response()
        }
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: ErrorBody {
                    code: "not_found".to_string(),
                    message: "Connector not found".to_string(),
                },
            }),
        )
            .into_response(),
        Err(err) => store_error_response(err),
    }
}

#[derive(Debug, Deserialize)]
struct GoogleTokenResponse {
    refresh_token: Option<String>,
    scope: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoogleOAuthErrorResponse {
    error: String,
    error_description: Option<String>,
}

async fn exchange_google_code(
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

async fn revoke_google_token(
    client: &reqwest::Client,
    oauth: &OAuthConfig,
    refresh_token: &str,
) -> Result<(), Response> {
    let response = match client
        .post(&oauth.revoke_url)
        .form(&[("token", refresh_token)])
        .send()
        .await
    {
        Ok(response) => response,
        Err(err) => {
            warn!("oauth revoke request failed: {err}");
            return Err(bad_gateway_response(
                "oauth_revoke_unavailable",
                "Unable to reach Google OAuth revoke endpoint",
            ));
        }
    };

    if response.status().is_success() {
        return Ok(());
    }

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if status == StatusCode::BAD_REQUEST
        && let Some(error) = parse_google_oauth_error(&body)
        && error.error == "invalid_token"
    {
        return Ok(());
    }

    warn!("oauth revoke failed: status={status}");
    Err(bad_gateway_response(
        "oauth_revoke_failed",
        "Google token revoke failed",
    ))
}

fn parse_google_oauth_error(body: &str) -> Option<GoogleOAuthErrorResponse> {
    serde_json::from_str::<GoogleOAuthErrorResponse>(body).ok()
}

fn build_google_auth_url(
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
