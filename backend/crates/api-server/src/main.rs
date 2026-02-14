use std::net::SocketAddr;
use std::path::Path;

use shared::config::ApiConfig;
use shared::repos::Store;
use tracing::{error, info};

mod http;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "api_server=debug,axum=info,tower_http=info".to_string()),
        )
        .init();

    let config = match ApiConfig::from_env() {
        Ok(cfg) => cfg,
        Err(err) => {
            error!("failed to read config: {err}");
            std::process::exit(1);
        }
    };

    let store = match Store::connect(&config.database_url, config.database_max_connections).await {
        Ok(store) => store,
        Err(err) => {
            error!("failed to connect to postgres: {err}");
            std::process::exit(1);
        }
    };

    let migrator = match sqlx::migrate::Migrator::new(Path::new("../db/migrations")).await {
        Ok(migrator) => migrator,
        Err(err) => {
            error!("failed to load migrations: {err}");
            std::process::exit(1);
        }
    };

    if let Err(err) = migrator.run(store.pool()).await {
        error!("failed to run migrations: {err}");
        std::process::exit(1);
    }

    let app = http::build_router(http::AppState { store });

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
