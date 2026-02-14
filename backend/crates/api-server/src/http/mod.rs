use axum::routing::{delete, get, post};
use axum::{Router, middleware};
use shared::repos::Store;
use shared::security::SecretRuntime;
use uuid::Uuid;

mod audit;
mod authn;
mod connectors;
mod devices;
mod errors;
mod health;
mod preferences;
mod privacy;
mod session;
mod tokens;

#[derive(Clone)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub auth_url: String,
    pub token_url: String,
    pub revoke_url: String,
    pub scopes: Vec<String>,
}

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub oauth: OAuthConfig,
    pub secret_runtime: SecretRuntime,
    pub session_ttl_seconds: u64,
    pub oauth_state_ttl_seconds: u64,
    pub http_client: reqwest::Client,
}

#[derive(Clone, Copy)]
pub(super) struct AuthUser {
    pub(super) user_id: Uuid,
}

pub fn build_router(app_state: AppState) -> Router {
    let public_routes = Router::new()
        .route("/healthz", get(health::healthz))
        .route("/readyz", get(health::readyz))
        .route("/v1/auth/ios/session", post(session::create_session))
        .with_state(app_state.clone());

    let auth_layer_state = app_state.clone();

    let protected_routes = Router::new()
        .route("/v1/devices/apns", post(devices::register_device))
        .route(
            "/v1/devices/apns/test",
            post(devices::send_test_notification),
        )
        .route(
            "/v1/connectors/google/start",
            post(connectors::start_google_connect),
        )
        .route(
            "/v1/connectors/google/callback",
            post(connectors::complete_google_connect),
        )
        .route(
            "/v1/connectors/{connector_id}",
            delete(connectors::revoke_connector),
        )
        .route(
            "/v1/preferences",
            get(preferences::get_preferences).put(preferences::update_preferences),
        )
        .route("/v1/audit-events", get(audit::list_audit_events))
        .route("/v1/privacy/delete-all", post(privacy::delete_all))
        .route(
            "/v1/privacy/delete-all/{request_id}",
            get(privacy::get_delete_all_status),
        )
        .layer(middleware::from_fn_with_state(
            auth_layer_state,
            authn::auth_middleware,
        ))
        .with_state(app_state);

    public_routes.merge(protected_routes)
}
