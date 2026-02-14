use std::collections::HashMap;
use std::net::SocketAddr;

use axum::extract::{Path, Query, Request};
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use chrono::Utc;
use shared::config::ApiConfig;
use shared::models::{
    AuditEvent, CompleteGoogleConnectRequest, CompleteGoogleConnectResponse, ConnectorStatus,
    CreateSessionRequest, CreateSessionResponse, DeleteAllResponse, ErrorBody, ErrorResponse,
    ListAuditEventsResponse, OkResponse, Preferences, RegisterDeviceRequest,
    RevokeConnectorResponse, StartGoogleConnectRequest, StartGoogleConnectResponse,
};
use tracing::{info, warn};
use uuid::Uuid;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "api_server=debug,axum=info,tower_http=info".to_string()),
        )
        .init();

    let config = ApiConfig::from_env();

    let public_routes = Router::new().route("/v1/auth/ios/session", post(create_session));

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
        .layer(middleware::from_fn(auth_middleware));

    let app = public_routes.merge(protected_routes);

    let addr: SocketAddr = config
        .bind_addr
        .parse()
        .unwrap_or_else(|_| "127.0.0.1:8080".parse().expect("valid default bind addr"));

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind should succeed");

    info!(
        "api server listening on {}",
        listener.local_addr().unwrap_or(addr)
    );
    axum::serve(listener, app).await.expect("server should run");
}

async fn auth_middleware(req: Request, next: Next) -> Response {
    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();

    if !auth_header.starts_with("Bearer ") || auth_header == "Bearer " {
        warn!("missing or invalid authorization header");
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: ErrorBody {
                    code: "unauthorized".to_string(),
                    message: "Missing or invalid bearer token".to_string(),
                },
            }),
        )
            .into_response();
    }

    next.run(req).await
}

async fn create_session(Json(_req): Json<CreateSessionRequest>) -> impl IntoResponse {
    let response = CreateSessionResponse {
        access_token: format!("alfred_access_{}", Uuid::new_v4()),
        refresh_token: format!("alfred_refresh_{}", Uuid::new_v4()),
        expires_in: 3600,
    };
    (StatusCode::OK, Json(response))
}

async fn register_device(Json(_req): Json<RegisterDeviceRequest>) -> impl IntoResponse {
    (StatusCode::OK, Json(OkResponse { ok: true }))
}

async fn start_google_connect(Json(_req): Json<StartGoogleConnectRequest>) -> impl IntoResponse {
    let response = StartGoogleConnectResponse {
        auth_url: "https://accounts.google.com/o/oauth2/v2/auth?client_id=replace-me".to_string(),
        state: Uuid::new_v4().to_string(),
    };
    (StatusCode::OK, Json(response))
}

async fn complete_google_connect(
    Json(_req): Json<CompleteGoogleConnectRequest>,
) -> impl IntoResponse {
    let response = CompleteGoogleConnectResponse {
        connector_id: format!("con_{}", Uuid::new_v4()),
        status: ConnectorStatus::Active,
        granted_scopes: vec![
            "gmail.readonly".to_string(),
            "calendar.readonly".to_string(),
        ],
    };
    (StatusCode::OK, Json(response))
}

async fn revoke_connector(Path(connector_id): Path<String>) -> impl IntoResponse {
    let _ = connector_id;
    (
        StatusCode::OK,
        Json(RevokeConnectorResponse {
            status: ConnectorStatus::Revoked,
        }),
    )
}

async fn get_preferences() -> impl IntoResponse {
    let response = Preferences {
        meeting_reminder_minutes: 15,
        morning_brief_local_time: "08:00".to_string(),
        quiet_hours_start: "22:00".to_string(),
        quiet_hours_end: "07:00".to_string(),
        high_risk_requires_confirm: true,
    };
    (StatusCode::OK, Json(response))
}

async fn update_preferences(Json(_req): Json<Preferences>) -> impl IntoResponse {
    (StatusCode::OK, Json(OkResponse { ok: true }))
}

#[derive(serde::Deserialize)]
struct AuditEventsQuery {
    cursor: Option<String>,
}

async fn list_audit_events(Query(query): Query<AuditEventsQuery>) -> impl IntoResponse {
    let mut metadata = HashMap::new();
    metadata.insert("source".to_string(), "calendar".to_string());
    if let Some(cursor) = query.cursor {
        metadata.insert("cursor".to_string(), cursor);
    }

    let response = ListAuditEventsResponse {
        items: vec![AuditEvent {
            id: format!("ae_{}", Uuid::new_v4()),
            timestamp: Utc::now(),
            event_type: "CONNECTOR_FETCH".to_string(),
            connector: Some("google".to_string()),
            result: "SUCCESS".to_string(),
            metadata,
        }],
        next_cursor: None,
    };

    (StatusCode::OK, Json(response))
}

async fn delete_all() -> impl IntoResponse {
    let response = DeleteAllResponse {
        request_id: format!("del_{}", Uuid::new_v4()),
        status: "QUEUED".to_string(),
    };
    (StatusCode::OK, Json(response))
}

fn _json_content_type() -> HeaderValue {
    HeaderValue::from_static("application/json")
}
