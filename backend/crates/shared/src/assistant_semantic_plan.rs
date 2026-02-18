use chrono::{DateTime, Duration, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::models::AssistantQueryCapability;

pub const ASSISTANT_SEMANTIC_PLAN_VERSION_V1: &str = "2026-02-18";
const MIN_LOOKBACK_DAYS: u16 = 1;
const MAX_LOOKBACK_DAYS: u16 = 30;
const DEFAULT_LOOKBACK_DAYS: u16 = 7;
const MAX_TIME_WINDOW_SPAN_DAYS: i64 = 31;
const MAX_LANGUAGE_CHARS: usize = 16;
const MAX_CLARIFYING_QUESTION_CHARS: usize = 240;
const MAX_SENDER_CHARS: usize = 160;
const MAX_KEYWORD_CHARS: usize = 48;
const MAX_KEYWORDS: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssistantSemanticCapability {
    CalendarLookup,
    EmailLookup,
    Mixed,
    GeneralChat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssistantTimeWindowResolutionSource {
    ExplicitDate,
    RelativeDate,
    FollowUpContext,
    DefaultWindow,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssistantSemanticTimeWindowOutput {
    pub start: String,
    pub end: String,
    pub timezone: String,
    pub resolution_source: AssistantTimeWindowResolutionSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssistantSemanticEmailFiltersOutput {
    #[serde(default)]
    pub sender: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub lookback_days: Option<u16>,
    #[serde(default)]
    pub unread_only: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssistantSemanticPlanOutput {
    pub capabilities: Vec<AssistantSemanticCapability>,
    pub confidence: f64,
    #[serde(default)]
    pub needs_clarification: bool,
    #[serde(default)]
    pub clarifying_question: Option<String>,
    #[serde(default)]
    pub time_window: Option<AssistantSemanticTimeWindowOutput>,
    #[serde(default)]
    pub email_filters: Option<AssistantSemanticEmailFiltersOutput>,
    #[serde(default)]
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssistantSemanticPlanContract {
    pub version: String,
    pub output: AssistantSemanticPlanOutput,
}

#[derive(Debug, Clone)]
pub struct AssistantSemanticPlan {
    pub capabilities: Vec<AssistantQueryCapability>,
    pub confidence: f32,
    pub needs_clarification: bool,
    pub clarifying_question: Option<String>,
    pub time_window: Option<AssistantSemanticTimeWindow>,
    pub email_filters: Option<AssistantSemanticEmailFilters>,
    pub language: Option<String>,
    pub planned_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct AssistantSemanticTimeWindow {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub timezone: String,
    pub resolution_source: AssistantTimeWindowResolutionSource,
}

#[derive(Debug, Clone)]
pub struct AssistantSemanticEmailFilters {
    pub sender: Option<String>,
    pub keywords: Vec<String>,
    pub lookback_days: u16,
    pub unread_only: bool,
}

#[derive(Debug, Error)]
pub enum AssistantSemanticPlanNormalizationError {
    #[error(
        "semantic planner contract version mismatch: expected={ASSISTANT_SEMANTIC_PLAN_VERSION_V1}, actual={0}"
    )]
    ContractVersionMismatch(String),
    #[error("semantic planner confidence must be a finite number between 0.0 and 1.0")]
    InvalidConfidence,
    #[error("semantic planner requires clarifying_question when needs_clarification is true")]
    MissingClarifyingQuestion,
    #[error("semantic planner time_window start/end must be valid RFC3339 timestamps")]
    InvalidTimeWindowTimestamp,
    #[error("semantic planner time_window end must be after start")]
    InvalidTimeWindowOrder,
    #[error("semantic planner time_window exceeds {MAX_TIME_WINDOW_SPAN_DAYS} days")]
    TimeWindowExceedsBounds,
}

pub fn normalize_semantic_plan_contract(
    contract: AssistantSemanticPlanContract,
    user_time_zone: &str,
    now: DateTime<Utc>,
) -> Result<AssistantSemanticPlan, AssistantSemanticPlanNormalizationError> {
    if contract.version != ASSISTANT_SEMANTIC_PLAN_VERSION_V1 {
        return Err(
            AssistantSemanticPlanNormalizationError::ContractVersionMismatch(contract.version),
        );
    }

    normalize_semantic_plan_output(contract.output, user_time_zone, now)
}

pub fn normalize_semantic_plan_output(
    output: AssistantSemanticPlanOutput,
    user_time_zone: &str,
    now: DateTime<Utc>,
) -> Result<AssistantSemanticPlan, AssistantSemanticPlanNormalizationError> {
    if !output.confidence.is_finite() || !(0.0..=1.0).contains(&output.confidence) {
        return Err(AssistantSemanticPlanNormalizationError::InvalidConfidence);
    }

    let capabilities = normalize_capabilities(&output.capabilities);
    let needs_clarification = output.needs_clarification;
    let clarifying_question = normalize_optional_text(
        output.clarifying_question.as_deref(),
        MAX_CLARIFYING_QUESTION_CHARS,
    );

    if needs_clarification && clarifying_question.is_none() {
        return Err(AssistantSemanticPlanNormalizationError::MissingClarifyingQuestion);
    }

    let time_window = match output.time_window {
        Some(window) => Some(normalize_time_window(window, user_time_zone)?),
        None => None,
    };
    let email_filters = output.email_filters.map(normalize_email_filters);
    let language = normalize_language_hint(output.language.as_deref());

    Ok(AssistantSemanticPlan {
        capabilities,
        confidence: output.confidence as f32,
        needs_clarification,
        clarifying_question,
        time_window,
        email_filters,
        language,
        planned_at: now,
    })
}

fn normalize_capabilities(
    capabilities: &[AssistantSemanticCapability],
) -> Vec<AssistantQueryCapability> {
    let mut has_calendar = false;
    let mut has_email = false;
    let mut has_mixed = false;
    let mut has_chat = false;

    for capability in capabilities {
        match capability {
            AssistantSemanticCapability::CalendarLookup => has_calendar = true,
            AssistantSemanticCapability::EmailLookup => has_email = true,
            AssistantSemanticCapability::Mixed => has_mixed = true,
            AssistantSemanticCapability::GeneralChat => has_chat = true,
        }
    }

    if has_mixed || (has_calendar && has_email) {
        return vec![AssistantQueryCapability::Mixed];
    }
    if has_calendar {
        return vec![AssistantQueryCapability::CalendarLookup];
    }
    if has_email {
        return vec![AssistantQueryCapability::EmailLookup];
    }
    if has_chat {
        return vec![AssistantQueryCapability::GeneralChat];
    }

    vec![AssistantQueryCapability::GeneralChat]
}

fn normalize_time_window(
    output: AssistantSemanticTimeWindowOutput,
    user_time_zone: &str,
) -> Result<AssistantSemanticTimeWindow, AssistantSemanticPlanNormalizationError> {
    let start = DateTime::parse_from_rfc3339(output.start.as_str())
        .map_err(|_| AssistantSemanticPlanNormalizationError::InvalidTimeWindowTimestamp)?
        .with_timezone(&Utc);
    let end = DateTime::parse_from_rfc3339(output.end.as_str())
        .map_err(|_| AssistantSemanticPlanNormalizationError::InvalidTimeWindowTimestamp)?
        .with_timezone(&Utc);

    if end <= start {
        return Err(AssistantSemanticPlanNormalizationError::InvalidTimeWindowOrder);
    }
    if end - start > Duration::days(MAX_TIME_WINDOW_SPAN_DAYS) {
        return Err(AssistantSemanticPlanNormalizationError::TimeWindowExceedsBounds);
    }

    let timezone = normalize_optional_text(Some(output.timezone.as_str()), 64)
        .unwrap_or_else(|| user_time_zone.to_string());

    Ok(AssistantSemanticTimeWindow {
        start,
        end,
        timezone,
        resolution_source: output.resolution_source,
    })
}

fn normalize_email_filters(
    output: AssistantSemanticEmailFiltersOutput,
) -> AssistantSemanticEmailFilters {
    let sender = normalize_optional_text(output.sender.as_deref(), MAX_SENDER_CHARS);
    let keywords = output
        .keywords
        .iter()
        .filter_map(|keyword| normalize_optional_text(Some(keyword.as_str()), MAX_KEYWORD_CHARS))
        .take(MAX_KEYWORDS)
        .collect::<Vec<_>>();
    let lookback_days = output
        .lookback_days
        .unwrap_or(DEFAULT_LOOKBACK_DAYS)
        .clamp(MIN_LOOKBACK_DAYS, MAX_LOOKBACK_DAYS);

    AssistantSemanticEmailFilters {
        sender,
        keywords,
        lookback_days,
        unread_only: output.unread_only.unwrap_or(false),
    }
}

fn normalize_language_hint(value: Option<&str>) -> Option<String> {
    let candidate = normalize_optional_text(value, MAX_LANGUAGE_CHARS)?;
    if candidate
        .chars()
        .all(|c| c.is_ascii_alphabetic() || c == '-')
    {
        Some(candidate.to_ascii_lowercase())
    } else {
        None
    }
}

fn normalize_optional_text(value: Option<&str>, max_chars: usize) -> Option<String> {
    let trimmed = value?.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.chars().take(max_chars).collect())
}

#[cfg(test)]
mod tests;
