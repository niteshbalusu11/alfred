use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::routing::{get, post};
use shared::config::load_dotenv;
use shared::enclave::EnclaveOperationService;
use shared::repos::Store;
use shared::security::{KmsDecryptPolicy, SecretRuntime, TeeAttestationPolicy};
use tracing::{error, info, warn};

mod config;
mod http;

#[derive(Clone)]
struct RuntimeState {
    config: config::RuntimeConfig,
    enclave_service: EnclaveOperationService,
    rpc_replay_guard: Arc<Mutex<std::collections::HashMap<String, i64>>>,
}

#[tokio::main]
async fn main() {
    if let Err(err) = load_dotenv() {
        eprintln!("{err}");
        std::process::exit(1);
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "enclave_runtime=info,axum=info".to_string()),
        )
        .json()
        .flatten_event(true)
        .with_current_span(true)
        .init();

    let config = match config::RuntimeConfig::from_env() {
        Ok(config) => config,
        Err(err) => {
            error!(error = %err, "failed to load enclave runtime config");
            std::process::exit(1);
        }
    };

    if matches!(
        config.mode,
        shared::enclave_runtime::EnclaveRuntimeMode::DevShim
    ) {
        warn!(
            "starting enclave runtime in dev-shim mode; do not use this mode in staging/production"
        );
    }

    let http_client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
    {
        Ok(http_client) => http_client,
        Err(err) => {
            error!(error = %err, "failed to initialize enclave runtime http client");
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
    let secret_runtime = SecretRuntime::new(
        TeeAttestationPolicy {
            required: config.tee_attestation_required,
            expected_runtime: config.tee_expected_runtime.clone(),
            allowed_measurements: config.tee_allowed_measurements.clone(),
            attestation_public_key: config.tee_attestation_public_key.clone(),
            max_attestation_age_seconds: config.tee_attestation_max_age_seconds,
            allow_insecure_dev_attestation: config.tee_allow_insecure_dev_attestation,
        },
        KmsDecryptPolicy {
            key_id: config.kms_key_id.clone(),
            key_version: config.kms_key_version,
            allowed_measurements: config.kms_allowed_measurements.clone(),
        },
        config.enclave_runtime_base_url.clone(),
        config.tee_attestation_challenge_timeout_ms,
        http_client.clone(),
    );
    let enclave_service =
        EnclaveOperationService::new(store, secret_runtime, http_client, config.oauth.clone());

    let app = Router::new()
        .route("/healthz", get(http::healthz))
        .route("/v1/attestation/document", get(http::attestation_document))
        .route(
            "/v1/attestation/challenge",
            post(http::attestation_challenge),
        )
        .route(
            "/v1/rpc/google/token/exchange",
            post(http::exchange_google_access_token),
        )
        .route(
            "/v1/rpc/google/token/revoke",
            post(http::revoke_google_connector_token),
        )
        .with_state(RuntimeState {
            config: config.clone(),
            enclave_service,
            rpc_replay_guard: Arc::new(Mutex::new(std::collections::HashMap::new())),
        });

    let addr: SocketAddr = match config.bind_addr.parse() {
        Ok(addr) => addr,
        Err(err) => {
            error!(error = %err, bind_addr = %config.bind_addr, "invalid bind addr");
            std::process::exit(1);
        }
    };

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(err) => {
            error!(error = %err, bind_addr = %addr, "failed to bind enclave runtime listener");
            std::process::exit(1);
        }
    };

    info!(
        bind_addr = %listener.local_addr().unwrap_or(addr),
        environment = config.environment.as_str(),
        mode = config.mode.as_str(),
        "enclave runtime listening"
    );

    if let Err(err) = axum::serve(listener, app.into_make_service()).await {
        error!(error = %err, "enclave runtime failed");
        std::process::exit(1);
    }
}
