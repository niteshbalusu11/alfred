use chrono::Utc;
use shared::config::WorkerConfig;
use tokio::signal;
use tokio::time::{self, Duration};
use tracing::{error, info};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "worker=debug".to_string()),
        )
        .init();

    let config = match WorkerConfig::from_env() {
        Ok(cfg) => cfg,
        Err(err) => {
            error!("failed to read worker config: {err}");
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
                process_due_jobs().await;
            }
        }
    }
}

async fn process_due_jobs() {
    info!(
        "worker tick at {}: placeholder run for due jobs",
        Utc::now().to_rfc3339()
    );
    // TODO: 1) fetch due jobs from Postgres
    // TODO: 2) invoke TEE-backed task runner
    // TODO: 3) write audit status + next_run_at
}
