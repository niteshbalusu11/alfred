use std::time::Duration;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::llm::{AssistantCapability, LlmGatewayRequest, LlmGatewayResponse};

pub(crate) fn estimate_cost_usd(response: &LlmGatewayResponse) -> Option<f64> {
    let usage = response.usage.as_ref()?;
    let pricing = pricing_for_model(&response.model)?;
    let prompt = f64::from(usage.prompt_tokens);
    let completion = f64::from(usage.completion_tokens);
    Some(
        (prompt * pricing.input_per_million + completion * pricing.output_per_million)
            / 1_000_000.0,
    )
}

pub(crate) fn duration_to_retry_after_seconds(duration: Duration) -> u64 {
    let seconds = duration.as_secs();
    if seconds == 0 {
        return 1;
    }
    if duration.subsec_nanos() > 0 {
        seconds.saturating_add(1)
    } else {
        seconds
    }
}

pub(crate) fn cache_key(request: &LlmGatewayRequest) -> String {
    let payload = CacheKeyPayload {
        requester_id: request.requester_id.as_deref(),
        capability: capability_label(request.capability),
        contract_version: &request.contract_version,
        system_prompt: &request.system_prompt,
        context_prompt: &request.context_prompt,
        output_schema: &request.output_schema,
        context_payload: &request.context_payload,
    };
    let serialized = serde_json::to_vec(&payload).unwrap_or_default();
    let digest = Sha256::digest(serialized);
    digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
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

#[derive(Serialize)]
struct CacheKeyPayload<'a> {
    requester_id: Option<&'a str>,
    capability: &'static str,
    contract_version: &'a str,
    system_prompt: &'a str,
    context_prompt: &'a str,
    output_schema: &'a serde_json::Value,
    context_payload: &'a serde_json::Value,
}

fn capability_label(capability: AssistantCapability) -> &'static str {
    match capability {
        AssistantCapability::MeetingsSummary => "meetings_summary",
        AssistantCapability::MorningBrief => "morning_brief",
        AssistantCapability::UrgentEmailSummary => "urgent_email_summary",
        AssistantCapability::AssistantSemanticPlan => "assistant_semantic_plan",
    }
}
