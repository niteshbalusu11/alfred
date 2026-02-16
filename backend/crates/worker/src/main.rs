use shared::config::{WorkerConfig, load_dotenv};
use shared::enclave_runtime::{EnclaveRuntimeEndpointConfig, verify_connectivity};
use shared::llm::{LlmReliabilityConfig, OpenRouterGatewayConfig, ReliableOpenRouterGateway};
use shared::repos::Store;
use shared::security::{KmsDecryptPolicy, SecretRuntime, TeeAttestationPolicy};
use tokio::signal;
use tokio::time::{self, Duration};
use tracing::{error, info};
use uuid::Uuid;

mod job_actions;
mod job_processing;
mod privacy_delete;
mod privacy_delete_revoke;
mod push_sender;
mod retry;
mod types;

use job_processing::process_due_jobs;
pub(crate) use push_sender::{
    NotificationContent, PushSendError, PushSender, apns_environment_label,
};
pub(crate) use retry::retry_delay_seconds;
pub(crate) use types::{FailureClass, JobExecutionError, WorkerTickMetrics};

#[tokio::main]
async fn main() {
    if let Err(err) = load_dotenv() {
        eprintln!("{err}");
        std::process::exit(1);
    }

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

    let openrouter_config = match OpenRouterGatewayConfig::from_env() {
        Ok(cfg) => cfg,
        Err(err) => {
            error!("failed to read OpenRouter configuration required for LLM startup path: {err}");
            std::process::exit(1);
        }
    };
    let llm_reliability_config = match LlmReliabilityConfig::from_env() {
        Ok(cfg) => cfg,
        Err(err) => {
            error!("failed to read LLM reliability configuration: {err}");
            std::process::exit(1);
        }
    };
    let llm_gateway = match ReliableOpenRouterGateway::from_openrouter_config_with_redis(
        openrouter_config,
        llm_reliability_config,
        &config.redis_url,
    )
    .await
    {
        Ok(gateway) => gateway,
        Err(err) => {
            error!("failed to initialize LLM gateway: {err}");
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

    let push_sender = PushSender::new(
        config.apns_sandbox_endpoint.clone(),
        config.apns_production_endpoint.clone(),
        config.apns_auth_token.clone(),
    );
    let oauth_client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
    {
        Ok(client) => client,
        Err(err) => {
            error!("failed to initialize worker http client: {err}");
            std::process::exit(1);
        }
    };
    let enclave_runtime_config = EnclaveRuntimeEndpointConfig {
        mode: config.enclave_runtime_mode,
        base_url: config.enclave_runtime_base_url.clone(),
        probe_timeout_ms: config.enclave_runtime_probe_timeout_ms,
    };
    if let Err(err) = verify_connectivity(&oauth_client, &enclave_runtime_config).await {
        error!(error = %err, "failed enclave runtime startup connectivity check");
        std::process::exit(1);
    }
    info!(
        enclave_runtime_mode = enclave_runtime_config.mode.as_str(),
        enclave_runtime_base_url = %enclave_runtime_config.base_url,
        "enclave runtime connectivity verified"
    );
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
        config.tee_attestation_document.clone(),
        config.tee_attestation_document_path.clone(),
    );

    let worker_id = Uuid::new_v4();
    info!(
        worker_id = %worker_id,
        tick_seconds = config.tick_seconds,
        batch_size = config.batch_size,
        lease_seconds = config.lease_seconds,
        per_user_concurrency_limit = config.per_user_concurrency_limit,
        apns_sandbox_endpoint_configured = config.apns_sandbox_endpoint.is_some(),
        apns_production_endpoint_configured = config.apns_production_endpoint.is_some(),
        "worker starting"
    );

    let mut ticker = time::interval(Duration::from_secs(config.tick_seconds));

    loop {
        tokio::select! {
            _ = signal::ctrl_c() => {
                info!(worker_id = %worker_id, "shutdown signal received");
                break;
            }
            _ = ticker.tick() => {
                privacy_delete::process_delete_requests(
                    &store,
                    &config,
                    &secret_runtime,
                    &oauth_client,
                    worker_id,
                ).await;
                process_due_jobs(
                    &store,
                    &config,
                    &secret_runtime,
                    &oauth_client,
                    &llm_gateway,
                    &push_sender,
                    worker_id,
                )
                .await;
            }
        }
    }
}
