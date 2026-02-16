use axum::response::Response;
use shared::enclave::{
    EnclaveRpcGenerateMorningBriefRequest, EnclaveRpcGenerateUrgentEmailSummaryRequest,
    EnclaveRpcProcessAssistantQueryRequest,
};

use crate::RuntimeState;

mod mapping;
mod memory;
mod notifications;
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
