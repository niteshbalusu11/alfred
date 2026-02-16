use std::collections::VecDeque;
use std::sync::Arc;

use serde_json::json;
use shared::llm::gateway::{LlmGatewayFuture, LlmTokenUsage};
use shared::llm::reliability::ReliableLlmGateway;
use shared::llm::{
    AssistantCapability, LlmGateway, LlmGatewayError, LlmGatewayRequest, LlmGatewayResponse,
    LlmReliabilityConfig, template_for_capability,
};
use tokio::sync::Mutex;

#[derive(Clone)]
struct StubGateway {
    responses: Arc<Mutex<VecDeque<Result<LlmGatewayResponse, LlmGatewayError>>>>,
    seen_requesters: Arc<Mutex<Vec<String>>>,
}

impl StubGateway {
    fn with_responses(responses: Vec<Result<LlmGatewayResponse, LlmGatewayError>>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(VecDeque::from(responses))),
            seen_requesters: Arc::new(Mutex::new(Vec::new())),
        }
    }

    async fn calls(&self) -> usize {
        self.seen_requesters.lock().await.len()
    }

    async fn seen_requesters(&self) -> Vec<String> {
        self.seen_requesters.lock().await.clone()
    }
}

impl LlmGateway for StubGateway {
    fn generate<'a>(&'a self, request: LlmGatewayRequest) -> LlmGatewayFuture<'a> {
        Box::pin(async move {
            self.seen_requesters.lock().await.push(
                request
                    .requester_id
                    .unwrap_or_else(|| "anonymous".to_string()),
            );

            self.responses.lock().await.pop_front().unwrap_or_else(|| {
                Err(LlmGatewayError::ProviderFailure(
                    "missing_stub_response".to_string(),
                ))
            })
        })
    }
}

#[tokio::test]
async fn enforces_per_user_rate_limit() {
    let primary = StubGateway::with_responses(vec![
        Ok(success_response("openai/gpt-4o-mini", 5, 5)),
        Ok(success_response("openai/gpt-4o-mini", 5, 5)),
    ]);
    let mut config = base_config();
    config.rate_limit_per_user_max_requests = 1;
    config.rate_limit_global_max_requests = 10;

    let gateway =
        ReliableLlmGateway::new(primary.clone(), None, config).expect("gateway should build");

    gateway
        .generate(request_for("user-a", "first"))
        .await
        .expect("first user request should pass");
    let err = gateway
        .generate(request_for("user-a", "second"))
        .await
        .expect_err("second request should be rate limited");
    assert!(
        matches!(err, LlmGatewayError::ProviderFailure(message) if message.contains("rate_limited scope=user"))
    );

    gateway
        .generate(request_for("user-b", "first"))
        .await
        .expect("different user should not be blocked by per-user limit");
}

#[tokio::test]
async fn enforces_global_rate_limit() {
    let primary = StubGateway::with_responses(vec![
        Ok(success_response("openai/gpt-4o-mini", 5, 5)),
        Ok(success_response("openai/gpt-4o-mini", 5, 5)),
    ]);
    let mut config = base_config();
    config.rate_limit_global_max_requests = 2;
    config.rate_limit_per_user_max_requests = 10;

    let gateway =
        ReliableLlmGateway::new(primary.clone(), None, config).expect("gateway should build");

    gateway
        .generate(request_for("user-a", "first"))
        .await
        .expect("first request should pass");
    gateway
        .generate(request_for("user-b", "second"))
        .await
        .expect("second request should pass");
    let err = gateway
        .generate(request_for("user-c", "third"))
        .await
        .expect_err("third request should be globally rate limited");
    assert!(
        matches!(err, LlmGatewayError::ProviderFailure(message) if message.contains("rate_limited scope=global"))
    );
}

#[tokio::test]
async fn opens_circuit_breaker_after_consecutive_failures() {
    let primary = StubGateway::with_responses(vec![
        Err(LlmGatewayError::Timeout),
        Err(LlmGatewayError::ProviderFailure(
            "provider_down".to_string(),
        )),
        Ok(success_response("openai/gpt-4o-mini", 5, 5)),
    ]);
    let mut config = base_config();
    config.circuit_breaker_failure_threshold = 2;
    config.circuit_breaker_cooldown_seconds = 120;

    let gateway =
        ReliableLlmGateway::new(primary.clone(), None, config).expect("gateway should build");

    let _ = gateway.generate(request_for("user-a", "first")).await;
    let _ = gateway.generate(request_for("user-a", "second")).await;
    let err = gateway
        .generate(request_for("user-a", "third"))
        .await
        .expect_err("third request should be blocked by circuit breaker");
    assert!(
        matches!(err, LlmGatewayError::ProviderFailure(message) if message.contains("circuit_breaker_open"))
    );
    assert_eq!(
        primary.calls().await,
        2,
        "breaker should reject before calling provider"
    );
}

#[tokio::test]
async fn returns_cached_response_without_hitting_provider() {
    let primary =
        StubGateway::with_responses(vec![Ok(success_response("openai/gpt-4o-mini", 5, 5))]);
    let mut config = base_config();
    config.cache_ttl_seconds = 300;

    let gateway =
        ReliableLlmGateway::new(primary.clone(), None, config).expect("gateway should build");

    let first = gateway
        .generate(request_for("user-a", "identical"))
        .await
        .expect("first request should pass");
    let second = gateway
        .generate(request_for("user-a", "identical"))
        .await
        .expect("second request should be served from cache");

    assert_eq!(first.output, second.output);
    assert_eq!(primary.calls().await, 1, "provider should be called once");
}

#[tokio::test]
async fn switches_to_budget_gateway_when_budget_threshold_exceeded() {
    let primary = StubGateway::with_responses(vec![Ok(success_response(
        "anthropic/claude-3.5-haiku",
        1_000_000,
        0,
    ))]);
    let budget =
        StubGateway::with_responses(vec![Ok(success_response("openai/gpt-4o-mini", 10, 10))]);

    let mut config = base_config();
    config.budget_max_estimated_cost_usd = 0.5;
    config.budget_window_seconds = 3_600;

    let gateway = ReliableLlmGateway::new(primary.clone(), Some(budget.clone()), config)
        .expect("gateway should build");

    gateway
        .generate(request_for("user-a", "first"))
        .await
        .expect("first request should use primary");
    gateway
        .generate(request_for("user-a", "second"))
        .await
        .expect("second request should route to budget gateway");

    assert_eq!(primary.calls().await, 1);
    assert_eq!(budget.calls().await, 1);
    assert_eq!(
        budget.seen_requesters().await,
        vec!["user-a".to_string()],
        "budget request should preserve user identity"
    );
}

fn request_for(requester_id: &str, marker: &str) -> LlmGatewayRequest {
    LlmGatewayRequest::from_template(
        template_for_capability(AssistantCapability::MeetingsSummary),
        json!({
            "calendar_day": "2026-02-16",
            "meetings": [{"title": marker}]
        }),
    )
    .with_requester_id(requester_id)
}

fn success_response(model: &str, prompt_tokens: u32, completion_tokens: u32) -> LlmGatewayResponse {
    LlmGatewayResponse {
        model: model.to_string(),
        provider_request_id: Some("req-id".to_string()),
        output: json!({
            "version": "2026-02-15",
            "output": {
                "title": "Daily meetings",
                "summary": "You have one meeting",
                "key_points": ["One meeting"],
                "follow_ups": []
            }
        }),
        usage: Some(LlmTokenUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens.saturating_add(completion_tokens),
        }),
    }
}

fn base_config() -> LlmReliabilityConfig {
    LlmReliabilityConfig {
        rate_limit_window_seconds: 60,
        rate_limit_global_max_requests: 50,
        rate_limit_per_user_max_requests: 50,
        circuit_breaker_failure_threshold: 5,
        circuit_breaker_cooldown_seconds: 60,
        cache_ttl_seconds: 60,
        cache_max_entries: 128,
        budget_window_seconds: 3_600,
        budget_max_estimated_cost_usd: 5.0,
        budget_model: Some("openai/gpt-4o-mini".to_string()),
    }
}
