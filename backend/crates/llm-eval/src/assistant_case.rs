use serde::{Deserialize, Serialize};
use shared::models::AssistantQueryCapability;

#[derive(Debug, Clone, Deserialize)]
pub struct AssistantRoutingEvalCaseFixture {
    pub case_id: String,
    pub description: String,
    pub query: String,
    #[serde(default)]
    pub prior_capability: Option<AssistantQueryCapability>,
    pub expectations: AssistantRoutingExpectations,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssistantRoutingExpectations {
    #[serde(default)]
    pub detected_capability: Option<AssistantQueryCapability>,
    pub resolved_capability: AssistantQueryCapability,
    pub expected_response_part_types: Vec<ExpectedResponsePartType>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedResponsePartType {
    ChatText,
    ToolSummary,
}
