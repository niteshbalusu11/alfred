use shared::models::{AssistantQueryCapability, AssistantStructuredPayload};

use super::{AssistantOrchestratorResult, local_attested_identity};
use crate::RuntimeState;

pub(super) fn execute_email_query(
    state: &RuntimeState,
    _query: &str,
) -> AssistantOrchestratorResult {
    let summary =
        "Email lookup routing is selected. Detailed inbox retrieval lands in the next execution issue.".to_string();

    AssistantOrchestratorResult {
        capability: AssistantQueryCapability::EmailLookup,
        display_text: summary.clone(),
        payload: AssistantStructuredPayload {
            title: "Email lookup".to_string(),
            summary,
            key_points: vec![
                "Intent was classified as email-focused.".to_string(),
                "Enclave orchestrator now supports dedicated email capability routing.".to_string(),
            ],
            follow_ups: vec![
                "Try: summarize my inbox for today".to_string(),
                "Try: any emails from finance this week".to_string(),
            ],
        },
        attested_identity: local_attested_identity(state),
    }
}
