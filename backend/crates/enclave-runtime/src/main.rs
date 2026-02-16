use std::net::SocketAddr;

use axum::Router;
use axum::routing::get;
use shared::config::load_dotenv;
use tracing::{error, info, warn};

mod config;
mod http;

#[derive(Clone)]
struct RuntimeState {
    config: config::RuntimeConfig,
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

    let app = Router::new()
        .route("/healthz", get(http::healthz))
        .route("/v1/attestation/document", get(http::attestation_document))
        .with_state(RuntimeState {
            config: config.clone(),
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
