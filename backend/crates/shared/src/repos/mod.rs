use chrono::{DateTime, Utc};
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

use crate::models::ApnsEnvironment;

mod assistant_encrypted_sessions;
mod audit;
mod auth;
mod connectors;
mod devices;
mod jobs;
mod preferences;
mod privacy;
mod users;

pub use assistant_encrypted_sessions::AssistantEncryptedSessionRecord;

const DEFAULT_MEETING_REMINDER_MINUTES: i32 = 15;
const DEFAULT_MORNING_BRIEF_LOCAL_TIME: &str = "08:00";
const DEFAULT_QUIET_HOURS_START: &str = "22:00";
const DEFAULT_QUIET_HOURS_END: &str = "07:00";
const DEFAULT_TIME_ZONE: &str = "UTC";
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
    MeetingReminder,
    MorningBrief,
    UrgentEmailCheck,
}

impl JobType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MeetingReminder => "MEETING_REMINDER",
            Self::MorningBrief => "MORNING_BRIEF",
            Self::UrgentEmailCheck => "URGENT_EMAIL_CHECK",
        }
    }

    fn from_db(value: &str) -> Result<Self, StoreError> {
        match value {
            "MEETING_REMINDER" => Ok(Self::MeetingReminder),
            "MORNING_BRIEF" => Ok(Self::MorningBrief),
            "URGENT_EMAIL_CHECK" => Ok(Self::UrgentEmailCheck),
            _ => Err(StoreError::InvalidData(format!(
                "unknown job type persisted: {value}"
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
}
