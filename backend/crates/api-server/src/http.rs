use std::collections::HashMap;

use axum::extract::{Extension, Path, Query, Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use chrono::{Duration, Utc};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use shared::models::{
    CompleteGoogleConnectRequest, CompleteGoogleConnectResponse, ConnectorStatus,
    CreateSessionRequest, CreateSessionResponse, DeleteAllResponse, ErrorBody, ErrorResponse,
    ListAuditEventsResponse, OkResponse, Preferences, RegisterDeviceRequest,
    RevokeConnectorResponse, StartGoogleConnectRequest, StartGoogleConnectResponse,
};
use shared::repos::{AuditResult, JobType, Store, StoreError};
use tracing::{error, warn};
use url::Url;
use uuid::Uuid;

#[derive(Clone)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub auth_url: String,
    pub token_url: String,
    pub scopes: Vec<String>,
}

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub oauth: OAuthConfig,
    pub session_ttl_seconds: u64,
    pub oauth_state_ttl_seconds: u64,
    pub http_client: reqwest::Client,
}

#[derive(Clone, Copy)]
struct AuthUser {
    user_id: Uuid,
}

pub fn build_router(app_state: AppState) -> Router {
    let public_routes = Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/v1/auth/ios/session", post(create_session))
        .with_state(app_state.clone());

    let auth_layer_state = app_state.clone();

    let protected_routes = Router::new()
        .route("/v1/devices/apns", post(register_device))
        .route("/v1/connectors/google/start", post(start_google_connect))
        .route(
            "/v1/connectors/google/callback",
            post(complete_google_connect),
        )
        .route("/v1/connectors/{connector_id}", delete(revoke_connector))
        .route(
            "/v1/preferences",
            get(get_preferences).put(update_preferences),
        )
        .route("/v1/audit-events", get(list_audit_events))
        .route("/v1/privacy/delete-all", post(delete_all))
        .layer(middleware::from_fn_with_state(
            auth_layer_state,
            auth_middleware,
        ))
        .with_state(app_state);

    public_routes.merge(protected_routes)
}

async fn auth_middleware(State(state): State<AppState>, mut req: Request, next: Next) -> Response {
    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();

    let token = auth_header
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|token| !token.is_empty());

    let Some(token) = token else {
        warn!("missing or invalid authorization header");
        return unauthorized_response();
    };

    let token_hash = hash_token(token);

    let user_id = match state
        .store
        .resolve_session_user(&token_hash, Utc::now())
        .await
    {
        Ok(Some(user_id)) => user_id,
        Ok(None) => return unauthorized_response(),
        Err(err) => return store_error_response(err),
    };

    req.extensions_mut().insert(AuthUser { user_id });
    next.run(req).await
}

async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, Json(OkResponse { ok: true }))
}

async fn readyz(State(state): State<AppState>) -> impl IntoResponse {
    match state.store.ping().await {
        Ok(_) => (StatusCode::OK, Json(OkResponse { ok: true })).into_response(),
        Err(err) => {
            warn!("readiness check failed: {err}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: ErrorBody {
                        code: "db_unavailable".to_string(),
                        message: "Database not ready".to_string(),
                    },
                }),
            )
                .into_response()
        }
    }
}

async fn create_session(
    State(state): State<AppState>,
    Json(_req): Json<CreateSessionRequest>,
) -> impl IntoResponse {
    let user_id = match state.store.create_user().await {
        Ok(user_id) => user_id,
        Err(err) => return store_error_response(err),
    };

    let access_token = generate_secure_token("at");
    let refresh_token = generate_secure_token("rt");
    let access_hash = hash_token(&access_token);
    let refresh_hash = hash_token(&refresh_token);
    let expires_in = state.session_ttl_seconds;

    if let Err(err) = state
        .store
        .create_session(
            user_id,
            &access_hash,
            &refresh_hash,
            Utc::now() + Duration::seconds(expires_in as i64),
        )
        .await
    {
        return store_error_response(err);
    }

    let mut metadata = HashMap::new();
    metadata.insert("action".to_string(), "session_created".to_string());

    if let Err(err) = state
        .store
        .add_audit_event(
            user_id,
            "AUTH_SESSION_CREATED",
            None,
            AuditResult::Success,
            &metadata,
        )
        .await
    {
        return store_error_response(err);
    }

    let response = CreateSessionResponse {
        access_token,
        refresh_token,
        expires_in: expires_in as u32,
    };

    (StatusCode::OK, Json(response)).into_response()
}

async fn register_device(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Json(req): Json<RegisterDeviceRequest>,
) -> impl IntoResponse {
    if let Err(err) = state
        .store
        .register_device(
            user.user_id,
            &req.device_id,
            &req.apns_token,
            &req.environment,
        )
        .await
    {
        return store_error_response(err);
    }

    let mut metadata = HashMap::new();
    metadata.insert("device_id".to_string(), req.device_id);

    if let Err(err) = state
        .store
        .add_audit_event(
            user.user_id,
            "DEVICE_REGISTERED",
            None,
            AuditResult::Success,
            &metadata,
        )
        .await
    {
        return store_error_response(err);
    }

    (StatusCode::OK, Json(OkResponse { ok: true })).into_response()
}

async fn start_google_connect(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Json(req): Json<StartGoogleConnectRequest>,
) -> impl IntoResponse {
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

async fn complete_google_connect(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Json(req): Json<CompleteGoogleConnectRequest>,
) -> impl IntoResponse {
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

    let token_response = match exchange_google_code(
        &state.http_client,
        &state.oauth,
        &req.code,
        &redirect_uri,
    )
    .await
    {
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
        .upsert_google_connector(user.user_id, &refresh_token, &granted_scopes)
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

async fn revoke_connector(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(connector_id): Path<String>,
) -> impl IntoResponse {
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

    match state
        .store
        .revoke_connector(user.user_id, connector_id)
        .await
    {
        Ok(true) => {
            let mut metadata = HashMap::new();
            metadata.insert("connector_id".to_string(), connector_id.to_string());

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

async fn get_preferences(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> impl IntoResponse {
    match state.store.get_or_create_preferences(user.user_id).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => store_error_response(err),
    }
}

async fn update_preferences(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Json(req): Json<Preferences>,
) -> impl IntoResponse {
    if let Err(err) = state.store.upsert_preferences(user.user_id, &req).await {
        return store_error_response(err);
    }

    let mut metadata = HashMap::new();
    metadata.insert(
        "meeting_reminder_minutes".to_string(),
        req.meeting_reminder_minutes.to_string(),
    );

    if let Err(err) = state
        .store
        .add_audit_event(
            user.user_id,
            "PREFERENCES_UPDATED",
            None,
            AuditResult::Success,
            &metadata,
        )
        .await
    {
        return store_error_response(err);
    }

    (StatusCode::OK, Json(OkResponse { ok: true })).into_response()
}

#[derive(serde::Deserialize)]
struct AuditEventsQuery {
    cursor: Option<String>,
}

async fn list_audit_events(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Query(query): Query<AuditEventsQuery>,
) -> impl IntoResponse {
    match state
        .store
        .list_audit_events(user.user_id, query.cursor.as_deref(), 50)
        .await
    {
        Ok((items, next_cursor)) => (
            StatusCode::OK,
            Json(ListAuditEventsResponse { items, next_cursor }),
        )
            .into_response(),
        Err(err) => store_error_response(err),
    }
}

async fn delete_all(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> impl IntoResponse {
    let request_id = match state.store.queue_delete_all(user.user_id).await {
        Ok(request_id) => request_id,
        Err(err) => return store_error_response(err),
    };

    let mut metadata = HashMap::new();
    metadata.insert("request_id".to_string(), request_id.to_string());

    if let Err(err) = state
        .store
        .add_audit_event(
            user.user_id,
            "PRIVACY_DELETE_ALL_REQUESTED",
            None,
            AuditResult::Success,
            &metadata,
        )
        .await
    {
        return store_error_response(err);
    }

    (
        StatusCode::OK,
        Json(DeleteAllResponse {
            request_id: request_id.to_string(),
            status: "QUEUED".to_string(),
        }),
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
struct GoogleTokenResponse {
    refresh_token: Option<String>,
    scope: Option<String>,
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
        warn!("oauth token exchange failed: status={status} body={body}");
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

fn hash_token(value: &str) -> Vec<u8> {
    let digest = Sha256::digest(value.as_bytes());
    digest.to_vec()
}

fn generate_secure_token(prefix: &str) -> String {
    format!(
        "{prefix}_{}_{}",
        Uuid::new_v4().as_simple(),
        Uuid::new_v4().as_simple()
    )
}

fn bad_request_response(code: &str, message: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: ErrorBody {
                code: code.to_string(),
                message: message.to_string(),
            },
        }),
    )
        .into_response()
}

fn bad_gateway_response(code: &str, message: &str) -> Response {
    (
        StatusCode::BAD_GATEWAY,
        Json(ErrorResponse {
            error: ErrorBody {
                code: code.to_string(),
                message: message.to_string(),
            },
        }),
    )
        .into_response()
}

fn unauthorized_response() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(ErrorResponse {
            error: ErrorBody {
                code: "unauthorized".to_string(),
                message: "Missing or invalid bearer token".to_string(),
            },
        }),
    )
        .into_response()
}

fn store_error_response(err: StoreError) -> Response {
    match err {
        StoreError::InvalidCursor => bad_request_response("invalid_cursor", "Cursor is invalid"),
        other => {
            error!("database operation failed: {other}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: ErrorBody {
                        code: "internal_error".to_string(),
                        message: "Unexpected server error".to_string(),
                    },
                }),
            )
                .into_response()
        }
    }
}
