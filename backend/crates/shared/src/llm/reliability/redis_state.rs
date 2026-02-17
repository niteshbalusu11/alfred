use std::time::Duration;

use crate::llm::LlmGatewayResponse;
use redis::AsyncCommands;
use redis::aio::ConnectionManager;
use sha2::{Digest, Sha256};

use super::LlmReliabilityConfig;
use super::state::RateLimitRejection;

const DEFAULT_RELIABILITY_KEY_PREFIX: &str = "alfred:llm:reliability:v1";
const CACHE_SCOPE: &str = "cache:data";
const RATE_LIMIT_SCOPE: &str = "rate_limit";
const CIRCUIT_BREAKER_SCOPE: &str = "circuit_breaker";
const BUDGET_SCOPE: &str = "budget";

#[derive(Clone)]
pub(crate) struct RedisReliabilityState {
    connection: ConnectionManager,
    key_prefix: String,
}

impl RedisReliabilityState {
    pub(crate) async fn new(redis_url: &str) -> Result<Self, String> {
        let client = redis::Client::open(redis_url).map_err(|err| err.to_string())?;
        let connection = ConnectionManager::new(client)
            .await
            .map_err(|err| err.to_string())?;

        let mut health_connection = connection.clone();
        redis::cmd("PING")
            .query_async::<String>(&mut health_connection)
            .await
            .map_err(|err| format!("failed to connect to redis: {err}"))?;

        Ok(Self {
            connection,
            key_prefix: DEFAULT_RELIABILITY_KEY_PREFIX.to_string(),
        })
    }

    pub(crate) async fn check_rate_limits(
        &self,
        requester_id: &str,
        config: &LlmReliabilityConfig,
    ) -> redis::RedisResult<Option<RateLimitRejection>> {
        let now_seconds = unix_timestamp_seconds();
        let window_seconds = i64::try_from(config.rate_limit_window_seconds).unwrap_or(i64::MAX);
        let window_start = fixed_window_start(now_seconds, window_seconds);
        let retry_after_seconds = retry_after_seconds(now_seconds, window_start, window_seconds);
        let ttl_seconds = expiry_ttl_seconds(config.rate_limit_window_seconds);

        let global_key = self.rate_limit_global_key(window_start);
        if self
            .increment_counter_and_check_limit(
                global_key,
                i64::from(config.rate_limit_global_max_requests),
                ttl_seconds,
            )
            .await?
        {
            return Ok(Some(RateLimitRejection {
                scope: "global",
                retry_after: Duration::from_secs(retry_after_seconds),
            }));
        }

        let user_key = self.rate_limit_user_key(requester_id, window_start);
        if self
            .increment_counter_and_check_limit(
                user_key,
                i64::from(config.rate_limit_per_user_max_requests),
                ttl_seconds,
            )
            .await?
        {
            return Ok(Some(RateLimitRejection {
                scope: "user",
                retry_after: Duration::from_secs(retry_after_seconds),
            }));
        }

        Ok(None)
    }

    pub(crate) async fn cached_response(
        &self,
        key: &str,
    ) -> redis::RedisResult<Option<LlmGatewayResponse>> {
        let cache_key = self.cache_data_key(key);
        let mut connection = self.connection.clone();
        let _: i64 = connection.del(cache_key).await?;
        Ok(None)
    }

    pub(crate) async fn store_cached_response(
        &self,
        key: &str,
        _response: &LlmGatewayResponse,
        _config: &LlmReliabilityConfig,
    ) -> redis::RedisResult<()> {
        let mut connection = self.connection.clone();
        let _: i64 = connection.del(self.cache_data_key(key)).await?;
        Ok(())
    }

    pub(crate) async fn circuit_breaker_retry_after(
        &self,
        config: &LlmReliabilityConfig,
    ) -> redis::RedisResult<Option<Duration>> {
        let key = self.circuit_breaker_open_key();
        let mut connection = self.connection.clone();
        let ttl_seconds: i64 = connection.ttl(&key).await?;

        if ttl_seconds > 0 {
            return Ok(Some(Duration::from_secs(
                u64::try_from(ttl_seconds).unwrap_or(1),
            )));
        }

        if ttl_seconds == -1 {
            let cooldown =
                i64::try_from(config.circuit_breaker_cooldown_seconds).unwrap_or(i64::MAX);
            let _: bool = connection.expire(&key, cooldown).await?;
            return Ok(Some(Duration::from_secs(
                config.circuit_breaker_cooldown_seconds.max(1),
            )));
        }

        Ok(None)
    }

    pub(crate) async fn should_use_budget_gateway(
        &self,
        config: &LlmReliabilityConfig,
    ) -> redis::RedisResult<bool> {
        let now_seconds = unix_timestamp_seconds();
        let window_seconds = i64::try_from(config.budget_window_seconds).unwrap_or(i64::MAX);
        let window_start = fixed_window_start(now_seconds, window_seconds);

        let mut connection = self.connection.clone();
        let spent_micros: Option<i64> =
            connection.get(self.budget_window_key(window_start)).await?;
        Ok(spent_micros.unwrap_or(0) >= budget_limit_micros(config))
    }

    pub(crate) async fn record_provider_success(&self) -> redis::RedisResult<()> {
        let mut connection = self.connection.clone();
        let _: i64 = connection.del(self.circuit_breaker_failures_key()).await?;
        let _: i64 = connection.del(self.circuit_breaker_open_key()).await?;
        Ok(())
    }

    pub(crate) async fn record_provider_failure(
        &self,
        config: &LlmReliabilityConfig,
    ) -> redis::RedisResult<()> {
        let failure_key = self.circuit_breaker_failures_key();
        let mut connection = self.connection.clone();
        let failure_count: i64 = connection.incr(&failure_key, 1_i64).await?;
        let failure_ttl = expiry_ttl_seconds(config.circuit_breaker_cooldown_seconds);
        let _: bool = connection
            .expire(&failure_key, i64::try_from(failure_ttl).unwrap_or(i64::MAX))
            .await?;

        if failure_count >= i64::from(config.circuit_breaker_failure_threshold) {
            connection
                .set_ex::<_, _, ()>(
                    self.circuit_breaker_open_key(),
                    "1",
                    config.circuit_breaker_cooldown_seconds.max(1),
                )
                .await?;
        }

        Ok(())
    }

    pub(crate) async fn record_budget_spend(
        &self,
        estimated_cost_usd: f64,
        config: &LlmReliabilityConfig,
    ) -> redis::RedisResult<()> {
        let budget_delta_micros = usd_to_micros(estimated_cost_usd);
        if budget_delta_micros <= 0 {
            return Ok(());
        }

        let now_seconds = unix_timestamp_seconds();
        let window_seconds = i64::try_from(config.budget_window_seconds).unwrap_or(i64::MAX);
        let window_start = fixed_window_start(now_seconds, window_seconds);
        let key = self.budget_window_key(window_start);

        let mut connection = self.connection.clone();
        let _: i64 = connection.incr(&key, budget_delta_micros).await?;
        let budget_ttl = expiry_ttl_seconds(config.budget_window_seconds);
        let _: bool = connection
            .expire(&key, i64::try_from(budget_ttl).unwrap_or(i64::MAX))
            .await?;
        Ok(())
    }

    async fn increment_counter_and_check_limit(
        &self,
        key: String,
        max_allowed: i64,
        ttl_seconds: u64,
    ) -> redis::RedisResult<bool> {
        let mut connection = self.connection.clone();
        let count: i64 = connection.incr(&key, 1_i64).await?;
        let _: bool = connection
            .expire(&key, i64::try_from(ttl_seconds).unwrap_or(i64::MAX))
            .await?;
        Ok(count > max_allowed)
    }

    fn cache_data_key(&self, cache_key: &str) -> String {
        self.compose_key(CACHE_SCOPE, cache_key)
    }

    fn rate_limit_global_key(&self, window_start: i64) -> String {
        self.compose_key(RATE_LIMIT_SCOPE, &format!("global:{window_start}"))
    }

    fn rate_limit_user_key(&self, requester_id: &str, window_start: i64) -> String {
        let requester_hash = hashed_label(requester_id);
        self.compose_key(
            RATE_LIMIT_SCOPE,
            &format!("user:{requester_hash}:{window_start}"),
        )
    }

    fn circuit_breaker_failures_key(&self) -> String {
        self.compose_key(CIRCUIT_BREAKER_SCOPE, "failures")
    }

    fn circuit_breaker_open_key(&self) -> String {
        self.compose_key(CIRCUIT_BREAKER_SCOPE, "open")
    }

    fn budget_window_key(&self, window_start: i64) -> String {
        self.compose_key(BUDGET_SCOPE, &window_start.to_string())
    }

    fn compose_key(&self, scope: &str, suffix: &str) -> String {
        format!("{}:{scope}:{suffix}", self.key_prefix)
    }
}

fn unix_timestamp_seconds() -> i64 {
    chrono::Utc::now().timestamp()
}

fn fixed_window_start(now_seconds: i64, window_seconds: i64) -> i64 {
    if window_seconds <= 0 {
        return now_seconds;
    }

    now_seconds - now_seconds.rem_euclid(window_seconds)
}

fn retry_after_seconds(now_seconds: i64, window_start: i64, window_seconds: i64) -> u64 {
    let retry_after = (window_start + window_seconds).saturating_sub(now_seconds);
    if retry_after <= 0 {
        1
    } else {
        u64::try_from(retry_after).unwrap_or(1)
    }
}

fn expiry_ttl_seconds(window_seconds: u64) -> u64 {
    window_seconds.saturating_mul(2).max(1)
}

fn budget_limit_micros(config: &LlmReliabilityConfig) -> i64 {
    usd_to_micros(config.budget_max_estimated_cost_usd).max(1)
}

fn usd_to_micros(usd: f64) -> i64 {
    if !usd.is_finite() || usd <= 0.0 {
        return 0;
    }

    let micros = (usd * 1_000_000.0).round();
    if micros >= i64::MAX as f64 {
        i64::MAX
    } else {
        micros as i64
    }
}

fn hashed_label(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::{fixed_window_start, retry_after_seconds, usd_to_micros};

    #[test]
    fn fixed_window_start_aligns_timestamp_to_window_boundary() {
        assert_eq!(fixed_window_start(125, 60), 120);
        assert_eq!(fixed_window_start(60, 60), 60);
        assert_eq!(fixed_window_start(59, 60), 0);
    }

    #[test]
    fn retry_after_seconds_is_at_least_one_second() {
        assert_eq!(retry_after_seconds(120, 120, 60), 60);
        assert_eq!(retry_after_seconds(180, 120, 60), 1);
    }

    #[test]
    fn usd_to_micros_rounds_to_integer_micro_units() {
        assert_eq!(usd_to_micros(0.000_001), 1);
        assert_eq!(usd_to_micros(1.5), 1_500_000);
        assert_eq!(usd_to_micros(0.0), 0);
        assert_eq!(usd_to_micros(-1.0), 0);
    }
}
