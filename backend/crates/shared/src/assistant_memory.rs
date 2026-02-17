use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::models::AssistantQueryCapability;

pub const ASSISTANT_SESSION_MEMORY_VERSION_V1: &str = "2026-02-16";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssistantSessionMemory {
    pub version: String,
    pub turns: Vec<AssistantSessionTurn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssistantSessionTurn {
    pub user_query_snippet: String,
    pub assistant_summary_snippet: String,
    pub capability: AssistantQueryCapability,
    pub created_at: DateTime<Utc>,
}
