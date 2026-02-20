use axum::response::Response;
use shared::enclave::{
    EnclaveRpcExecuteAutomationRequest, EnclaveRpcGenerateMorningBriefRequest,
    EnclaveRpcGenerateUrgentEmailSummaryRequest, EnclaveRpcProcessAssistantQueryRequest,
};

use crate::RuntimeState;

mod automation;
mod mapping;
mod memory;
mod notifications;
mod orchestrator;
mod proactive;
mod query;
mod session_state;

pub(super) async fn process_assistant_query(
    state: RuntimeState,
    request: EnclaveRpcProcessAssistantQueryRequest,
) -> Response {
    query::process_assistant_query(state, request).await
}

pub(super) async fn generate_morning_brief(
    state: RuntimeState,
    request: EnclaveRpcGenerateMorningBriefRequest,
) -> Response {
    proactive::generate_morning_brief(state, request).await
}

pub(super) async fn generate_urgent_email_summary(
    state: RuntimeState,
    request: EnclaveRpcGenerateUrgentEmailSummaryRequest,
) -> Response {
    proactive::generate_urgent_email_summary(state, request).await
}

pub(super) async fn execute_automation(
    state: RuntimeState,
    request: EnclaveRpcExecuteAutomationRequest,
) -> Response {
    automation::execute_automation(state, request).await
}
