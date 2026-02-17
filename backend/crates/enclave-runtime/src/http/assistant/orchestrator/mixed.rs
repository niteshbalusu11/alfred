use shared::models::{AssistantQueryCapability, AssistantStructuredPayload};

use super::{AssistantOrchestratorResult, local_attested_identity};
use crate::RuntimeState;

pub(super) fn execute_mixed_query(
    state: &RuntimeState,
    _query: &str,
) -> AssistantOrchestratorResult {
    let summary = "Mixed intent detected (calendar + email). Mixed tool execution scaffolding is now in place and will be expanded in follow-up issues.".to_string();

    AssistantOrchestratorResult {
        capability: AssistantQueryCapability::Mixed,
        display_text: summary.clone(),
        payload: AssistantStructuredPayload {
            title: "Mixed lookup".to_string(),
            summary,
            key_points: vec![
                "Query planner selected mixed capability.".to_string(),
                "Dedicated calendar/email tool execution layers are now separated in orchestrator."
                    .to_string(),
            ],
            follow_ups: vec![
                "Ask one focused calendar question for now.".to_string(),
                "Ask one focused email question for now.".to_string(),
            ],
        },
        attested_identity: local_attested_identity(state),
    }
}
