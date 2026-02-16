use std::path::Path;

use serde_json::{Value, json};
use shared::llm::{
    AssistantOutputContract, LlmGateway, LlmGatewayRequest, OpenRouterConfigError,
    OpenRouterGateway, OpenRouterGatewayConfig, SafeOutputSource, resolve_safe_output,
    template_for_capability, validate_output_value,
};
use thiserror::Error;

use crate::case::{EvalCaseFixture, ExpectedOutputSource};
use crate::cli::{CliOptions, EvalMode};
use crate::fixture_io::{
    FixtureIoError, golden_path, load_cases, read_json_value, write_pretty_json,
};
use crate::quality::evaluate_quality;

#[derive(Debug)]
pub struct EvalSummary {
    mode: EvalMode,
    update_goldens: bool,
    results: Vec<CaseResult>,
}

impl EvalSummary {
    pub fn has_failures(&self) -> bool {
        self.results
            .iter()
            .any(|result| !result.failures.is_empty())
    }

    pub fn print(&self) {
        println!(
            "LLM Eval Harness ({})",
            if self.update_goldens {
                "mocked/update-goldens"
            } else {
                self.mode.as_str()
            }
        );

        let mut passed = 0usize;
        for result in &self.results {
            if result.failures.is_empty() {
                passed += 1;
                println!("[PASS] {}: {}", result.case_id, result.description);
            } else {
                println!("[FAIL] {}: {}", result.case_id, result.description);
                for failure in &result.failures {
                    println!("  - {failure}");
                }
            }

            for note in &result.notes {
                println!("  * {note}");
            }
        }

        let total = self.results.len();
        let failed = total.saturating_sub(passed);
        println!(
            "Summary: {} total, {} passed, {} failed",
            total, passed, failed
        );
    }
}

#[derive(Debug)]
struct CaseResult {
    case_id: String,
    description: String,
    failures: Vec<String>,
    notes: Vec<String>,
}

#[derive(Debug, Error)]
pub enum EvalError {
    #[error(transparent)]
    Fixtures(#[from] FixtureIoError),
    #[error("failed to initialize OpenRouter in live mode: {0}")]
    OpenRouterConfig(#[from] OpenRouterConfigError),
    #[error("live mode requires at least one fixture with include_in_live_smoke=true")]
    NoLiveCases,
}

pub async fn run_eval(options: &CliOptions) -> Result<EvalSummary, EvalError> {
    let mut cases = load_cases()?;
    cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));

    if options.mode == EvalMode::Live {
        cases.retain(|case| case.include_in_live_smoke);
        if cases.is_empty() {
            return Err(EvalError::NoLiveCases);
        }
    }

    let gateway = if options.mode == EvalMode::Live {
        Some(OpenRouterGateway::new(OpenRouterGatewayConfig::from_env()?)?)
    } else {
        None
    };

    let mut results = Vec::with_capacity(cases.len());
    for case in &cases {
        let result = run_case(case, options, gateway.as_ref()).await;
        results.push(result);
    }

    Ok(EvalSummary {
        mode: options.mode,
        update_goldens: options.update_goldens,
        results,
    })
}

async fn run_case(
    case: &EvalCaseFixture,
    options: &CliOptions,
    gateway: Option<&OpenRouterGateway>,
) -> CaseResult {
    let mut failures = Vec::new();
    let mut notes = Vec::new();

    let request = LlmGatewayRequest::from_template(
        template_for_capability(case.capability),
        case.context_payload.clone(),
    )
    .with_requester_id(format!("llm-eval-{}", case.case_id));

    let mut model_output = case.mocked_model_output.clone();
    let mut provider_model: Option<String> = None;
    let mut provider_error: Option<String> = None;

    if options.mode == EvalMode::Live {
        let Some(gateway) = gateway else {
            failures.push("internal_error: missing live gateway instance".to_string());
            return CaseResult {
                case_id: case.case_id.clone(),
                description: case.description.clone(),
                failures,
                notes,
            };
        };

        match gateway.generate(request.clone()).await {
            Ok(response) => {
                provider_model = Some(response.model);
                model_output = Some(response.output);
            }
            Err(err) => {
                provider_error = Some(err.to_string());
                failures.push(format!("provider_request: {err}"));
            }
        }
    } else if model_output.is_none() {
        failures.push("mocked_model_output: missing output fixture for mocked mode".to_string());
    }

    let (schema_valid, schema_error) = match model_output.as_ref() {
        Some(output) => match validate_output_value(case.capability, output) {
            Ok(_) => (true, None),
            Err(err) => (false, Some(err.to_string())),
        },
        None => (false, Some("missing_model_output".to_string())),
    };

    if schema_valid != case.expectations.schema_valid {
        failures.push(format!(
            "schema_validity: expected={}, actual={}, details={}",
            case.expectations.schema_valid,
            schema_valid,
            schema_error.as_deref().unwrap_or("validation succeeded")
        ));
    }

    let resolved = resolve_safe_output(
        case.capability,
        model_output.as_ref(),
        &request.context_payload,
    );
    let actual_source = safe_source_label(resolved.source);

    if let Some(expected_source) = case.expectations.safe_output_source {
        let expected_source_label = expected_source_label(expected_source);
        if expected_source_label != actual_source {
            failures.push(format!(
                "safe_output_source: expected={expected_source_label}, actual={actual_source}"
            ));
        }
    } else if options.mode == EvalMode::Live && actual_source != "model_output" {
        failures.push(format!(
            "safe_output_source: live smoke requires model_output, got {actual_source}"
        ));
    }

    let quality_issues = evaluate_quality(&resolved.contract, &case.expectations.quality);
    for issue in &quality_issues {
        failures.push(format!("quality: {issue}"));
    }

    let snapshot = json!({
        "case_id": case.case_id,
        "description": case.description,
        "capability": case.capability,
        "request": {
            "requester_id": request.requester_id,
            "capability": request.capability,
            "contract_version": request.contract_version,
            "system_prompt": request.system_prompt,
            "context_prompt": request.context_prompt,
            "output_schema": request.output_schema,
            "context_payload": request.context_payload,
        },
        "provider_model": provider_model,
        "provider_error": provider_error,
        "model_output": model_output,
        "schema_valid": schema_valid,
        "schema_error": schema_error,
        "safe_output_source": actual_source,
        "resolved_contract": contract_to_value(&resolved.contract),
        "quality_issues": quality_issues,
    });

    if options.mode == EvalMode::Mocked {
        let path = golden_path(&case.case_id);
        if options.update_goldens {
            if let Err(err) = write_pretty_json(&path, &snapshot) {
                failures.push(format!("golden_update: {err}"));
            } else {
                notes.push(format!("golden updated: {}", path.display()));
            }
        } else {
            compare_golden_snapshot(&path, &snapshot, &mut failures);
        }
    }

    CaseResult {
        case_id: case.case_id.clone(),
        description: case.description.clone(),
        failures,
        notes,
    }
}

fn compare_golden_snapshot(path: &Path, actual: &Value, failures: &mut Vec<String>) {
    match read_json_value(path) {
        Ok(expected) => {
            if expected != *actual {
                failures.push(format!(
                    "golden_snapshot: mismatch for {} (run `just backend-eval-update` to intentionally refresh)",
                    path.display()
                ));
            }
        }
        Err(FixtureIoError::ReadFile { source, .. })
            if source.kind() == std::io::ErrorKind::NotFound =>
        {
            failures.push(format!(
                "golden_snapshot: missing {} (run `just backend-eval-update`)",
                path.display()
            ));
        }
        Err(err) => failures.push(format!("golden_snapshot: {err}")),
    }
}

fn contract_to_value(contract: &AssistantOutputContract) -> Value {
    match contract {
        AssistantOutputContract::MeetingsSummary(summary) => {
            serde_json::to_value(summary).expect("meetings summary contract should serialize")
        }
        AssistantOutputContract::MorningBrief(brief) => {
            serde_json::to_value(brief).expect("morning brief contract should serialize")
        }
        AssistantOutputContract::UrgentEmailSummary(urgent) => {
            serde_json::to_value(urgent).expect("urgent email contract should serialize")
        }
    }
}

fn safe_source_label(source: SafeOutputSource) -> &'static str {
    match source {
        SafeOutputSource::ModelOutput => "model_output",
        SafeOutputSource::DeterministicFallback => "deterministic_fallback",
    }
}

fn expected_source_label(source: ExpectedOutputSource) -> &'static str {
    match source {
        ExpectedOutputSource::ModelOutput => "model_output",
        ExpectedOutputSource::DeterministicFallback => "deterministic_fallback",
    }
}
