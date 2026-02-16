pub mod context;
pub mod contracts;
pub mod gateway;
pub mod observability;
pub mod openrouter;
pub mod prompts;
pub mod reliability;
pub mod safety;
pub mod validation;

pub use context::{
    CONTEXT_CONTRACT_VERSION_V1, GoogleCalendarMeetingSource, GoogleEmailCandidateSource,
    MeetingContextEntry, MeetingsTodayContext, MorningBriefContext,
    UrgentEmailCandidateContextEntry, UrgentEmailCandidatesContext,
    assemble_meetings_today_context, assemble_morning_brief_context,
    assemble_urgent_email_candidates_context,
};
pub use contracts::{
    AssistantCapability, AssistantOutputContract, ContractError, MeetingsSummaryContract,
    MorningBriefContract, UrgentEmailSummaryContract, output_schema,
};
pub use gateway::{LlmGateway, LlmGatewayError, LlmGatewayRequest, LlmGatewayResponse};
pub use observability::{LlmExecutionSource, LlmTelemetryEvent, generate_with_telemetry};
pub use openrouter::{
    OpenRouterConfigError, OpenRouterGateway, OpenRouterGatewayConfig, OpenRouterModelRoute,
};
pub use prompts::{PromptTemplate, template_for_capability};
pub use reliability::{
    LlmReliabilityConfig, LlmReliabilityConfigError, ReliableGatewayBuildError,
    ReliableOpenRouterGateway,
};
pub use safety::{SafeOutputSource, resolve_safe_output, sanitize_context_payload};
pub use validation::{OutputValidationError, validate_output_json, validate_output_value};
