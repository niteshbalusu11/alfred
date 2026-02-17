use shared::config::WorkerConfig;
use shared::repos::{AuditResult, Store};
use shared::security::SecretRuntime;

use super::super::JobActionResult;
use super::ai_observability::record_ai_audit_event;
use super::session::{build_enclave_client, build_google_session};
use crate::{JobExecutionError, NotificationContent};

const URGENT_EMAIL_CANDIDATE_MAX_RESULTS: usize = 10;

pub(super) async fn build_urgent_email_alert(
    store: &Store,
    config: &WorkerConfig,
    secret_runtime: &SecretRuntime,
    oauth_client: &reqwest::Client,
    user_id: uuid::Uuid,
) -> Result<JobActionResult, JobExecutionError> {
    let session =
        build_google_session(store, config, secret_runtime, oauth_client, user_id).await?;
    let enclave_client = build_enclave_client(config, oauth_client);

    let response = enclave_client
        .generate_urgent_email_summary(
            user_id,
            session.connector_request,
            URGENT_EMAIL_CANDIDATE_MAX_RESULTS,
        )
        .await
        .map_err(map_enclave_orchestration_error)?;

    let mut metadata = response.metadata;
    metadata
        .entry("attested_measurement".to_string())
        .or_insert(response.attested_identity.measurement);

    let audit_result = if metadata
        .get("llm_request_outcome")
        .map(String::as_str)
        .is_some_and(|value| value == "success")
        && metadata
            .get("llm_output_source")
            .map(String::as_str)
            .is_some_and(|value| value == "model_output")
    {
        AuditResult::Success
    } else {
        AuditResult::Failure
    };

    record_ai_audit_event(
        store,
        user_id,
        "AI_WORKER_URGENT_EMAIL_OUTPUT",
        audit_result,
        &metadata,
    )
    .await;

    if !response.should_notify {
        metadata.insert("reason".to_string(), "llm_marked_not_urgent".to_string());
        return Ok(JobActionResult {
            notification: None,
            metadata,
        });
    }

    let notification = response.notification.ok_or_else(|| {
        JobExecutionError::permanent(
            "URGENT_EMAIL_MISSING_NOTIFICATION",
            "enclave urgent email response was missing notification content",
        )
    })?;

    Ok(JobActionResult {
        notification: Some(NotificationContent {
            title: notification.title,
            body: notification.body,
        }),
        metadata,
    })
}

fn map_enclave_orchestration_error(err: shared::enclave::EnclaveRpcError) -> JobExecutionError {
    use shared::enclave::{EnclaveRpcError, ProviderOperation};

    match err {
        EnclaveRpcError::DecryptNotAuthorized { .. } => JobExecutionError::permanent(
            "CONNECTOR_DECRYPT_NOT_AUTHORIZED",
            "connector decrypt authorization failed",
        ),
        EnclaveRpcError::ConnectorTokenDecryptFailed { .. } => JobExecutionError::transient(
            "CONNECTOR_TOKEN_DECRYPT_FAILED",
            "failed to decrypt connector token in enclave",
        ),
        EnclaveRpcError::ConnectorTokenUnavailable => JobExecutionError::permanent(
            "CONNECTOR_TOKEN_MISSING",
            "refresh token was unavailable for active connector",
        ),
        EnclaveRpcError::ProviderRequestUnavailable { operation, .. } => match operation {
            ProviderOperation::TokenRefresh | ProviderOperation::OAuthCodeExchange => {
                JobExecutionError::transient(
                    "GOOGLE_TOKEN_REFRESH_UNAVAILABLE",
                    "google token refresh request failed",
                )
            }
            ProviderOperation::CalendarFetch | ProviderOperation::GmailFetch => {
                JobExecutionError::transient(
                    "GOOGLE_PROVIDER_UNAVAILABLE",
                    "provider request failed",
                )
            }
            ProviderOperation::TokenRevoke
            | ProviderOperation::AssistantAttestedKey
            | ProviderOperation::AssistantQuery
            | ProviderOperation::AssistantMorningBrief
            | ProviderOperation::AssistantUrgentEmail => JobExecutionError::transient(
                "ENCLAVE_ORCHESTRATION_UNAVAILABLE",
                "enclave orchestration request failed",
            ),
        },
        EnclaveRpcError::ProviderRequestFailed {
            operation, status, ..
        } => {
            let message = format!("provider request failed with HTTP {status}");
            match operation {
                ProviderOperation::TokenRefresh | ProviderOperation::OAuthCodeExchange => {
                    JobExecutionError::transient("GOOGLE_TOKEN_REFRESH_FAILED", message)
                }
                ProviderOperation::CalendarFetch | ProviderOperation::GmailFetch => {
                    JobExecutionError::transient("GOOGLE_PROVIDER_FAILED", message)
                }
                ProviderOperation::TokenRevoke
                | ProviderOperation::AssistantAttestedKey
                | ProviderOperation::AssistantQuery
                | ProviderOperation::AssistantMorningBrief
                | ProviderOperation::AssistantUrgentEmail => {
                    JobExecutionError::transient("ENCLAVE_ORCHESTRATION_FAILED", message)
                }
            }
        }
        EnclaveRpcError::ProviderResponseInvalid { operation, .. } => match operation {
            ProviderOperation::TokenRefresh | ProviderOperation::OAuthCodeExchange => {
                JobExecutionError::transient(
                    "GOOGLE_TOKEN_REFRESH_PARSE_FAILED",
                    "google token refresh response was invalid",
                )
            }
            ProviderOperation::CalendarFetch | ProviderOperation::GmailFetch => {
                JobExecutionError::transient(
                    "GOOGLE_PROVIDER_PARSE_FAILED",
                    "provider response was invalid",
                )
            }
            ProviderOperation::TokenRevoke
            | ProviderOperation::AssistantAttestedKey
            | ProviderOperation::AssistantQuery
            | ProviderOperation::AssistantMorningBrief
            | ProviderOperation::AssistantUrgentEmail => JobExecutionError::transient(
                "ENCLAVE_ORCHESTRATION_PARSE_FAILED",
                "enclave orchestration response was invalid",
            ),
        },
        EnclaveRpcError::RpcUnauthorized { code }
        | EnclaveRpcError::RpcContractRejected { code } => JobExecutionError::permanent(
            "ENCLAVE_RPC_REJECTED",
            format!("secure enclave rpc request rejected: {code}"),
        ),
        EnclaveRpcError::RpcTransportUnavailable { .. }
        | EnclaveRpcError::RpcResponseInvalid { .. } => JobExecutionError::transient(
            "ENCLAVE_RPC_UNAVAILABLE",
            "secure enclave rpc unavailable",
        ),
    }
}
