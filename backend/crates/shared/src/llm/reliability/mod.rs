use std::sync::{Arc, Mutex};
use std::time::Instant;

use thiserror::Error;

use super::gateway::{LlmGateway, LlmGatewayError, LlmGatewayFuture, LlmGatewayRequest};
use super::openrouter::{
    OpenRouterConfigError, OpenRouterGateway, OpenRouterGatewayConfig, OpenRouterModelRoute,
};
use config::DEFAULT_BUDGET_MODEL;
use state::ReliabilityState;
use util::{cache_key, duration_to_retry_after_seconds, estimate_cost_usd};

mod config;
mod state;
mod util;

pub use config::{LlmReliabilityConfig, LlmReliabilityConfigError};

#[derive(Debug, Error)]
pub enum ReliableGatewayBuildError {
    #[error(transparent)]
    ReliabilityConfig(#[from] LlmReliabilityConfigError),
    #[error(transparent)]
    OpenRouterConfig(#[from] OpenRouterConfigError),
}

pub type ReliableOpenRouterGateway = ReliableLlmGateway<OpenRouterGateway>;

#[derive(Clone)]
pub struct ReliableLlmGateway<G>
where
    G: LlmGateway + Clone + Send + Sync + 'static,
{
    primary_gateway: G,
    budget_gateway: Option<G>,
    config: LlmReliabilityConfig,
    state: Arc<Mutex<ReliabilityState>>,
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
            state: Arc::new(Mutex::new(ReliabilityState::default())),
        })
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, ReliabilityState> {
        match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

impl ReliableOpenRouterGateway {
    pub fn from_openrouter_config(
        openrouter_config: OpenRouterGatewayConfig,
        reliability_config: LlmReliabilityConfig,
    ) -> Result<Self, ReliableGatewayBuildError> {
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

        Ok(Self {
            primary_gateway,
            budget_gateway,
            config: reliability_config,
            state: Arc::new(Mutex::new(ReliabilityState::default())),
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
            let now = Instant::now();

            {
                let mut state = self.lock_state();
                if let Some(rejection) = state.check_rate_limits(&requester_id, now, &self.config) {
                    return Err(LlmGatewayError::ProviderFailure(format!(
                        "rate_limited scope={} retry_after_seconds={}",
                        rejection.scope,
                        duration_to_retry_after_seconds(rejection.retry_after)
                    )));
                }
                if let Some(cached_response) = state.cached_response(&request_cache_key, now) {
                    return Ok(cached_response);
                }
                if let Some(retry_after) = state.circuit_breaker_retry_after(now) {
                    return Err(LlmGatewayError::ProviderFailure(format!(
                        "circuit_breaker_open retry_after_seconds={}",
                        duration_to_retry_after_seconds(retry_after)
                    )));
                }
            }

            let use_budget_gateway = {
                let mut state = self.lock_state();
                state.should_use_budget_gateway(now, &self.config)
            };
            let selected_gateway = if use_budget_gateway {
                self.budget_gateway
                    .as_ref()
                    .unwrap_or(&self.primary_gateway)
            } else {
                &self.primary_gateway
            };
            let result = selected_gateway.generate(request).await;

            let completed_at = Instant::now();
            let mut state = self.lock_state();
            match &result {
                Ok(response) => {
                    state.record_provider_success();
                    state.record_budget_spend(
                        completed_at,
                        &self.config,
                        estimate_cost_usd(response).unwrap_or(0.0),
                    );
                    state.store_cached_response(
                        request_cache_key,
                        response.clone(),
                        completed_at,
                        &self.config,
                    );
                }
                Err(_) => {
                    state.record_provider_failure(completed_at, &self.config);
                }
            }

            result
        })
    }
}
