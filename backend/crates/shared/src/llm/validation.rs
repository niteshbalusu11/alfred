use jsonschema::JSONSchema;
use serde_json::Value;
use std::sync::LazyLock;
use thiserror::Error;

use super::contracts::{
    AssistantCapability, AssistantOutputContract, ContractError, output_schema, parse_contract,
};

#[derive(Debug, Error)]
pub enum OutputValidationError {
    #[error("assistant output is not valid json: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("assistant schema for {capability:?} failed to compile: {message}")]
    SchemaCompile {
        capability: AssistantCapability,
        message: String,
    },
    #[error("assistant output failed schema validation for {capability:?}: {errors:?}")]
    SchemaViolation {
        capability: AssistantCapability,
        errors: Vec<String>,
    },
    #[error(transparent)]
    Contract(#[from] ContractError),
}

pub fn validate_output_json(
    capability: AssistantCapability,
    raw_json: &str,
) -> Result<AssistantOutputContract, OutputValidationError> {
    let payload: Value = serde_json::from_str(raw_json)?;
    validate_output_value(capability, &payload)
}

pub fn validate_output_value(
    capability: AssistantCapability,
    payload: &Value,
) -> Result<AssistantOutputContract, OutputValidationError> {
    let validator = validator_for_capability(capability)?;

    if let Err(validation_errors) = validator.validate(payload) {
        let errors = validation_errors
            .map(|err| err.to_string())
            .collect::<Vec<_>>();
        return Err(OutputValidationError::SchemaViolation { capability, errors });
    }

    parse_contract(capability, payload.clone()).map_err(OutputValidationError::from)
}

static MEETINGS_SUMMARY_VALIDATOR: LazyLock<Result<JSONSchema, String>> = LazyLock::new(|| {
    JSONSchema::compile(&output_schema(AssistantCapability::MeetingsSummary))
        .map_err(|err| err.to_string())
});

static GENERAL_CHAT_VALIDATOR: LazyLock<Result<JSONSchema, String>> = LazyLock::new(|| {
    JSONSchema::compile(&output_schema(AssistantCapability::GeneralChat))
        .map_err(|err| err.to_string())
});

static MORNING_BRIEF_VALIDATOR: LazyLock<Result<JSONSchema, String>> = LazyLock::new(|| {
    JSONSchema::compile(&output_schema(AssistantCapability::MorningBrief))
        .map_err(|err| err.to_string())
});

static URGENT_EMAIL_SUMMARY_VALIDATOR: LazyLock<Result<JSONSchema, String>> = LazyLock::new(|| {
    JSONSchema::compile(&output_schema(AssistantCapability::UrgentEmailSummary))
        .map_err(|err| err.to_string())
});

static ASSISTANT_SEMANTIC_PLAN_VALIDATOR: LazyLock<Result<JSONSchema, String>> =
    LazyLock::new(|| {
        JSONSchema::compile(&output_schema(AssistantCapability::AssistantSemanticPlan))
            .map_err(|err| err.to_string())
    });

fn validator_for_capability(
    capability: AssistantCapability,
) -> Result<&'static JSONSchema, OutputValidationError> {
    let validator_result = match capability {
        AssistantCapability::MeetingsSummary => &*MEETINGS_SUMMARY_VALIDATOR,
        AssistantCapability::GeneralChat => &*GENERAL_CHAT_VALIDATOR,
        AssistantCapability::MorningBrief => &*MORNING_BRIEF_VALIDATOR,
        AssistantCapability::UrgentEmailSummary => &*URGENT_EMAIL_SUMMARY_VALIDATOR,
        AssistantCapability::AssistantSemanticPlan => &*ASSISTANT_SEMANTIC_PLAN_VALIDATOR,
    };

    validator_result
        .as_ref()
        .map_err(|message| OutputValidationError::SchemaCompile {
            capability,
            message: message.clone(),
        })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{OutputValidationError, validate_output_json, validate_output_value};
    use crate::llm::contracts::{
        AssistantCapability, AssistantOutputContract, ContractError, OUTPUT_CONTRACT_VERSION_V1,
    };

    #[test]
    fn validate_output_value_accepts_valid_meetings_summary_contract() {
        let payload = json!({
            "version": OUTPUT_CONTRACT_VERSION_V1,
            "output": {
                "title": "Team sync",
                "summary": "Discussed launch blockers and ownership.",
                "key_points": ["API migration risk", "Need QA plan"],
                "follow_ups": ["Create QA checklist", "Confirm migration timeline"]
            }
        });

        let parsed = validate_output_value(AssistantCapability::MeetingsSummary, &payload)
            .expect("valid meetings payload should pass");

        assert!(matches!(
            parsed,
            AssistantOutputContract::MeetingsSummary(_)
        ));
        if let AssistantOutputContract::MeetingsSummary(contract) = parsed {
            assert_eq!(contract.version, OUTPUT_CONTRACT_VERSION_V1);
            assert_eq!(contract.output.key_points.len(), 2);
        }
    }

    #[test]
    fn validate_output_json_rejects_invalid_output_shape() {
        let invalid_json = r#"{
            "version":"2026-02-15",
            "output":{
                "title":"Team sync",
                "key_points":["missing required summary field"],
                "follow_ups":[]
            }
        }"#;

        let err = validate_output_json(AssistantCapability::MeetingsSummary, invalid_json)
            .expect_err("missing required fields must fail validation");

        assert!(
            matches!(err, OutputValidationError::SchemaViolation { .. }),
            "expected schema violation, got {err:?}"
        );
    }

    #[test]
    fn validate_output_value_rejects_schema_mismatch_for_capability() {
        let morning_brief_payload = json!({
            "version": OUTPUT_CONTRACT_VERSION_V1,
            "output": {
                "headline": "Good morning",
                "summary": "You have two meetings and one high-priority email.",
                "priorities": ["Finalize roadmap", "Reply to legal review"],
                "schedule": ["09:00 Product sync", "13:00 Design review"],
                "alerts": ["Budget approval deadline at 17:00"]
            }
        });

        let err = validate_output_value(
            AssistantCapability::UrgentEmailSummary,
            &morning_brief_payload,
        )
        .expect_err("mismatched payload should fail schema validation");

        assert!(
            matches!(err, OutputValidationError::SchemaViolation { .. }),
            "expected schema violation, got {err:?}"
        );
    }

    #[test]
    fn validate_output_value_rejects_contract_version_mismatch() {
        let payload = json!({
            "version": "2025-01-01",
            "output": {
                "title": "Team sync",
                "summary": "Summary text.",
                "key_points": [],
                "follow_ups": []
            }
        });

        let err = validate_output_value(AssistantCapability::MeetingsSummary, &payload)
            .expect_err("stale contract version must be rejected");

        assert!(
            matches!(
                err,
                OutputValidationError::Contract(ContractError::VersionMismatch { .. })
            ),
            "expected version mismatch, got {err:?}"
        );
    }
}
