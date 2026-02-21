use chrono::{DateTime, Utc};
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

use crate::automation_schedule::{AutomationScheduleSpec, AutomationScheduleType};
use crate::models::ApnsEnvironment;

mod assistant_encrypted_sessions;
mod audit;
mod auth;
mod automation;
mod automation_runs;
mod connectors;
mod devices;
mod jobs;
mod privacy;
mod users;

pub use assistant_encrypted_sessions::AssistantEncryptedSessionMetadataRecord;
pub use assistant_encrypted_sessions::AssistantEncryptedSessionRecord;

pub const LEGACY_CONNECTOR_TOKEN_KEY_ID: &str = "__legacy__";

#[derive(Debug, Clone)]
pub enum AuditResult {
    Success,
    Failure,
}

impl AuditResult {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Success => "SUCCESS",
            Self::Failure => "FAILURE",
        }
    }
}

#[derive(Debug, Clone)]
pub enum JobType {
    AutomationRun,
}

impl JobType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AutomationRun => "AUTOMATION_RUN",
        }
    }

    fn from_db(value: &str) -> Result<Self, StoreError> {
        match value {
            "AUTOMATION_RUN" => Ok(Self::AutomationRun),
            _ => Err(StoreError::InvalidData(format!(
                "unknown job type persisted: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutomationRuleStatus {
    Active,
    Paused,
}

impl AutomationRuleStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::Paused => "PAUSED",
        }
    }

    fn from_db(value: &str) -> Result<Self, StoreError> {
        match value {
            "ACTIVE" => Ok(Self::Active),
            "PAUSED" => Ok(Self::Paused),
            _ => Err(StoreError::InvalidData(format!(
                "unknown automation rule status persisted: {value}"
            ))),
        }
    }
}

impl AutomationScheduleType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Daily => "DAILY",
            Self::Weekly => "WEEKLY",
            Self::Monthly => "MONTHLY",
            Self::Annually => "ANNUALLY",
        }
    }

    fn from_db(value: &str) -> Result<Self, StoreError> {
        match value {
            "DAILY" => Ok(Self::Daily),
            "WEEKLY" => Ok(Self::Weekly),
            "MONTHLY" => Ok(Self::Monthly),
            "ANNUALLY" => Ok(Self::Annually),
            _ => Err(StoreError::InvalidData(format!(
                "unknown automation schedule type persisted: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutomationRunState {
    Materialized,
    Enqueued,
    Failed,
}

impl AutomationRunState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Materialized => "MATERIALIZED",
            Self::Enqueued => "ENQUEUED",
            Self::Failed => "FAILED",
        }
    }

    fn from_db(value: &str) -> Result<Self, StoreError> {
        match value {
            "MATERIALIZED" => Ok(Self::Materialized),
            "ENQUEUED" => Ok(Self::Enqueued),
            "FAILED" => Ok(Self::Failed),
            _ => Err(StoreError::InvalidData(format!(
                "unknown automation run state persisted: {value}"
            ))),
        }
    }
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("invalid cursor")]
    InvalidCursor,
    #[error("invalid persisted data: {0}")]
    InvalidData(String),
}

#[derive(Clone)]
pub struct Store {
    pool: PgPool,
    data_encryption_key: String,
}

#[derive(Debug, Clone)]
pub struct ConnectorKeyMetadata {
    pub provider: String,
    pub token_key_id: String,
    pub token_version: i32,
}

#[derive(Debug, Clone)]
pub struct ActiveConnectorMetadata {
    pub connector_id: Uuid,
    pub provider: String,
    pub token_key_id: String,
    pub token_version: i32,
}

#[derive(Debug, Clone)]
pub struct ConnectorStateRecord {
    pub connector_id: Uuid,
    pub provider: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct ClaimedJob {
    pub id: Uuid,
    pub user_id: Uuid,
    pub job_type: JobType,
    pub due_at: DateTime<Utc>,
    pub payload_ciphertext: Option<Vec<u8>>,
    pub attempts: i32,
    pub max_attempts: i32,
    pub idempotency_key: String,
}

#[derive(Debug, Clone)]
pub struct AutomationRuleRecord {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub status: AutomationRuleStatus,
    pub schedule_type: AutomationScheduleType,
    pub local_time_minutes: i32,
    pub anchor_day_of_week: Option<i16>,
    pub anchor_day_of_month: Option<i16>,
    pub anchor_month: Option<i16>,
    pub time_zone: String,
    pub next_run_at: DateTime<Utc>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub prompt_sha256: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ClaimedAutomationRule {
    pub id: Uuid,
    pub user_id: Uuid,
    pub schedule_type: AutomationScheduleType,
    pub local_time_minutes: i32,
    pub anchor_day_of_week: Option<i16>,
    pub anchor_day_of_month: Option<i16>,
    pub anchor_month: Option<i16>,
    pub time_zone: String,
    pub next_run_at: DateTime<Utc>,
    pub prompt_ciphertext: Vec<u8>,
    pub prompt_sha256: String,
}

#[derive(Debug, Clone)]
pub struct AutomationPromptMaterial {
    pub prompt_ciphertext: Vec<u8>,
    pub prompt_sha256: String,
}

#[derive(Debug, Clone)]
pub struct AutomationRunRecord {
    pub id: Uuid,
    pub rule_id: Uuid,
    pub user_id: Uuid,
    pub scheduled_for: DateTime<Utc>,
    pub job_id: Option<Uuid>,
    pub idempotency_key: String,
    pub state: AutomationRunState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum PrivacyDeleteStatus {
    Queued,
    Running,
    Completed,
    Failed,
}

impl PrivacyDeleteStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "QUEUED",
            Self::Running => "RUNNING",
            Self::Completed => "COMPLETED",
            Self::Failed => "FAILED",
        }
    }

    fn from_db(value: &str) -> Result<Self, StoreError> {
        match value {
            "QUEUED" => Ok(Self::Queued),
            "RUNNING" => Ok(Self::Running),
            "COMPLETED" => Ok(Self::Completed),
            "FAILED" => Ok(Self::Failed),
            _ => Err(StoreError::InvalidData(format!(
                "unknown privacy delete status persisted: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ClaimedDeleteRequest {
    pub id: Uuid,
    pub user_id: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct PrivacyDeleteRequestStatus {
    pub id: Uuid,
    pub status: PrivacyDeleteStatus,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub failed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct DeviceRegistration {
    pub device_id: String,
    pub apns_token: String,
    pub environment: ApnsEnvironment,
    pub notification_key_algorithm: Option<String>,
    pub notification_public_key: Option<String>,
}

impl AutomationRuleRecord {
    pub fn schedule_spec(&self) -> Result<AutomationScheduleSpec, StoreError> {
        automation_schedule_spec_from_fields(
            self.schedule_type,
            self.time_zone.as_str(),
            self.local_time_minutes,
            self.anchor_day_of_week,
            self.anchor_day_of_month,
            self.anchor_month,
        )
    }
}

impl ClaimedAutomationRule {
    pub fn schedule_spec(&self) -> Result<AutomationScheduleSpec, StoreError> {
        automation_schedule_spec_from_fields(
            self.schedule_type,
            self.time_zone.as_str(),
            self.local_time_minutes,
            self.anchor_day_of_week,
            self.anchor_day_of_month,
            self.anchor_month,
        )
    }
}

fn automation_schedule_spec_from_fields(
    schedule_type: AutomationScheduleType,
    time_zone: &str,
    local_time_minutes: i32,
    anchor_day_of_week: Option<i16>,
    anchor_day_of_month: Option<i16>,
    anchor_month: Option<i16>,
) -> Result<AutomationScheduleSpec, StoreError> {
    let local_time_minutes = u16::try_from(local_time_minutes)
        .map_err(|_| StoreError::InvalidData("local_time_minutes must be >= 0".to_string()))?;
    let anchor_day_of_week = option_i16_to_u8(anchor_day_of_week, "anchor_day_of_week")?;
    let anchor_day_of_month = option_i16_to_u8(anchor_day_of_month, "anchor_day_of_month")?;
    let anchor_month = option_i16_to_u8(anchor_month, "anchor_month")?;

    Ok(AutomationScheduleSpec {
        schedule_type,
        time_zone: time_zone.to_string(),
        local_time_minutes,
        anchor_day_of_week,
        anchor_day_of_month,
        anchor_month,
    })
}

fn option_i16_to_u8(value: Option<i16>, field: &str) -> Result<Option<u8>, StoreError> {
    value
        .map(|inner| {
            u8::try_from(inner).map_err(|_| {
                StoreError::InvalidData(format!("{field} could not be represented as u8"))
            })
        })
        .transpose()
}
