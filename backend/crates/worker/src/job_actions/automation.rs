use std::collections::HashMap;

use base64::Engine as _;
use shared::assistant_crypto::ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305;
use shared::enclave::{AutomationRecipientDevice, EnclaveRpcError};
use shared::repos::{ClaimedJob, JobType};

use super::{JobActionContext, JobActionResult};
use crate::{JobExecutionError, automation_runs::AutomationRunJobPayload};

pub(super) async fn resolve_job_action(
    context: &JobActionContext<'_>,
    job: &ClaimedJob,
) -> Result<JobActionResult, JobExecutionError> {
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

    let prompt_envelope = decode_prompt_envelope(payload.prompt_envelope_ciphertext_b64.as_str())
        .map_err(|err| {
        JobExecutionError::permanent("INVALID_AUTOMATION_PROMPT_ENVELOPE", err.to_string())
    })?;

    let devices = context
        .store
        .list_registered_devices(job.user_id)
        .await
        .map_err(|err| {
            JobExecutionError::transient(
                "DEVICE_LOOKUP_FAILED",
                format!("failed to fetch registered devices: {err}"),
            )
        })?;

    let mut recipient_devices = Vec::new();
    let mut missing_key_count = 0_usize;
    let mut unsupported_algorithm_count = 0_usize;
    for device in &devices {
        let Some(key_algorithm) = device.notification_key_algorithm.as_deref() else {
            missing_key_count += 1;
            continue;
        };
        let Some(public_key) = device.notification_public_key.as_deref() else {
            missing_key_count += 1;
            continue;
        };
        if key_algorithm != ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305 {
            unsupported_algorithm_count += 1;
            continue;
        }

        recipient_devices.push(AutomationRecipientDevice {
            device_id: device.device_id.clone(),
            key_id: device.device_id.clone(),
            algorithm: key_algorithm.to_string(),
            public_key: public_key.to_string(),
        });
    }

    let enclave_response = context
        .enclave_client
        .execute_automation_run(
            job.user_id,
            payload.automation_rule_id,
            payload.automation_run_id,
            payload.scheduled_for,
            prompt_envelope,
            recipient_devices,
        )
        .await
        .map_err(map_automation_enclave_error)?;

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
    metadata.insert(
        "registered_device_count".to_string(),
        devices.len().to_string(),
    );
    metadata.insert(
        "recipient_device_count".to_string(),
        enclave_response.notification_artifacts.len().to_string(),
    );
    metadata.insert(
        "recipient_devices_missing_key".to_string(),
        missing_key_count.to_string(),
    );
    metadata.insert(
        "recipient_devices_unsupported_algorithm".to_string(),
        unsupported_algorithm_count.to_string(),
    );
    metadata.insert(
        "automation_should_notify".to_string(),
        enclave_response.should_notify.to_string(),
    );
    metadata.insert(
        "attested_measurement".to_string(),
        enclave_response.attested_identity.measurement.clone(),
    );
    for (key, value) in enclave_response.metadata {
        if is_allowed_enclave_metadata_key(key.as_str()) {
            metadata.insert(key, value);
        }
    }

    Ok(JobActionResult {
        notification: None,
        metadata,
    })
}

fn decode_prompt_envelope(
    encoded: &str,
) -> Result<shared::models::AutomationPromptEnvelope, &'static str> {
    let payload = base64::engine::general_purpose::STANDARD
        .decode(encoded.as_bytes())
        .map_err(|_| "prompt envelope payload must be valid base64")?;

    serde_json::from_slice(payload.as_slice())
        .map_err(|_| "prompt envelope payload must be valid JSON")
}

fn map_automation_enclave_error(err: EnclaveRpcError) -> JobExecutionError {
    match err {
        EnclaveRpcError::RpcContractRejected { .. }
        | EnclaveRpcError::DecryptNotAuthorized { .. }
        | EnclaveRpcError::ConnectorTokenDecryptFailed { .. }
        | EnclaveRpcError::ConnectorTokenUnavailable => JobExecutionError::permanent(
            "AUTOMATION_ENCLAVE_REJECTED",
            "secure enclave rejected automation execution payload",
        ),
        EnclaveRpcError::RpcUnauthorized { .. }
        | EnclaveRpcError::RpcTransportUnavailable { .. }
        | EnclaveRpcError::RpcResponseInvalid { .. }
        | EnclaveRpcError::ProviderRequestUnavailable { .. }
        | EnclaveRpcError::ProviderRequestFailed { .. }
        | EnclaveRpcError::ProviderResponseInvalid { .. } => JobExecutionError::transient(
            "AUTOMATION_ENCLAVE_UNAVAILABLE",
            "secure enclave automation execution unavailable",
        ),
    }
}

fn is_allowed_enclave_metadata_key(key: &str) -> bool {
    matches!(
        key,
        "action_source"
            | "automation_rule_id"
            | "automation_run_id"
            | "scheduled_for"
            | "llm_output_source"
            | "prompt_key_id"
            | "recipient_device_count"
            | "encrypted_artifact_count"
            | "attested_measurement"
    ) || key.starts_with("llm_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_prompt_envelope_rejects_invalid_base64() {
        let err = decode_prompt_envelope("%%%").expect_err("invalid base64 should fail");
        assert_eq!(err, "prompt envelope payload must be valid base64");
    }

    #[test]
    fn map_automation_enclave_error_sanitizes_transport_failures() {
        let mapped = map_automation_enclave_error(EnclaveRpcError::RpcTransportUnavailable {
            message: "authorization header leaked".to_string(),
        });
        assert_eq!(mapped.code, "AUTOMATION_ENCLAVE_UNAVAILABLE");
        assert_eq!(
            mapped.message,
            "secure enclave automation execution unavailable"
        );
    }

    #[test]
    fn is_allowed_enclave_metadata_key_only_allows_expected_keys() {
        assert!(is_allowed_enclave_metadata_key("llm_provider"));
        assert!(is_allowed_enclave_metadata_key("attested_measurement"));
        assert!(!is_allowed_enclave_metadata_key("notification_title"));
    }
}
