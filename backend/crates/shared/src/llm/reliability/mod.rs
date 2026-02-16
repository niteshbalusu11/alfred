use std::sync::{Arc, Mutex};
use std::time::Instant;

use thiserror::Error;
use tracing::warn;

use super::gateway::{LlmGateway, LlmGatewayError, LlmGatewayFuture, LlmGatewayRequest};
use super::openrouter::{
    OpenRouterConfigError, OpenRouterGateway, OpenRouterGatewayConfig, OpenRouterModelRoute,
};
use config::DEFAULT_BUDGET_MODEL;
use redis_state::RedisReliabilityState;
use state::{RateLimitRejection, ReliabilityState};
use util::{cache_key, duration_to_retry_after_seconds, estimate_cost_usd};

mod config;
mod redis_state;
mod state;
mod util;

pub use config::{LlmReliabilityConfig, LlmReliabilityConfigError};

#[derive(Debug, Error)]
pub enum ReliableGatewayBuildError {
    #[error(transparent)]
    ReliabilityConfig(#[from] LlmReliabilityConfigError),
    #[error(transparent)]
    OpenRouterConfig(#[from] OpenRouterConfigError),
    #[error("failed to initialize redis reliability state: {0}")]
    RedisInitialization(String),
}

pub type ReliableOpenRouterGateway = ReliableLlmGateway<OpenRouterGateway>;

#[derive(Clone)]
enum ReliabilityStateBackend {
    InMemory(Arc<Mutex<ReliabilityState>>),
    Redis(RedisReliabilityState),
}

#[derive(Clone)]
pub struct ReliableLlmGateway<G>
where
    G: LlmGateway + Clone + Send + Sync + 'static,
{
    primary_gateway: G,
    budget_gateway: Option<G>,
    config: LlmReliabilityConfig,
    state_backend: ReliabilityStateBackend,
}

impl<G> ReliableLlmGateway<G>
where
    G: LlmGateway + Clone + Send + Sync + 'static,
{
    pub fn new(
        primary_gateway: G,
        budget_gateway: Option<G>,
        config: LlmReliabilityConfig,
    ) -> Result<Self, LlmReliabilityConfigError> {
        config.validate()?;
        Ok(Self {
            primary_gateway,
            budget_gateway,
            config,
            state_backend: ReliabilityStateBackend::InMemory(Arc::new(Mutex::new(
                ReliabilityState::default(),
            ))),
        })
    }

    fn lock_state(
        state: &Arc<Mutex<ReliabilityState>>,
    ) -> std::sync::MutexGuard<'_, ReliabilityState> {
        match state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    async fn check_rate_limits(&self, requester_id: &str) -> Option<RateLimitRejection> {
        match &self.state_backend {
            ReliabilityStateBackend::InMemory(state) => {
                let mut guard = Self::lock_state(state);
                guard.check_rate_limits(requester_id, Instant::now(), &self.config)
            }
            ReliabilityStateBackend::Redis(state) => {
                match state.check_rate_limits(requester_id, &self.config).await {
                    Ok(rejection) => rejection,
                    Err(err) => {
                        warn!(error = %err, "redis reliability rate limit lookup failed");
                        None
                    }
                }
            }
        }
    }

    async fn cached_response(&self, key: &str) -> Option<crate::llm::LlmGatewayResponse> {
        match &self.state_backend {
            ReliabilityStateBackend::InMemory(state) => {
                let mut guard = Self::lock_state(state);
                guard.cached_response(key, Instant::now())
            }
            ReliabilityStateBackend::Redis(state) => match state.cached_response(key).await {
                Ok(response) => response,
                Err(err) => {
                    warn!(error = %err, "redis reliability cache lookup failed");
                    None
                }
            },
        }
    }

    async fn circuit_breaker_retry_after(&self) -> Option<std::time::Duration> {
        match &self.state_backend {
            ReliabilityStateBackend::InMemory(state) => {
                let mut guard = Self::lock_state(state);
                guard.circuit_breaker_retry_after(Instant::now())
            }
            ReliabilityStateBackend::Redis(state) => {
                match state.circuit_breaker_retry_after(&self.config).await {
                    Ok(retry_after) => retry_after,
                    Err(err) => {
                        warn!(error = %err, "redis reliability circuit-breaker lookup failed");
                        None
                    }
                }
            }
        }
    }

    async fn should_use_budget_gateway(&self) -> bool {
        match &self.state_backend {
            ReliabilityStateBackend::InMemory(state) => {
                let mut guard = Self::lock_state(state);
                guard.should_use_budget_gateway(Instant::now(), &self.config)
            }
            ReliabilityStateBackend::Redis(state) => {
                match state.should_use_budget_gateway(&self.config).await {
                    Ok(should_use_budget_gateway) => should_use_budget_gateway,
                    Err(err) => {
                        warn!(error = %err, "redis reliability budget lookup failed");
                        false
                    }
                }
            }
        }
    }

    async fn record_provider_success(&self) {
        match &self.state_backend {
            ReliabilityStateBackend::InMemory(state) => {
                let mut guard = Self::lock_state(state);
                guard.record_provider_success();
            }
            ReliabilityStateBackend::Redis(state) => {
                if let Err(err) = state.record_provider_success().await {
                    warn!(error = %err, "redis reliability provider success update failed");
                }
            }
        }
    }

    async fn record_provider_failure(&self) {
        match &self.state_backend {
            ReliabilityStateBackend::InMemory(state) => {
                let mut guard = Self::lock_state(state);
                guard.record_provider_failure(Instant::now(), &self.config);
            }
            ReliabilityStateBackend::Redis(state) => {
                if let Err(err) = state.record_provider_failure(&self.config).await {
                    warn!(error = %err, "redis reliability provider failure update failed");
                }
            }
        }
    }

    async fn record_budget_spend(&self, estimated_cost_usd: f64) {
        match &self.state_backend {
            ReliabilityStateBackend::InMemory(state) => {
                let mut guard = Self::lock_state(state);
                guard.record_budget_spend(Instant::now(), &self.config, estimated_cost_usd);
            }
            ReliabilityStateBackend::Redis(state) => {
                if let Err(err) = state
                    .record_budget_spend(estimated_cost_usd, &self.config)
                    .await
                {
                    warn!(error = %err, "redis reliability budget update failed");
                }
            }
        }
    }

    async fn store_cached_response(
        &self,
        cache_key: &str,
        response: &crate::llm::LlmGatewayResponse,
    ) {
        match &self.state_backend {
            ReliabilityStateBackend::InMemory(state) => {
                let mut guard = Self::lock_state(state);
                guard.store_cached_response(
                    cache_key.to_string(),
                    response.clone(),
                    Instant::now(),
                    &self.config,
                );
            }
            ReliabilityStateBackend::Redis(state) => {
                if let Err(err) = state
                    .store_cached_response(cache_key, response, &self.config)
                    .await
                {
                    warn!(error = %err, "redis reliability cache write failed");
                }
            }
        }
    }
}

impl ReliableOpenRouterGateway {
    pub fn from_openrouter_config(
        openrouter_config: OpenRouterGatewayConfig,
        reliability_config: LlmReliabilityConfig,
    ) -> Result<Self, ReliableGatewayBuildError> {
        let (primary_gateway, budget_gateway) =
            build_openrouter_gateways(openrouter_config, &reliability_config)?;

        Ok(Self {
            primary_gateway,
            budget_gateway,
            config: reliability_config,
            state_backend: ReliabilityStateBackend::InMemory(Arc::new(Mutex::new(
                ReliabilityState::default(),
            ))),
        })
    }

    pub async fn from_openrouter_config_with_redis(
        openrouter_config: OpenRouterGatewayConfig,
        reliability_config: LlmReliabilityConfig,
        redis_url: &str,
    ) -> Result<Self, ReliableGatewayBuildError> {
        let (primary_gateway, budget_gateway) =
            build_openrouter_gateways(openrouter_config, &reliability_config)?;
        let redis_state = RedisReliabilityState::new(redis_url)
            .await
            .map_err(ReliableGatewayBuildError::RedisInitialization)?;

        Ok(Self {
            primary_gateway,
            budget_gateway,
            config: reliability_config,
            state_backend: ReliabilityStateBackend::Redis(redis_state),
        })
    }
}

impl<G> LlmGateway for ReliableLlmGateway<G>
where
    G: LlmGateway + Clone + Send + Sync + 'static,
{
    fn generate<'a>(&'a self, request: LlmGatewayRequest) -> LlmGatewayFuture<'a> {
        Box::pin(async move {
            let request_cache_key = cache_key(&request);
            let requester_id = request
                .requester_id
                .clone()
                .unwrap_or_else(|| "anonymous".to_string());

            if let Some(rejection) = self.check_rate_limits(&requester_id).await {
                return Err(LlmGatewayError::ProviderFailure(format!(
                    "rate_limited scope={} retry_after_seconds={}",
                    rejection.scope,
                    duration_to_retry_after_seconds(rejection.retry_after)
                )));
            }

            if let Some(cached_response) = self.cached_response(&request_cache_key).await {
                return Ok(cached_response);
            }

            if let Some(retry_after) = self.circuit_breaker_retry_after().await {
                return Err(LlmGatewayError::ProviderFailure(format!(
                    "circuit_breaker_open retry_after_seconds={}",
                    duration_to_retry_after_seconds(retry_after)
                )));
            }

            let selected_gateway = if self.should_use_budget_gateway().await {
                self.budget_gateway
                    .as_ref()
                    .unwrap_or(&self.primary_gateway)
            } else {
                &self.primary_gateway
            };
            let result = selected_gateway.generate(request).await;

            match &result {
                Ok(response) => {
                    self.record_provider_success().await;
                    self.record_budget_spend(estimate_cost_usd(response).unwrap_or(0.0))
                        .await;
                    self.store_cached_response(&request_cache_key, response)
                        .await;
                }
                Err(_) => {
                    self.record_provider_failure().await;
                }
            }

            result
        })
    }
}

fn build_openrouter_gateways(
    openrouter_config: OpenRouterGatewayConfig,
    reliability_config: &LlmReliabilityConfig,
) -> Result<(OpenRouterGateway, Option<OpenRouterGateway>), ReliableGatewayBuildError> {
    reliability_config.validate()?;
    let primary_gateway = OpenRouterGateway::new(openrouter_config.clone())?;

    let budget_model = reliability_config
        .budget_model
        .clone()
        .unwrap_or_else(|| DEFAULT_BUDGET_MODEL.to_string());
    let mut budget_config = openrouter_config.clone();
    budget_config.model_route = OpenRouterModelRoute {
        primary_model: budget_model,
        fallback_model: None,
    };

    let budget_gateway = if budget_config.model_route.primary_model
        == openrouter_config.model_route.primary_model
        && openrouter_config.model_route.fallback_model.is_none()
    {
        None
    } else {
        Some(OpenRouterGateway::new(budget_config)?)
    };

    Ok((primary_gateway, budget_gateway))
}
