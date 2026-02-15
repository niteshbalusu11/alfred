use std::future::Future;
use std::pin::Pin;

use serde_json::Value;
use thiserror::Error;

use super::contracts::AssistantCapability;
use super::prompts::PromptTemplate;

pub type LlmGatewayFuture<'a> =
    Pin<Box<dyn Future<Output = Result<LlmGatewayResponse, LlmGatewayError>> + Send + 'a>>;

#[derive(Debug, Clone)]
pub struct LlmGatewayRequest {
    pub capability: AssistantCapability,
    pub contract_version: String,
    pub system_prompt: String,
    pub context_prompt: String,
    pub output_schema: Value,
    pub context_payload: Value,
}

impl LlmGatewayRequest {
    pub fn from_template(template: PromptTemplate, context_payload: Value) -> Self {
        Self {
            capability: template.capability,
            contract_version: template.contract_version.to_string(),
            system_prompt: template.system_prompt.to_string(),
            context_prompt: template.context_prompt.to_string(),
            output_schema: template.output_schema,
            context_payload,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LlmTokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone)]
pub struct LlmGatewayResponse {
    pub model: String,
    pub provider_request_id: Option<String>,
    pub output: Value,
    pub usage: Option<LlmTokenUsage>,
}

#[derive(Debug, Error)]
pub enum LlmGatewayError {
    #[error("llm provider request timed out")]
    Timeout,
    #[error("llm provider request failed: {0}")]
    ProviderFailure(String),
    #[error("llm provider returned an invalid payload: {0}")]
    InvalidProviderPayload(String),
}

pub trait LlmGateway: Send + Sync {
    fn generate<'a>(&'a self, request: LlmGatewayRequest) -> LlmGatewayFuture<'a>;
}
