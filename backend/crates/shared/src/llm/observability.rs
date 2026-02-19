use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use super::{
    AssistantCapability, LlmGateway, LlmGatewayError, LlmGatewayRequest, LlmGatewayResponse,
};

const PROVIDER_DEGRADATION_FAILURE_THRESHOLD: u32 = 5;
const PROVIDER_DEGRADATION_DURATION_THRESHOLD: Duration = Duration::from_secs(120);
const DEGRADATION_PROVIDER_KEY: &str = "openrouter";

#[derive(Debug, Clone, Copy)]
pub enum LlmExecutionSource {
    ApiAssistantQuery,
    WorkerMorningBrief,
    WorkerUrgentEmail,
}

impl LlmExecutionSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ApiAssistantQuery => "api_assistant_query",
            Self::WorkerMorningBrief => "worker_morning_brief",
            Self::WorkerUrgentEmail => "worker_urgent_email",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProviderDegradationAlert {
    pub consecutive_failures: u32,
    pub degraded_for_seconds: u64,
}

#[derive(Debug, Clone)]
pub struct LlmTelemetryEvent {
    pub source: &'static str,
    pub capability: &'static str,
    pub outcome: &'static str,
    pub latency_ms: u64,
    pub provider: String,
    pub degradation_provider: &'static str,
    pub model: Option<String>,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
    pub estimated_cost_usd: Option<f64>,
    pub error_type: Option<&'static str>,
    pub provider_degradation_alert: Option<ProviderDegradationAlert>,
    pub provider_recovered: bool,
}

pub async fn generate_with_telemetry(
    llm_gateway: &dyn LlmGateway,
    source: LlmExecutionSource,
    request: LlmGatewayRequest,
) -> (
    Result<LlmGatewayResponse, LlmGatewayError>,
    LlmTelemetryEvent,
) {
    let capability = request.capability;
    let started_at = Instant::now();
    let result = llm_gateway.generate(request).await;
    let telemetry = telemetry_for_result(source, capability, started_at.elapsed(), &result);
    (result, telemetry)
}

fn telemetry_for_result(
    source: LlmExecutionSource,
    capability: AssistantCapability,
    latency: Duration,
    result: &Result<LlmGatewayResponse, LlmGatewayError>,
) -> LlmTelemetryEvent {
    let latency_ms = duration_to_millis(latency);
    match result {
        Ok(response) => {
            let provider = provider_from_model(&response.model);
            let transition = update_provider_health(
                DEGRADATION_PROVIDER_KEY,
                true,
                Instant::now(),
                PROVIDER_DEGRADATION_FAILURE_THRESHOLD,
                PROVIDER_DEGRADATION_DURATION_THRESHOLD,
            );
            let usage = response.usage.clone().unwrap_or_default();
            let has_usage = response.usage.is_some();
            let estimated_cost_usd = if has_usage {
                estimate_cost_usd(
                    &response.model,
                    usage.prompt_tokens,
                    usage.completion_tokens,
                )
            } else {
                None
            };

            LlmTelemetryEvent {
                source: source.as_str(),
                capability: capability_label(capability),
                outcome: "success",
                latency_ms,
                provider,
                degradation_provider: DEGRADATION_PROVIDER_KEY,
                model: Some(response.model.clone()),
                prompt_tokens: has_usage.then_some(usage.prompt_tokens),
                completion_tokens: has_usage.then_some(usage.completion_tokens),
                total_tokens: has_usage.then_some(usage.total_tokens),
                estimated_cost_usd,
                error_type: None,
                provider_degradation_alert: transition.degradation_alert,
                provider_recovered: transition.recovered,
            }
        }
        Err(err) => {
            let provider = "openrouter".to_string();
            let transition = update_provider_health(
                DEGRADATION_PROVIDER_KEY,
                false,
                Instant::now(),
                PROVIDER_DEGRADATION_FAILURE_THRESHOLD,
                PROVIDER_DEGRADATION_DURATION_THRESHOLD,
            );

            LlmTelemetryEvent {
                source: source.as_str(),
                capability: capability_label(capability),
                outcome: "failure",
                latency_ms,
                provider,
                degradation_provider: DEGRADATION_PROVIDER_KEY,
                model: None,
                prompt_tokens: None,
                completion_tokens: None,
                total_tokens: None,
                estimated_cost_usd: None,
                error_type: Some(error_type(err)),
                provider_degradation_alert: transition.degradation_alert,
                provider_recovered: transition.recovered,
            }
        }
    }
}

fn duration_to_millis(duration: Duration) -> u64 {
    let millis = duration.as_millis();
    u64::try_from(millis).unwrap_or(u64::MAX)
}

fn capability_label(capability: AssistantCapability) -> &'static str {
    match capability {
        AssistantCapability::MeetingsSummary => "meetings_summary",
        AssistantCapability::GeneralChat => "general_chat",
        AssistantCapability::MorningBrief => "morning_brief",
        AssistantCapability::UrgentEmailSummary => "urgent_email_summary",
        AssistantCapability::AssistantSemanticPlan => "assistant_semantic_plan",
    }
}

fn provider_from_model(model: &str) -> String {
    model
        .split('/')
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("openrouter")
        .to_string()
}

fn error_type(error: &LlmGatewayError) -> &'static str {
    match error {
        LlmGatewayError::Timeout => "timeout",
        LlmGatewayError::ProviderFailure(_) => "provider_failure",
        LlmGatewayError::InvalidProviderPayload(_) => "invalid_provider_payload",
    }
}

fn estimate_cost_usd(model: &str, prompt_tokens: u32, completion_tokens: u32) -> Option<f64> {
    let pricing = pricing_for_model(model)?;
    let prompt = f64::from(prompt_tokens);
    let completion = f64::from(completion_tokens);
    let total = (prompt * pricing.input_per_million + completion * pricing.output_per_million)
        / 1_000_000.0;
    Some((total * 1_000_000.0).round() / 1_000_000.0)
}

#[derive(Debug, Clone, Copy)]
struct ModelPricing {
    input_per_million: f64,
    output_per_million: f64,
}

fn pricing_for_model(model: &str) -> Option<ModelPricing> {
    let normalized = model.trim().to_ascii_lowercase();
    if normalized.starts_with("openai/gpt-4o-mini") {
        return Some(ModelPricing {
            input_per_million: 0.15,
            output_per_million: 0.60,
        });
    }

    if normalized.starts_with("anthropic/claude-3.5-haiku") {
        return Some(ModelPricing {
            input_per_million: 0.80,
            output_per_million: 4.00,
        });
    }

    None
}

#[derive(Debug, Clone, Default)]
struct ProviderHealthState {
    consecutive_failures: u32,
    first_failure_at: Option<Instant>,
    alert_open: bool,
}

#[derive(Debug, Clone, Default)]
struct ProviderHealthTransition {
    degradation_alert: Option<ProviderDegradationAlert>,
    recovered: bool,
}

static PROVIDER_HEALTH: LazyLock<Mutex<HashMap<String, ProviderHealthState>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn update_provider_health(
    provider: &str,
    succeeded: bool,
    now: Instant,
    failure_threshold: u32,
    duration_threshold: Duration,
) -> ProviderHealthTransition {
    let mut tracker = lock_provider_health();
    let state = tracker.entry(provider.to_string()).or_default();

    if succeeded {
        let recovered = state.alert_open;
        tracker.remove(provider);
        return ProviderHealthTransition {
            degradation_alert: None,
            recovered,
        };
    }

    if state.consecutive_failures == 0 {
        state.first_failure_at = Some(now);
    }

    state.consecutive_failures = state.consecutive_failures.saturating_add(1);
    let degraded_for = state
        .first_failure_at
        .map(|started| now.saturating_duration_since(started))
        .unwrap_or_default();

    if !state.alert_open
        && state.consecutive_failures >= failure_threshold
        && degraded_for >= duration_threshold
    {
        state.alert_open = true;
        return ProviderHealthTransition {
            degradation_alert: Some(ProviderDegradationAlert {
                consecutive_failures: state.consecutive_failures,
                degraded_for_seconds: degraded_for.as_secs(),
            }),
            recovered: false,
        };
    }

    ProviderHealthTransition::default()
}

fn lock_provider_health() -> std::sync::MutexGuard<'static, HashMap<String, ProviderHealthState>> {
    match PROVIDER_HEALTH.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}
