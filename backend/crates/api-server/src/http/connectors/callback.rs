use std::collections::HashMap;

use axum::Json;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use shared::models::{
    CompleteGoogleConnectRequest, CompleteGoogleConnectResponse, ConnectorStatus,
};
use shared::repos::AuditResult;

use super::super::errors::{bad_request_response, store_error_response};
use super::super::tokens::hash_token;
use super::super::{AppState, AuthUser};
use super::helpers::{build_enclave_client, map_complete_connect_enclave_error};

pub(crate) async fn complete_google_connect(
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

    let enclave_client = build_enclave_client(&state);
    let connect_result = enclave_client
        .complete_google_connect(user.user_id, code.to_string(), redirect_uri)
        .await;
    let connect_result = match connect_result {
        Ok(response) => response,
        Err(err) => return map_complete_connect_enclave_error(err),
    };

    let mut metadata = HashMap::new();
    metadata.insert(
        "connector_id".to_string(),
        connect_result.connector_id.to_string(),
    );

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
        connector_id: connect_result.connector_id.to_string(),
        status: ConnectorStatus::Active,
        granted_scopes: connect_result.granted_scopes,
    };

    (StatusCode::OK, Json(response)).into_response()
}
