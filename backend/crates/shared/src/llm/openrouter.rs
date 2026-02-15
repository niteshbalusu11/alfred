use std::env;
use std::time::Duration;

use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::{Value, json};
use thiserror::Error;
use tokio::time::sleep;

use super::gateway::{
    LlmGateway, LlmGatewayError, LlmGatewayFuture, LlmGatewayRequest, LlmGatewayResponse,
    LlmTokenUsage,
};

const DEFAULT_CHAT_COMPLETIONS_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const DEFAULT_TIMEOUT_MS: u64 = 15_000;
const DEFAULT_MAX_RETRIES: u32 = 2;
const DEFAULT_RETRY_BASE_BACKOFF_MS: u64 = 250;

const DEFAULT_PRIMARY_MODEL: &str = "openai/gpt-4o-mini";
const DEFAULT_FALLBACK_MODEL: &str = "anthropic/claude-3.5-haiku";

#[derive(Debug, Clone)]
pub struct OpenRouterModelRoute {
    pub primary_model: String,
    pub fallback_model: Option<String>,
}

impl OpenRouterModelRoute {
    fn candidate_models(&self) -> Vec<&str> {
        let mut candidates = Vec::new();
        if !self.primary_model.is_empty() {
            candidates.push(self.primary_model.as_str());
        }

        if let Some(fallback_model) = self.fallback_model.as_deref()
            && !fallback_model.is_empty()
            && fallback_model != self.primary_model
        {
            candidates.push(fallback_model);
        }

        candidates
    }
}

#[derive(Debug, Clone)]
pub struct OpenRouterGatewayConfig {
    pub chat_completions_url: String,
    pub api_key: String,
    pub timeout_ms: u64,
    pub max_retries: u32,
    pub retry_base_backoff_ms: u64,
    pub model_route: OpenRouterModelRoute,
}

impl OpenRouterGatewayConfig {
    pub fn from_env() -> Result<Self, OpenRouterConfigError> {
        let api_key = require_non_empty_env("OPENROUTER_API_KEY")?;
        let chat_completions_url = optional_trimmed_env("OPENROUTER_CHAT_COMPLETIONS_URL")
            .unwrap_or_else(|| DEFAULT_CHAT_COMPLETIONS_URL.to_string());
        if !chat_completions_url.starts_with("http://")
            && !chat_completions_url.starts_with("https://")
        {
            return Err(OpenRouterConfigError::InvalidConfiguration(
                "OPENROUTER_CHAT_COMPLETIONS_URL must start with http:// or https://".to_string(),
            ));
        }

        Ok(Self {
            chat_completions_url,
            api_key,
            timeout_ms: parse_u64_env("OPENROUTER_TIMEOUT_MS", DEFAULT_TIMEOUT_MS)?,
            max_retries: parse_u32_env("OPENROUTER_MAX_RETRIES", DEFAULT_MAX_RETRIES)?,
            retry_base_backoff_ms: parse_u64_env(
                "OPENROUTER_RETRY_BASE_BACKOFF_MS",
                DEFAULT_RETRY_BASE_BACKOFF_MS,
            )?,
            model_route: parse_model_route(),
        })
    }
}

#[derive(Debug, Error)]
pub enum OpenRouterConfigError {
    #[error("missing required env var {0}")]
    MissingVar(String),
    #[error("invalid integer in env var {key}: {value}")]
    ParseInt { key: String, value: String },
    #[error("invalid configuration: {0}")]
    InvalidConfiguration(String),
    #[error("failed to build OpenRouter http client: {0}")]
    HttpClient(String),
}

#[derive(Clone)]
pub struct OpenRouterGateway {
    client: reqwest::Client,
    config: OpenRouterGatewayConfig,
}

impl OpenRouterGateway {
    pub fn new(config: OpenRouterGatewayConfig) -> Result<Self, OpenRouterConfigError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .build()
            .map_err(|err| OpenRouterConfigError::HttpClient(err.to_string()))?;

        Ok(Self { client, config })
    }

    async fn generate_for_model(
        &self,
        model: &str,
        request: &LlmGatewayRequest,
    ) -> Result<LlmGatewayResponse, ModelAttemptError> {
        let mut attempt = 0_u32;

        loop {
            match self.send_once(model, request).await {
                Ok(response) => return Ok(response),
                Err(err) => {
                    if err.retryable && attempt < self.config.max_retries {
                        let backoff_multiplier = 2_u64.saturating_pow(attempt);
                        let backoff_ms = self
                            .config
                            .retry_base_backoff_ms
                            .saturating_mul(backoff_multiplier);
                        sleep(Duration::from_millis(backoff_ms)).await;
                        attempt = attempt.saturating_add(1);
                        continue;
                    }

                    return Err(ModelAttemptError {
                        error: err.error,
                        fallback_allowed: err.fallback_allowed,
                    });
                }
            }
        }
    }

    async fn send_once(
        &self,
        model: &str,
        request: &LlmGatewayRequest,
    ) -> Result<LlmGatewayResponse, SendAttemptError> {
        let user_prompt = json!({
            "instruction": request.context_prompt,
            "contract_version": request.contract_version,
            "output_schema": request.output_schema,
            "context_payload": request.context_payload,
        })
        .to_string();

        let request_body = json!({
            "model": model,
            "messages": [
                { "role": "system", "content": request.system_prompt },
                { "role": "user", "content": user_prompt }
            ],
            "response_format": {
                "type": "json_object"
            },
            "temperature": 0
        });

        let response = self
            .client
            .post(&self.config.chat_completions_url)
            .bearer_auth(&self.config.api_key)
            .json(&request_body)
            .send()
            .await
            .map_err(|err| {
                if err.is_timeout() {
                    SendAttemptError::retryable(
                        LlmGatewayError::Timeout,
                        true, // allow fallback to alternate model on timeout.
                    )
                } else {
                    SendAttemptError::retryable(
                        LlmGatewayError::ProviderFailure("request_unavailable".to_string()),
                        true,
                    )
                }
            })?;

        let status = response.status();
        let header_request_id = header_request_id(response.headers());
        let body = response.text().await.map_err(|_| {
            SendAttemptError::non_retryable(
                LlmGatewayError::InvalidProviderPayload("response_body_read_failed".to_string()),
                true,
            )
        })?;

        if !status.is_success() {
            let provider_code = parse_provider_error_code(&body);
            let is_retryable = is_retryable_status(status);
            let fallback_allowed =
                status != StatusCode::UNAUTHORIZED && status != StatusCode::FORBIDDEN;
            return Err(SendAttemptError {
                error: LlmGatewayError::ProviderFailure(format!(
                    "status={} code={provider_code}",
                    status.as_u16()
                )),
                retryable: is_retryable,
                fallback_allowed,
            });
        }

        let parsed: OpenRouterSuccessResponse = serde_json::from_str(&body).map_err(|_| {
            SendAttemptError::non_retryable(
                LlmGatewayError::InvalidProviderPayload("response_json_parse_failed".to_string()),
                true,
            )
        })?;

        let content = parsed
            .choices
            .first()
            .ok_or_else(|| {
                SendAttemptError::non_retryable(
                    LlmGatewayError::InvalidProviderPayload("missing_choice".to_string()),
                    true,
                )
            })?
            .message
            .content
            .clone();

        let output = match content {
            Value::String(raw) => serde_json::from_str::<Value>(&raw).map_err(|_| {
                SendAttemptError::non_retryable(
                    LlmGatewayError::InvalidProviderPayload("content_not_json".to_string()),
                    true,
                )
            })?,
            value @ (Value::Object(_) | Value::Array(_)) => value,
            _ => {
                return Err(SendAttemptError::non_retryable(
                    LlmGatewayError::InvalidProviderPayload(
                        "unsupported_content_shape".to_string(),
                    ),
                    true,
                ));
            }
        };

        Ok(LlmGatewayResponse {
            model: parsed.model.unwrap_or_else(|| model.to_string()),
            provider_request_id: header_request_id.or(parsed.id),
            output,
            usage: parsed.usage.map(|usage| LlmTokenUsage {
                prompt_tokens: clamp_u64_to_u32(usage.prompt_tokens.unwrap_or(0)),
                completion_tokens: clamp_u64_to_u32(usage.completion_tokens.unwrap_or(0)),
                total_tokens: clamp_u64_to_u32(usage.total_tokens.unwrap_or(0)),
            }),
        })
    }
}

impl LlmGateway for OpenRouterGateway {
    fn generate<'a>(&'a self, request: LlmGatewayRequest) -> LlmGatewayFuture<'a> {
        Box::pin(async move {
            let candidate_models = self.config.model_route.candidate_models();

            for (index, model) in candidate_models.iter().enumerate() {
                match self.generate_for_model(model, &request).await {
                    Ok(response) => return Ok(response),
                    Err(model_err) => {
                        let has_more_candidates = index + 1 < candidate_models.len();
                        if has_more_candidates && model_err.fallback_allowed {
                            continue;
                        }
                        return Err(model_err.error);
                    }
                }
            }

            Err(LlmGatewayError::ProviderFailure(
                "no_openrouter_model_candidates".to_string(),
            ))
        })
    }
}

#[derive(Debug)]
struct SendAttemptError {
    error: LlmGatewayError,
    retryable: bool,
    fallback_allowed: bool,
}

impl SendAttemptError {
    fn retryable(error: LlmGatewayError, fallback_allowed: bool) -> Self {
        Self {
            error,
            retryable: true,
            fallback_allowed,
        }
    }

    fn non_retryable(error: LlmGatewayError, fallback_allowed: bool) -> Self {
        Self {
            error,
            retryable: false,
            fallback_allowed,
        }
    }
}

#[derive(Debug)]
struct ModelAttemptError {
    error: LlmGatewayError,
    fallback_allowed: bool,
}

#[derive(Debug, Deserialize)]
struct OpenRouterSuccessResponse {
    id: Option<String>,
    model: Option<String>,
    choices: Vec<OpenRouterChoice>,
    usage: Option<OpenRouterUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterChoice {
    message: OpenRouterMessage,
}

#[derive(Debug, Deserialize)]
struct OpenRouterMessage {
    content: Value,
}

#[derive(Debug, Deserialize)]
struct OpenRouterUsage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
    total_tokens: Option<u64>,
}

fn parse_model_route() -> OpenRouterModelRoute {
    OpenRouterModelRoute {
        primary_model: optional_trimmed_env("OPENROUTER_MODEL_PRIMARY")
            .unwrap_or_else(|| DEFAULT_PRIMARY_MODEL.to_string()),
        fallback_model: optional_trimmed_env("OPENROUTER_MODEL_FALLBACK")
            .or_else(|| Some(DEFAULT_FALLBACK_MODEL.to_string())),
    }
}

fn require_non_empty_env(key: &str) -> Result<String, OpenRouterConfigError> {
    let value = env::var(key).map_err(|_| OpenRouterConfigError::MissingVar(key.to_string()))?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(OpenRouterConfigError::MissingVar(key.to_string()));
    }
    Ok(trimmed.to_string())
}

fn parse_u64_env(key: &str, default: u64) -> Result<u64, OpenRouterConfigError> {
    match optional_trimmed_env(key) {
        Some(value) => value
            .parse::<u64>()
            .map_err(|_| OpenRouterConfigError::ParseInt {
                key: key.to_string(),
                value,
            }),
        None => Ok(default),
    }
}

fn parse_u32_env(key: &str, default: u32) -> Result<u32, OpenRouterConfigError> {
    match optional_trimmed_env(key) {
        Some(value) => value
            .parse::<u32>()
            .map_err(|_| OpenRouterConfigError::ParseInt {
                key: key.to_string(),
                value,
            }),
        None => Ok(default),
    }
}

fn optional_trimmed_env(key: &str) -> Option<String> {
    env::var(key).ok().and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn is_retryable_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::REQUEST_TIMEOUT
            | StatusCode::TOO_MANY_REQUESTS
            | StatusCode::INTERNAL_SERVER_ERROR
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
    )
}

fn header_request_id(headers: &reqwest::header::HeaderMap) -> Option<String> {
    headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string)
}

fn parse_provider_error_code(body: &str) -> String {
    #[derive(Deserialize)]
    struct ProviderErrorEnvelope {
        error: Option<ProviderErrorDetails>,
    }

    #[derive(Deserialize)]
    struct ProviderErrorDetails {
        code: Option<Value>,
    }

    let parsed = serde_json::from_str::<ProviderErrorEnvelope>(body).ok();
    let Some(provider_error_code) = parsed
        .and_then(|envelope| envelope.error)
        .and_then(|details| details.code)
    else {
        return "unknown".to_string();
    };

    match provider_error_code {
        Value::String(code) => code,
        Value::Number(code) => code.to_string(),
        _ => "unknown".to_string(),
    }
}

fn clamp_u64_to_u32(value: u64) -> u32 {
    value.min(u32::MAX as u64) as u32
}
