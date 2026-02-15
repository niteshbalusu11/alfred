pub mod contracts;
pub mod gateway;
pub mod openrouter;
pub mod prompts;
pub mod validation;

pub use contracts::{
    AssistantCapability, AssistantOutputContract, ContractError, MeetingsSummaryContract,
    MorningBriefContract, UrgentEmailSummaryContract, output_schema,
};
pub use gateway::{LlmGateway, LlmGatewayError, LlmGatewayRequest, LlmGatewayResponse};
pub use openrouter::{
    OpenRouterConfigError, OpenRouterGateway, OpenRouterGatewayConfig, OpenRouterModelRoute,
};
pub use prompts::{PromptTemplate, template_for_capability};
pub use validation::{OutputValidationError, validate_output_json, validate_output_value};
