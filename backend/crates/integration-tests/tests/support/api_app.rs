#![allow(dead_code)]

use std::collections::HashSet;
use std::net::IpAddr;
use std::time::Duration;

use api_server::http::{
    AppState, ClerkJwksCache, ClerkJwksCacheConfig, EnclaveRpcConfig, OAuthConfig, RateLimiter,
    build_router,
};
use shared::repos::Store;
use shared::security::{KmsDecryptPolicy, SecretRuntime, TeeAttestationPolicy};
use uuid::Uuid;

use super::clerk::TestClerkAuth;
use super::test_redis_url;

const OAUTH_REDIRECT_URI: &str = "alfred://oauth/google/callback";
const CLERK_SUBJECT_NAMESPACE: Uuid = Uuid::from_u128(0x10850be7d81f4f4ea2dc0bb96943a09e);
const DEFAULT_ENCLAVE_RPC_BASE_URL: &str = "http://127.0.0.1:65530";

pub async fn build_test_router(store: Store, clerk: &TestClerkAuth) -> axum::Router {
    build_test_router_with_enclave_base_url(store, clerk, DEFAULT_ENCLAVE_RPC_BASE_URL).await
}

pub async fn build_test_router_with_enclave_base_url(
    store: Store,
    clerk: &TestClerkAuth,
    enclave_rpc_base_url: &str,
) -> axum::Router {
    let clerk_jwks_cache = build_clerk_jwks_cache().await;
    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("http client should initialize");

    let state = AppState {
        store,
        oauth: OAuthConfig {
            client_id: "test-google-client".to_string(),
            redirect_uri: OAUTH_REDIRECT_URI.to_string(),
            auth_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
            scopes: vec![
                "https://www.googleapis.com/auth/gmail.readonly".to_string(),
                "https://www.googleapis.com/auth/calendar.readonly".to_string(),
            ],
        },
        enclave_rpc: EnclaveRpcConfig {
            base_url: enclave_rpc_base_url.to_string(),
            auth: shared::enclave::EnclaveRpcAuthConfig {
                shared_secret: "integration-test-secret".to_string(),
                max_clock_skew_seconds: 30,
            },
        },
        allow_debug_automation_run: true,
        secret_runtime: SecretRuntime::new(
            TeeAttestationPolicy {
                required: false,
                expected_runtime: "nitro".to_string(),
                allowed_measurements: vec!["dev-local-enclave".to_string()],
                attestation_public_key: None,
                max_attestation_age_seconds: 300,
                allow_insecure_dev_attestation: true,
            },
            KmsDecryptPolicy {
                key_id: "kms/local/alfred-refresh-token".to_string(),
                key_version: 1,
                allowed_measurements: vec!["dev-local-enclave".to_string()],
            },
            enclave_rpc_base_url.to_string(),
            2000,
            http_client.clone(),
        ),
        rate_limiter: RateLimiter::default(),
        trusted_proxy_ips: HashSet::<IpAddr>::new(),
        oauth_state_ttl_seconds: 300,
        clerk_issuer: clerk.issuer.clone(),
        clerk_audience: clerk.audience.clone(),
        clerk_secret_key: "test-clerk-secret".to_string(),
        clerk_jwks_url: clerk.jwks_url.clone(),
        clerk_jwks_cache,
        http_client,
    };

    build_router(state)
}

pub fn oauth_redirect_uri() -> &'static str {
    OAUTH_REDIRECT_URI
}

pub fn user_id_for_subject(issuer: &str, subject: &str) -> Uuid {
    let stable_subject = format!("{}:{subject}", issuer.trim_end_matches('/'));
    Uuid::new_v5(&CLERK_SUBJECT_NAMESPACE, stable_subject.as_bytes())
}

async fn build_clerk_jwks_cache() -> ClerkJwksCache {
    ClerkJwksCache::new(ClerkJwksCacheConfig {
        redis_url: test_redis_url(),
        cache_key: format!("integration-tests:clerk-jwks:{}", Uuid::new_v4()),
        default_ttl_seconds: 300,
        stale_ttl_seconds: 300,
    })
    .await
    .expect("clerk jwks cache should initialize")
}
