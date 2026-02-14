use chrono::Utc;
use shared::config::WorkerConfig;
use shared::repos::Store;
use tokio::signal;
use tokio::time::{self, Duration};
use tracing::{error, info};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "worker=debug".to_string()))
        .init();

    let config = match WorkerConfig::from_env() {
        Ok(cfg) => cfg,
        Err(err) => {
            error!("failed to read worker config: {err}");
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
            error!("failed to connect to postgres: {err}");
            std::process::exit(1);
        }
    };

    info!(
        "worker starting (tick every {} seconds)",
        config.tick_seconds
    );

    let mut ticker = time::interval(Duration::from_secs(config.tick_seconds));

    loop {
        tokio::select! {
            _ = signal::ctrl_c() => {
                info!("shutdown signal received");
                break;
            }
            _ = ticker.tick() => {
                process_due_jobs(&store).await;
            }
        }
    }
}

async fn process_due_jobs(store: &Store) {
    let now = Utc::now();

    match store.count_due_jobs(now).await {
        Ok(count) => info!(
            "worker tick at {}: {} pending due jobs",
            now.to_rfc3339(),
            count
        ),
        Err(err) => error!("failed to count due jobs: {err}"),
    }
}
