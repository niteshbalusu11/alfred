use std::collections::HashMap;

use axum::extract::{Extension, Path, Query, Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use chrono::Utc;
use shared::models::{
    CompleteGoogleConnectRequest, CompleteGoogleConnectResponse, ConnectorStatus,
    CreateSessionRequest, CreateSessionResponse, DeleteAllResponse, ErrorBody, ErrorResponse,
    ListAuditEventsResponse, OkResponse, Preferences, RegisterDeviceRequest,
    RevokeConnectorResponse, StartGoogleConnectRequest, StartGoogleConnectResponse,
};
use shared::repos::{AuditResult, JobType, Store, StoreError};
use tracing::{error, warn};
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
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
        .layer(middleware::from_fn(auth_middleware))
        .with_state(app_state);

    public_routes.merge(protected_routes)
}

async fn auth_middleware(mut req: Request, next: Next) -> Response {
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

    let user_id = match Uuid::parse_str(token) {
        Ok(user_id) => user_id,
        Err(_) => {
            warn!("invalid bearer token format");
            return unauthorized_response();
        }
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
        access_token: user_id.to_string(),
        refresh_token: format!("alfred_refresh_{}", Uuid::new_v4()),
        expires_in: 3600,
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
    let state_token = Uuid::new_v4().to_string();
    let response = StartGoogleConnectResponse {
        auth_url: format!(
            "https://accounts.google.com/o/oauth2/v2/auth?client_id=replace-me&redirect_uri={}&state={}",
            req.redirect_uri, state_token
        ),
        state: state_token.clone(),
    };

    let mut metadata = HashMap::new();
    metadata.insert("redirect_uri".to_string(), req.redirect_uri);
    metadata.insert("state".to_string(), state_token);

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
    let granted_scopes = vec![
        "gmail.readonly".to_string(),
        "calendar.readonly".to_string(),
    ];

    let connector_id = match state
        .store
        .upsert_google_connector(user.user_id, req.code.as_bytes(), &granted_scopes)
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
        StoreError::InvalidCursor => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: ErrorBody {
                    code: "invalid_cursor".to_string(),
                    message: "Cursor is invalid".to_string(),
                },
            }),
        )
            .into_response(),
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
