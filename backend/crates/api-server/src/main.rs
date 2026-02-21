use std::net::SocketAddr;
use std::time::Duration;

use shared::config::{ApiConfig, load_dotenv};
use shared::enclave::EnclaveRpcAuthConfig;
use shared::enclave_runtime::{
    AlfredEnvironment, EnclaveRuntimeEndpointConfig, verify_connectivity,
};
use shared::repos::Store;
use shared::security::{KmsDecryptPolicy, SecretRuntime, TeeAttestationPolicy};
use tracing::{error, info};

use api_server::http;

#[tokio::main]
async fn main() {
    if let Err(err) = load_dotenv() {
        eprintln!("{err}");
        std::process::exit(1);
    }

    init_tracing();

    let config = match ApiConfig::from_env() {
        Ok(cfg) => cfg,
        Err(err) => {
            error!(error = %err, "failed to read config");
            std::process::exit(1);
        }
    };

    let store = match Store::connect(
        &config.database_url,
        config.database_max_connections,
        &config.data_encryption_key,
    )
    .await
    {
        Ok(store) => store,
        Err(err) => {
            error!(error = %err, "failed to connect to postgres");
            std::process::exit(1);
        }
    };

    let migrator = match sqlx::migrate::Migrator::new(config.migrations_dir.clone()).await {
        Ok(migrator) => migrator,
        Err(err) => {
            error!(error = %err, "failed to load migrations");
            std::process::exit(1);
        }
    };

    if let Err(err) = migrator.run(store.pool()).await {
        error!(error = %err, "failed to run migrations");
        std::process::exit(1);
    }

    let rate_limiter = http::RateLimiter::default();
    let _rate_limiter_pruner = rate_limiter.spawn_pruner(Duration::from_secs(60));
    let clerk_jwks_cache = match http::ClerkJwksCache::new(http::ClerkJwksCacheConfig {
        redis_url: config.redis_url.clone(),
        cache_key: config.clerk_jwks_cache_key.clone(),
        default_ttl_seconds: config.clerk_jwks_cache_default_ttl_seconds,
        stale_ttl_seconds: config.clerk_jwks_cache_stale_ttl_seconds,
    })
    .await
    {
        Ok(cache) => cache,
        Err(err) => {
            error!(error = %err, "failed to initialize Clerk JWKS redis cache");
            std::process::exit(1);
        }
    };
    let http_client = match reqwest::Client::builder()
        .timeout(Duration::from_millis(config.api_http_timeout_ms))
        .build()
    {
        Ok(http_client) => http_client,
        Err(err) => {
            error!(error = %err, "failed to initialize api http client");
            std::process::exit(1);
        }
    };
    let enclave_runtime_config = EnclaveRuntimeEndpointConfig {
        mode: config.enclave_runtime_mode,
        base_url: config.enclave_runtime_base_url.clone(),
        probe_timeout_ms: config.enclave_runtime_probe_timeout_ms,
    };
    if let Err(err) = verify_connectivity(&http_client, &enclave_runtime_config).await {
        error!(error = %err, "failed enclave runtime startup connectivity check");
        std::process::exit(1);
    }
    info!(
        enclave_runtime_mode = enclave_runtime_config.mode.as_str(),
        enclave_runtime_base_url = %enclave_runtime_config.base_url,
        "enclave runtime connectivity verified"
    );

    let app = http::build_router(http::AppState {
        store,
        oauth: http::OAuthConfig {
            client_id: config.google_client_id,
            redirect_uri: config.google_redirect_uri,
            auth_url: config.google_auth_url,
            scopes: vec![
                "https://www.googleapis.com/auth/gmail.readonly".to_string(),
                "https://www.googleapis.com/auth/calendar.readonly".to_string(),
            ],
        },
        enclave_rpc: http::EnclaveRpcConfig {
            base_url: config.enclave_runtime_base_url.clone(),
            auth: EnclaveRpcAuthConfig {
                shared_secret: config.enclave_rpc_shared_secret.clone(),
                max_clock_skew_seconds: config.enclave_rpc_auth_max_skew_seconds,
            },
        },
        allow_debug_automation_run: matches!(config.alfred_environment, AlfredEnvironment::Local),
        secret_runtime: SecretRuntime::new(
            TeeAttestationPolicy {
                required: config.tee_attestation_required,
                expected_runtime: config.tee_expected_runtime,
                allowed_measurements: config.tee_allowed_measurements,
                attestation_public_key: config.tee_attestation_public_key,
                max_attestation_age_seconds: config.tee_attestation_max_age_seconds,
                allow_insecure_dev_attestation: config.tee_allow_insecure_dev_attestation,
            },
            KmsDecryptPolicy {
                key_id: config.kms_key_id,
                key_version: config.kms_key_version,
                allowed_measurements: config.kms_allowed_measurements,
            },
            config.enclave_runtime_base_url.clone(),
            config.tee_attestation_challenge_timeout_ms,
            http_client.clone(),
        ),
        rate_limiter,
        trusted_proxy_ips: config.trusted_proxy_ips.into_iter().collect(),
        oauth_state_ttl_seconds: config.oauth_state_ttl_seconds,
        clerk_issuer: config.clerk_issuer,
        clerk_audience: config.clerk_audience,
        clerk_secret_key: config.clerk_secret_key,
        clerk_jwks_url: config.clerk_jwks_url,
        clerk_jwks_cache,
        http_client,
    });

    let addr: SocketAddr = config
        .bind_addr
        .parse()
        .unwrap_or_else(|_| "127.0.0.1:8080".parse().expect("valid default bind addr"));

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind should succeed");

    info!(bind_addr = %listener.local_addr().unwrap_or(addr), "api server listening");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("server should run");
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "api_server=debug,axum=info,tower_http=info".to_string()),
        )
        .json()
        .flatten_event(true)
        .with_current_span(true)
        .init();
}
