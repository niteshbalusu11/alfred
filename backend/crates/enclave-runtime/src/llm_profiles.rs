use std::env;
use std::sync::Arc;

use shared::llm::{
    LlmGateway, LlmReliabilityConfig, OpenRouterGatewayConfig, ReliableGatewayBuildError,
    ReliableOpenRouterGateway,
};
use tracing::warn;

type DynLlmGateway = dyn LlmGateway + Send + Sync;

const ASSISTANT_PLANNER_PROFILE_PREFIX: &str = "ASSISTANT_PLANNER";
const ASSISTANT_CHAT_PROFILE_PREFIX: &str = "ASSISTANT_CHAT";

const DEFAULT_ASSISTANT_PLANNER_TIMEOUT_MS: u64 = 4_000;
const DEFAULT_ASSISTANT_PLANNER_MAX_RETRIES: u32 = 0;
const DEFAULT_ASSISTANT_PLANNER_MAX_OUTPUT_TOKENS: u32 = 180;

const DEFAULT_ASSISTANT_CHAT_TIMEOUT_MS: u64 = 3_000;
const DEFAULT_ASSISTANT_CHAT_MAX_RETRIES: u32 = 0;
const DEFAULT_ASSISTANT_CHAT_MAX_OUTPUT_TOKENS: u32 = 260;

#[derive(Clone)]
pub(crate) struct LlmGatewayProfiles {
    planner: Arc<DynLlmGateway>,
    assistant_chat: Arc<DynLlmGateway>,
    worker: Arc<DynLlmGateway>,
}

impl LlmGatewayProfiles {
    pub(crate) fn planner(&self) -> &DynLlmGateway {
        self.planner.as_ref()
    }

    pub(crate) fn assistant_chat(&self) -> &DynLlmGateway {
        self.assistant_chat.as_ref()
    }

    pub(crate) fn worker(&self) -> &DynLlmGateway {
        self.worker.as_ref()
    }
}

pub(crate) async fn build_llm_gateway_profiles(
    openrouter_config: OpenRouterGatewayConfig,
    llm_reliability_config: LlmReliabilityConfig,
    redis_url: &str,
) -> Result<LlmGatewayProfiles, ReliableGatewayBuildError> {
    let planner_config = assistant_profile_config(
        &openrouter_config,
        ASSISTANT_PLANNER_PROFILE_PREFIX,
        AssistantProfileDefaults {
            timeout_ms: DEFAULT_ASSISTANT_PLANNER_TIMEOUT_MS,
            max_retries: DEFAULT_ASSISTANT_PLANNER_MAX_RETRIES,
            max_output_tokens: DEFAULT_ASSISTANT_PLANNER_MAX_OUTPUT_TOKENS,
            use_model_fallback: false,
        },
    );
    let assistant_chat_config = assistant_profile_config(
        &openrouter_config,
        ASSISTANT_CHAT_PROFILE_PREFIX,
        AssistantProfileDefaults {
            timeout_ms: DEFAULT_ASSISTANT_CHAT_TIMEOUT_MS,
            max_retries: DEFAULT_ASSISTANT_CHAT_MAX_RETRIES,
            max_output_tokens: DEFAULT_ASSISTANT_CHAT_MAX_OUTPUT_TOKENS,
            use_model_fallback: false,
        },
    );
    let planner = build_gateway(planner_config, llm_reliability_config.clone(), redis_url).await?;
    let assistant_chat = build_gateway(
        assistant_chat_config,
        llm_reliability_config.clone(),
        redis_url,
    )
    .await?;
    let worker = build_gateway(openrouter_config, llm_reliability_config, redis_url).await?;

    Ok(LlmGatewayProfiles {
        planner,
        assistant_chat,
        worker,
    })
}

#[derive(Clone, Copy)]
struct AssistantProfileDefaults {
    timeout_ms: u64,
    max_retries: u32,
    max_output_tokens: u32,
    use_model_fallback: bool,
}

fn assistant_profile_config(
    base: &OpenRouterGatewayConfig,
    profile_prefix: &str,
    defaults: AssistantProfileDefaults,
) -> OpenRouterGatewayConfig {
    let overrides = AssistantProfileEnvOverrides {
        timeout_ms: optional_trimmed_env(profile_env_key(profile_prefix, "TIMEOUT_MS").as_str()),
        max_retries: optional_trimmed_env(profile_env_key(profile_prefix, "MAX_RETRIES").as_str()),
        max_output_tokens: optional_trimmed_env(
            profile_env_key(profile_prefix, "MAX_OUTPUT_TOKENS").as_str(),
        ),
        model_primary: optional_trimmed_env(
            profile_env_key(profile_prefix, "MODEL_PRIMARY").as_str(),
        ),
        model_fallback: optional_trimmed_env(
            profile_env_key(profile_prefix, "MODEL_FALLBACK").as_str(),
        ),
    };
    assistant_profile_config_with_overrides(base, defaults, overrides)
}

#[derive(Default)]
struct AssistantProfileEnvOverrides {
    timeout_ms: Option<String>,
    max_retries: Option<String>,
    max_output_tokens: Option<String>,
    model_primary: Option<String>,
    model_fallback: Option<String>,
}

fn assistant_profile_config_with_overrides(
    base: &OpenRouterGatewayConfig,
    defaults: AssistantProfileDefaults,
    overrides: AssistantProfileEnvOverrides,
) -> OpenRouterGatewayConfig {
    let mut config = base.clone();
    config.timeout_ms = parse_u64_override(
        overrides.timeout_ms.as_deref(),
        "profile_timeout_ms",
        defaults.timeout_ms,
        true,
    );
    config.max_retries = parse_u32_override(
        overrides.max_retries.as_deref(),
        "profile_max_retries",
        defaults.max_retries,
        false,
    );
    config.max_output_tokens = parse_u32_override(
        overrides.max_output_tokens.as_deref(),
        "profile_max_output_tokens",
        defaults.max_output_tokens,
        true,
    );
    if let Some(primary_model) = overrides.model_primary {
        config.model_route.primary_model = primary_model;
    }

    let fallback_model = overrides.model_fallback.or_else(|| {
        if defaults.use_model_fallback {
            base.model_route.fallback_model.clone()
        } else {
            None
        }
    });
    config.model_route.fallback_model = fallback_model
        .filter(|model| model != &config.model_route.primary_model && !model.trim().is_empty());

    config
}

async fn build_gateway(
    openrouter_config: OpenRouterGatewayConfig,
    llm_reliability_config: LlmReliabilityConfig,
    redis_url: &str,
) -> Result<Arc<DynLlmGateway>, ReliableGatewayBuildError> {
    let gateway = ReliableOpenRouterGateway::from_openrouter_config_with_redis(
        openrouter_config,
        llm_reliability_config,
        redis_url,
    )
    .await?;
    Ok(Arc::new(gateway))
}

fn profile_env_key(profile_prefix: &str, suffix: &str) -> String {
    format!("{profile_prefix}_OPENROUTER_{suffix}")
}

fn optional_trimmed_env(key: &str) -> Option<String> {
    env::var(key).ok().and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn parse_u64_override(
    value: Option<&str>,
    default_key_label: &str,
    default: u64,
    reject_zero: bool,
) -> u64 {
    let Some(raw) = value else {
        return default;
    };
    match raw.parse::<u64>() {
        Ok(parsed) if !(reject_zero && parsed == 0) => parsed,
        _ => {
            warn!(
                key = %default_key_label,
                value = %raw,
                fallback = default,
                "invalid assistant OpenRouter u64 override; using default"
            );
            default
        }
    }
}

fn parse_u32_override(
    value: Option<&str>,
    default_key_label: &str,
    default: u32,
    reject_zero: bool,
) -> u32 {
    let Some(raw) = value else {
        return default;
    };
    match raw.parse::<u32>() {
        Ok(parsed) if !(reject_zero && parsed == 0) => parsed,
        _ => {
            warn!(
                key = %default_key_label,
                value = %raw,
                fallback = default,
                "invalid assistant OpenRouter u32 override; using default"
            );
            default
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AssistantProfileDefaults, AssistantProfileEnvOverrides,
        assistant_profile_config_with_overrides,
    };
    use shared::llm::{OpenRouterGatewayConfig, OpenRouterModelRoute};

    fn base_config() -> OpenRouterGatewayConfig {
        OpenRouterGatewayConfig {
            chat_completions_url: "https://openrouter.ai/api/v1/chat/completions".to_string(),
            api_key: "test".to_string(),
            app_http_referer: None,
            app_title: None,
            timeout_ms: 15_000,
            max_retries: 2,
            retry_base_backoff_ms: 250,
            max_output_tokens: 600,
            allow_insecure_http: false,
            model_route: OpenRouterModelRoute {
                primary_model: "openai/gpt-4o-mini".to_string(),
                fallback_model: Some("anthropic/claude-3.5-haiku".to_string()),
            },
        }
    }

    #[test]
    fn profile_defaults_disable_fallback_and_apply_latency_defaults() {
        let config = assistant_profile_config_with_overrides(
            &base_config(),
            AssistantProfileDefaults {
                timeout_ms: 4_000,
                max_retries: 0,
                max_output_tokens: 180,
                use_model_fallback: false,
            },
            AssistantProfileEnvOverrides::default(),
        );
        assert_eq!(config.timeout_ms, 4_000);
        assert_eq!(config.max_retries, 0);
        assert_eq!(config.max_output_tokens, 180);
        assert!(config.model_route.fallback_model.is_none());
    }

    #[test]
    fn profile_overrides_apply_when_valid() {
        let config = assistant_profile_config_with_overrides(
            &base_config(),
            AssistantProfileDefaults {
                timeout_ms: 4_000,
                max_retries: 0,
                max_output_tokens: 180,
                use_model_fallback: false,
            },
            AssistantProfileEnvOverrides {
                timeout_ms: Some("5200".to_string()),
                max_retries: Some("1".to_string()),
                max_output_tokens: Some("320".to_string()),
                model_primary: Some("openai/gpt-4.1-mini".to_string()),
                model_fallback: Some("anthropic/claude-3.5-haiku".to_string()),
            },
        );
        assert_eq!(config.timeout_ms, 5_200);
        assert_eq!(config.max_retries, 1);
        assert_eq!(config.max_output_tokens, 320);
        assert_eq!(config.model_route.primary_model, "openai/gpt-4.1-mini");
        assert_eq!(
            config.model_route.fallback_model.as_deref(),
            Some("anthropic/claude-3.5-haiku")
        );
    }

    #[test]
    fn invalid_overrides_fall_back_to_defaults() {
        let config = assistant_profile_config_with_overrides(
            &base_config(),
            AssistantProfileDefaults {
                timeout_ms: 4_000,
                max_retries: 0,
                max_output_tokens: 180,
                use_model_fallback: false,
            },
            AssistantProfileEnvOverrides {
                timeout_ms: Some("invalid".to_string()),
                max_retries: Some("-1".to_string()),
                max_output_tokens: Some("0".to_string()),
                model_primary: None,
                model_fallback: Some("openai/gpt-4o-mini".to_string()),
            },
        );
        assert_eq!(config.timeout_ms, 4_000);
        assert_eq!(config.max_retries, 0);
        assert_eq!(config.max_output_tokens, 180);
        assert!(config.model_route.fallback_model.is_none());
    }
}
