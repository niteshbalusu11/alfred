use serde::Deserialize;
use serde_json::Value;
use shared::llm::AssistantCapability;

#[derive(Debug, Clone, Deserialize)]
pub struct EvalCaseFixture {
    pub case_id: String,
    pub description: String,
    pub capability: AssistantCapability,
    #[serde(default)]
    pub include_in_live_smoke: bool,
    pub context_payload: Value,
    #[serde(default)]
    pub mocked_model_output: Option<Value>,
    #[serde(default)]
    pub expectations: EvalExpectations,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EvalExpectations {
    #[serde(default = "default_schema_valid")]
    pub schema_valid: bool,
    #[serde(default)]
    pub safe_output_source: Option<ExpectedOutputSource>,
    #[serde(default)]
    pub quality: QualityExpectations,
}

impl Default for EvalExpectations {
    fn default() -> Self {
        Self {
            schema_valid: true,
            safe_output_source: None,
            quality: QualityExpectations::default(),
        }
    }
}

fn default_schema_valid() -> bool {
    true
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedOutputSource {
    ModelOutput,
    DeterministicFallback,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct QualityExpectations {
    #[serde(default)]
    pub min_key_points: Option<usize>,
    #[serde(default)]
    pub min_follow_ups: Option<usize>,
    #[serde(default)]
    pub min_priorities: Option<usize>,
    #[serde(default)]
    pub min_schedule: Option<usize>,
    #[serde(default)]
    pub min_alerts: Option<usize>,
    #[serde(default)]
    pub min_suggested_actions: Option<usize>,
}
