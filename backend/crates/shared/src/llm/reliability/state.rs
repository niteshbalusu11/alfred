use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use crate::llm::LlmGatewayResponse;

use super::LlmReliabilityConfig;

#[derive(Debug, Clone)]
struct WindowCounter {
    started_at: Instant,
    count: u32,
}

impl Default for WindowCounter {
    fn default() -> Self {
        Self {
            started_at: Instant::now(),
            count: 0,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct CircuitBreakerState {
    consecutive_failures: u32,
    open_until: Option<Instant>,
}

#[derive(Debug, Clone)]
struct CachedResponse {
    response: LlmGatewayResponse,
    expires_at: Instant,
}

#[derive(Debug, Clone)]
struct BudgetWindow {
    started_at: Instant,
    spent_usd: f64,
}

impl Default for BudgetWindow {
    fn default() -> Self {
        Self {
            started_at: Instant::now(),
            spent_usd: 0.0,
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct ReliabilityState {
    global_counter: WindowCounter,
    per_user_counter: HashMap<String, WindowCounter>,
    circuit_breaker: CircuitBreakerState,
    cache: HashMap<String, CachedResponse>,
    cache_order: VecDeque<String>,
    budget_window: BudgetWindow,
}

#[derive(Debug, Clone)]
pub(crate) struct RateLimitRejection {
    pub(crate) scope: &'static str,
    pub(crate) retry_after: Duration,
}

impl ReliabilityState {
    pub(crate) fn check_rate_limits(
        &mut self,
        requester_id: &str,
        now: Instant,
        config: &LlmReliabilityConfig,
    ) -> Option<RateLimitRejection> {
        let window = config.rate_limit_window();
        self.prune_stale_user_windows(now, window);

        if let Some(retry_after) = increment_window_counter(
            &mut self.global_counter,
            now,
            window,
            config.rate_limit_global_max_requests,
        ) {
            return Some(RateLimitRejection {
                scope: "global",
                retry_after,
            });
        }

        let user_counter = self
            .per_user_counter
            .entry(requester_id.to_string())
            .or_default();
        increment_window_counter(
            user_counter,
            now,
            window,
            config.rate_limit_per_user_max_requests,
        )
        .map(|retry_after| RateLimitRejection {
            scope: "user",
            retry_after,
        })
    }

    pub(crate) fn cached_response(
        &mut self,
        key: &str,
        now: Instant,
    ) -> Option<LlmGatewayResponse> {
        match self.cache.get(key) {
            Some(entry) if now < entry.expires_at => Some(entry.response.clone()),
            Some(_) => {
                self.cache.remove(key);
                self.drop_cache_order_key(key);
                None
            }
            None => None,
        }
    }

    pub(crate) fn circuit_breaker_retry_after(&mut self, now: Instant) -> Option<Duration> {
        let open_until = self.circuit_breaker.open_until?;
        if now >= open_until {
            self.circuit_breaker.open_until = None;
            self.circuit_breaker.consecutive_failures = 0;
            return None;
        }
        Some(open_until.saturating_duration_since(now))
    }

    pub(crate) fn should_use_budget_gateway(
        &mut self,
        now: Instant,
        config: &LlmReliabilityConfig,
    ) -> bool {
        self.roll_budget_window_if_needed(now, config);
        self.budget_window.spent_usd >= config.budget_max_estimated_cost_usd
    }

    pub(crate) fn record_provider_success(&mut self) {
        self.circuit_breaker.consecutive_failures = 0;
        self.circuit_breaker.open_until = None;
    }

    pub(crate) fn record_provider_failure(&mut self, now: Instant, config: &LlmReliabilityConfig) {
        self.circuit_breaker.consecutive_failures =
            self.circuit_breaker.consecutive_failures.saturating_add(1);
        if self.circuit_breaker.consecutive_failures >= config.circuit_breaker_failure_threshold {
            self.circuit_breaker.open_until = Some(now + config.circuit_breaker_cooldown());
        }
    }

    pub(crate) fn record_budget_spend(
        &mut self,
        now: Instant,
        config: &LlmReliabilityConfig,
        estimated_cost_usd: f64,
    ) {
        self.roll_budget_window_if_needed(now, config);
        if estimated_cost_usd > 0.0 {
            self.budget_window.spent_usd += estimated_cost_usd;
        }
    }

    pub(crate) fn store_cached_response(
        &mut self,
        key: String,
        response: LlmGatewayResponse,
        now: Instant,
        config: &LlmReliabilityConfig,
    ) {
        self.prune_expired_cache(now);
        self.drop_cache_order_key(&key);
        self.cache.insert(
            key.clone(),
            CachedResponse {
                response,
                expires_at: now + config.cache_ttl(),
            },
        );
        self.cache_order.push_back(key);

        while self.cache.len() > config.cache_max_entries {
            let Some(oldest_key) = self.cache_order.pop_front() else {
                break;
            };
            self.cache.remove(&oldest_key);
        }
    }

    fn prune_stale_user_windows(&mut self, now: Instant, window: Duration) {
        let stale_after = window.saturating_add(window);
        self.per_user_counter
            .retain(|_, counter| now.saturating_duration_since(counter.started_at) <= stale_after);
    }

    fn prune_expired_cache(&mut self, now: Instant) {
        self.cache.retain(|_, entry| now < entry.expires_at);
        self.cache_order
            .retain(|cache_key| self.cache.contains_key(cache_key));
    }

    fn roll_budget_window_if_needed(&mut self, now: Instant, config: &LlmReliabilityConfig) {
        let window = config.budget_window();
        if now.saturating_duration_since(self.budget_window.started_at) >= window {
            self.budget_window.started_at = now;
            self.budget_window.spent_usd = 0.0;
        }
    }

    fn drop_cache_order_key(&mut self, key: &str) {
        self.cache_order.retain(|cache_key| cache_key != key);
    }
}

fn increment_window_counter(
    counter: &mut WindowCounter,
    now: Instant,
    window: Duration,
    max_requests: u32,
) -> Option<Duration> {
    if now.saturating_duration_since(counter.started_at) >= window {
        counter.started_at = now;
        counter.count = 0;
    }

    if counter.count >= max_requests {
        let elapsed = now.saturating_duration_since(counter.started_at);
        return Some(window.saturating_sub(elapsed));
    }

    counter.count = counter.count.saturating_add(1);
    None
}
