use axum::routing::{delete, get, post};
use axum::{Router, middleware};
use shared::enclave::EnclaveRpcAuthConfig;
use shared::repos::Store;
use shared::security::SecretRuntime;
use std::collections::HashSet;
use std::net::IpAddr;
use uuid::Uuid;

mod assistant;
mod audit;
mod authn;
mod clerk_identity;
mod clerk_jwks_cache;
mod connectors;
mod devices;
mod errors;
mod health;
mod oauth_bridge;
mod observability;
mod preferences;
mod privacy;
mod rate_limit;
mod tokens;
pub(crate) use clerk_jwks_cache::{ClerkJwksCache, ClerkJwksCacheConfig};
pub use rate_limit::RateLimiter;

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
pub struct EnclaveRpcConfig {
    pub base_url: String,
    pub auth: EnclaveRpcAuthConfig,
}

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub oauth: OAuthConfig,
    pub enclave_rpc: EnclaveRpcConfig,
    pub secret_runtime: SecretRuntime,
    pub rate_limiter: RateLimiter,
    pub trusted_proxy_ips: HashSet<IpAddr>,
    pub oauth_state_ttl_seconds: u64,
    pub clerk_issuer: String,
    pub clerk_audience: String,
    pub clerk_secret_key: String,
    pub clerk_jwks_url: String,
    pub clerk_jwks_cache: ClerkJwksCache,
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
        .route(
            "/oauth/google/callback",
            get(oauth_bridge::redirect_google_oauth_callback),
        )
        .with_state(app_state.clone());

    let auth_layer_state = app_state.clone();
    let protected_rate_limit_layer_state = app_state.clone();

    let protected_routes = Router::new()
        .route("/v1/devices/apns", post(devices::register_device))
        .route(
            "/v1/devices/apns/test",
            post(devices::send_test_notification),
        )
        .route(
            "/v1/assistant/query",
            post(assistant::query_assistant).layer(middleware::from_fn_with_state(
                protected_rate_limit_layer_state.clone(),
                rate_limit::sensitive_rate_limit_middleware,
            )),
        )
        .route(
            "/v1/assistant/attested-key",
            post(assistant::fetch_attested_key).layer(middleware::from_fn_with_state(
                protected_rate_limit_layer_state.clone(),
                rate_limit::sensitive_rate_limit_middleware,
            )),
        )
        .route(
            "/v1/connectors/google/start",
            post(connectors::start_google_connect).layer(middleware::from_fn_with_state(
                protected_rate_limit_layer_state.clone(),
                rate_limit::sensitive_rate_limit_middleware,
            )),
        )
        .route(
            "/v1/connectors/google/callback",
            post(connectors::complete_google_connect).layer(middleware::from_fn_with_state(
                protected_rate_limit_layer_state.clone(),
                rate_limit::sensitive_rate_limit_middleware,
            )),
        )
        .route(
            "/v1/connectors/{connector_id}",
            delete(connectors::revoke_connector).layer(middleware::from_fn_with_state(
                protected_rate_limit_layer_state.clone(),
                rate_limit::sensitive_rate_limit_middleware,
            )),
        )
        .route(
            "/v1/preferences",
            get(preferences::get_preferences).put(preferences::update_preferences),
        )
        .route("/v1/audit-events", get(audit::list_audit_events))
        .route(
            "/v1/privacy/delete-all",
            post(privacy::delete_all).layer(middleware::from_fn_with_state(
                protected_rate_limit_layer_state,
                rate_limit::sensitive_rate_limit_middleware,
            )),
        )
        .route(
            "/v1/privacy/delete-all/{request_id}",
            get(privacy::get_delete_all_status),
        )
        .layer(middleware::from_fn_with_state(
            auth_layer_state,
            authn::auth_middleware,
        ))
        .with_state(app_state);

    public_routes
        .merge(protected_routes)
        .layer(middleware::from_fn(
            observability::request_observability_middleware,
        ))
}
