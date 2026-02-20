use std::collections::HashMap;

use shared::repos::{ClaimedJob, JobType};

use super::JobActionResult;
use crate::{JobExecutionError, automation_runs::AutomationRunJobPayload};

pub(super) fn resolve_job_action(job: &ClaimedJob) -> Result<JobActionResult, JobExecutionError> {
    if !matches!(job.job_type, JobType::AutomationRun) {
        return Err(JobExecutionError::permanent(
            "UNSUPPORTED_JOB_TYPE",
            format!("unsupported job type: {}", job.job_type.as_str()),
        ));
    }

    let payload =
        AutomationRunJobPayload::parse(job.payload_ciphertext.as_deref()).map_err(|err| {
            JobExecutionError::permanent("INVALID_AUTOMATION_RUN_PAYLOAD", err.to_string())
        })?;

    let mut metadata = HashMap::new();
    metadata.insert("action_source".to_string(), "automation_run".to_string());
    metadata.insert(
        "automation_run_id".to_string(),
        payload.automation_run_id.to_string(),
    );
    metadata.insert(
        "automation_rule_id".to_string(),
        payload.automation_rule_id.to_string(),
    );
    metadata.insert(
        "scheduled_for".to_string(),
        payload.scheduled_for.to_rfc3339(),
    );
    metadata.insert("prompt_sha256".to_string(), payload.prompt_sha256);

    Ok(JobActionResult {
        notification: None,
        metadata,
    })
}
