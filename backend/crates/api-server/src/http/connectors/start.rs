use std::collections::HashMap;

use axum::Json;
use axum::extract::{Extension, State};
use axum::response::{IntoResponse, Response};
use chrono::{Duration, Utc};
use shared::models::{StartGoogleConnectRequest, StartGoogleConnectResponse};
use shared::repos::AuditResult;
use tracing::warn;

use super::super::errors::{bad_request_response, store_error_response};
use super::super::tokens::{generate_secure_token, hash_token};
use super::super::{AppState, AuthUser};
use super::helpers::build_google_auth_url;

pub(crate) async fn start_google_connect(
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

    (axum::http::StatusCode::OK, Json(response)).into_response()
}
