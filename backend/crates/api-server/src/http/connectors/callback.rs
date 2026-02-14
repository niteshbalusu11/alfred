use std::collections::HashMap;

use axum::Json;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use shared::models::{
    CompleteGoogleConnectRequest, CompleteGoogleConnectResponse, ConnectorStatus,
};
use shared::repos::{AuditResult, JobType};

use super::super::errors::{bad_request_response, store_error_response};
use super::super::observability::RequestContext;
use super::super::tokens::hash_token;
use super::super::{AppState, AuthUser};
use super::helpers::exchange_google_code;

pub(crate) async fn complete_google_connect(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Extension(request_context): Extension<RequestContext>,
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

    let trace_payload =
        super::super::observability::request_trace_payload(&request_context.request_id);
    if let Err(err) = state
        .store
        .enqueue_job(
            user.user_id,
            JobType::UrgentEmailCheck,
            Utc::now(),
            Some(&trace_payload),
        )
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
