use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::assistant_semantic_plan::{
    ASSISTANT_SEMANTIC_PLAN_VERSION_V1, AssistantSemanticPlanContract,
};

pub const OUTPUT_CONTRACT_VERSION_V1: &str = "2026-02-15";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssistantCapability {
    MeetingsSummary,
    GeneralChatSummary,
    MorningBrief,
    UrgentEmailSummary,
    AssistantSemanticPlan,
}

impl AssistantCapability {
    pub const fn contract_version(self) -> &'static str {
        match self {
            Self::AssistantSemanticPlan => ASSISTANT_SEMANTIC_PLAN_VERSION_V1,
            _ => OUTPUT_CONTRACT_VERSION_V1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MeetingsSummaryContract {
    pub version: String,
    pub output: MeetingsSummaryOutput,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GeneralChatSummaryContract {
    pub version: String,
    pub output: GeneralChatSummaryOutput,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MorningBriefContract {
    pub version: String,
    pub output: MorningBriefOutput,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UrgentEmailSummaryContract {
    pub version: String,
    pub output: UrgentEmailSummaryOutput,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MeetingsSummaryOutput {
    pub title: String,
    pub summary: String,
    pub key_points: Vec<String>,
    pub follow_ups: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GeneralChatSummaryOutput {
    pub title: String,
    pub summary: String,
    pub key_points: Vec<String>,
    pub follow_ups: Vec<String>,
    pub response_style: ChatResponseStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChatResponseStyle {
    Conversational,
    Structured,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MorningBriefOutput {
    pub headline: String,
    pub summary: String,
    pub priorities: Vec<String>,
    pub schedule: Vec<String>,
    pub alerts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UrgentEmailSummaryOutput {
    pub should_notify: bool,
    pub urgency: UrgencyLevel,
    pub summary: String,
    pub reason: String,
    pub suggested_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum UrgencyLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone)]
pub enum AssistantOutputContract {
    MeetingsSummary(MeetingsSummaryContract),
    GeneralChatSummary(GeneralChatSummaryContract),
    MorningBrief(MorningBriefContract),
    UrgentEmailSummary(UrgentEmailSummaryContract),
    AssistantSemanticPlan(AssistantSemanticPlanContract),
}

#[derive(Debug, Error)]
pub enum ContractError {
    #[error("output contract payload is invalid: {0}")]
    Deserialize(#[from] serde_json::Error),
    #[error(
        "output contract version mismatch for {capability:?}: expected={expected}, actual={actual}"
    )]
    VersionMismatch {
        capability: AssistantCapability,
        expected: String,
        actual: String,
    },
}

pub fn output_schema(capability: AssistantCapability) -> Value {
    match capability {
        AssistantCapability::MeetingsSummary => {
            serde_json::to_value(schema_for!(MeetingsSummaryContract))
                .expect("meetings summary schema should be serializable")
        }
        AssistantCapability::GeneralChatSummary => {
            serde_json::to_value(schema_for!(GeneralChatSummaryContract))
                .expect("general chat summary schema should be serializable")
        }
        AssistantCapability::MorningBrief => {
            serde_json::to_value(schema_for!(MorningBriefContract))
                .expect("morning brief schema should be serializable")
        }
        AssistantCapability::UrgentEmailSummary => {
            serde_json::to_value(schema_for!(UrgentEmailSummaryContract))
                .expect("urgent email summary schema should be serializable")
        }
        AssistantCapability::AssistantSemanticPlan => {
            serde_json::to_value(schema_for!(AssistantSemanticPlanContract))
                .expect("assistant semantic plan schema should be serializable")
        }
    }
}

pub fn parse_contract(
    capability: AssistantCapability,
    payload: Value,
) -> Result<AssistantOutputContract, ContractError> {
    match capability {
        AssistantCapability::MeetingsSummary => {
            let contract: MeetingsSummaryContract = serde_json::from_value(payload)?;
            ensure_contract_version(capability, &contract.version)?;
            Ok(AssistantOutputContract::MeetingsSummary(contract))
        }
        AssistantCapability::GeneralChatSummary => {
            let contract: GeneralChatSummaryContract = serde_json::from_value(payload)?;
            ensure_contract_version(capability, &contract.version)?;
            Ok(AssistantOutputContract::GeneralChatSummary(contract))
        }
        AssistantCapability::MorningBrief => {
            let contract: MorningBriefContract = serde_json::from_value(payload)?;
            ensure_contract_version(capability, &contract.version)?;
            Ok(AssistantOutputContract::MorningBrief(contract))
        }
        AssistantCapability::UrgentEmailSummary => {
            let contract: UrgentEmailSummaryContract = serde_json::from_value(payload)?;
            ensure_contract_version(capability, &contract.version)?;
            Ok(AssistantOutputContract::UrgentEmailSummary(contract))
        }
        AssistantCapability::AssistantSemanticPlan => {
            let contract: AssistantSemanticPlanContract = serde_json::from_value(payload)?;
            ensure_contract_version(capability, &contract.version)?;
            Ok(AssistantOutputContract::AssistantSemanticPlan(contract))
        }
    }
}

fn ensure_contract_version(
    capability: AssistantCapability,
    actual_version: &str,
) -> Result<(), ContractError> {
    let expected_version = capability.contract_version();
    if actual_version == expected_version {
        return Ok(());
    }

    Err(ContractError::VersionMismatch {
        capability,
        expected: expected_version.to_string(),
        actual: actual_version.to_string(),
    })
}
