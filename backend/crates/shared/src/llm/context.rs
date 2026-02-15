use std::cmp::Ordering;
use std::collections::BTreeSet;

use chrono::{DateTime, NaiveDate, SecondsFormat, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONTEXT_CONTRACT_VERSION_V1: &str = "2026-02-15";

const DEFAULT_MORNING_BRIEF_LOCAL_TIME: &str = "08:00";
const MAX_MEETINGS: usize = 20;
const MAX_EMAIL_CANDIDATES: usize = 20;
const MAX_ATTENDEE_COUNT: usize = 50;
const MAX_LABELS: usize = 8;
const MAX_REF_CHARS: usize = 80;
const MAX_TITLE_CHARS: usize = 120;
const MAX_SUBJECT_CHARS: usize = 120;
const MAX_SENDER_CHARS: usize = 120;
const MAX_SNIPPET_CHARS: usize = 280;
const MAX_LABEL_CHARS: usize = 32;
const MAX_LOCAL_TIME_CHARS: usize = 16;

#[derive(Debug, Clone, Default)]
pub struct GoogleCalendarMeetingSource {
    pub event_id: Option<String>,
    pub title: Option<String>,
    pub start_at: Option<DateTime<Utc>>,
    pub end_at: Option<DateTime<Utc>>,
    pub attendee_emails: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct GoogleEmailCandidateSource {
    pub message_id: Option<String>,
    pub from: Option<String>,
    pub subject: Option<String>,
    pub snippet: Option<String>,
    pub received_at: Option<DateTime<Utc>>,
    pub label_ids: Vec<String>,
    pub has_attachments: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MeetingsTodayContext {
    pub version: String,
    pub calendar_day: String,
    pub meeting_count: usize,
    pub meetings: Vec<MeetingContextEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MeetingContextEntry {
    pub event_ref: String,
    pub title: String,
    pub start_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_minutes: Option<u32>,
    pub attendee_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UrgentEmailCandidatesContext {
    pub version: String,
    pub candidate_count: usize,
    pub candidates: Vec<UrgentEmailCandidateContextEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UrgentEmailCandidateContextEntry {
    pub message_ref: String,
    pub from: String,
    pub subject: String,
    pub snippet: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub received_at: Option<String>,
    pub labels: Vec<String>,
    pub has_attachments: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MorningBriefContext {
    pub version: String,
    pub local_date: String,
    pub morning_brief_local_time: String,
    pub meetings_today_count: usize,
    pub urgent_email_candidate_count: usize,
    pub meetings_today: Vec<MeetingContextEntry>,
    pub urgent_email_candidates: Vec<UrgentEmailCandidateContextEntry>,
}

pub fn assemble_meetings_today_context(
    calendar_day: NaiveDate,
    meetings: &[GoogleCalendarMeetingSource],
) -> MeetingsTodayContext {
    let mut normalized: Vec<NormalizedMeeting> = meetings
        .iter()
        .filter_map(|meeting| normalize_meeting(calendar_day, meeting))
        .collect();

    normalized.sort_by(|left, right| {
        left.start_at
            .cmp(&right.start_at)
            .then_with(|| left.event_ref.cmp(&right.event_ref))
            .then_with(|| left.title.cmp(&right.title))
    });

    let mut fallback_index = 0usize;
    let meetings = normalized
        .into_iter()
        .take(MAX_MEETINGS)
        .map(|meeting| {
            let event_ref = meeting.event_ref.unwrap_or_else(|| {
                fallback_index += 1;
                format!("meeting-{fallback_index:03}")
            });
            let duration_minutes = meeting
                .end_at
                .and_then(|end_at| positive_minutes(end_at - meeting.start_at));

            MeetingContextEntry {
                event_ref,
                title: meeting.title,
                start_at: format_datetime(meeting.start_at),
                end_at: meeting.end_at.map(format_datetime),
                duration_minutes,
                attendee_count: meeting.attendee_count,
            }
        })
        .collect::<Vec<_>>();

    MeetingsTodayContext {
        version: CONTEXT_CONTRACT_VERSION_V1.to_string(),
        calendar_day: calendar_day.to_string(),
        meeting_count: meetings.len(),
        meetings,
    }
}

pub fn assemble_urgent_email_candidates_context(
    candidates: &[GoogleEmailCandidateSource],
) -> UrgentEmailCandidatesContext {
    let mut normalized = candidates
        .iter()
        .map(normalize_email_candidate)
        .collect::<Vec<_>>();

    normalized.sort_by(|left, right| {
        compare_received_at_desc(left.received_at, right.received_at)
            .then_with(|| left.message_ref.cmp(&right.message_ref))
            .then_with(|| left.subject.cmp(&right.subject))
    });

    let mut fallback_index = 0usize;
    let candidates = normalized
        .into_iter()
        .take(MAX_EMAIL_CANDIDATES)
        .map(|candidate| {
            let message_ref = candidate.message_ref.unwrap_or_else(|| {
                fallback_index += 1;
                format!("email-{fallback_index:03}")
            });

            UrgentEmailCandidateContextEntry {
                message_ref,
                from: candidate.from,
                subject: candidate.subject,
                snippet: candidate.snippet,
                received_at: candidate.received_at.map(format_datetime),
                labels: candidate.labels,
                has_attachments: candidate.has_attachments,
            }
        })
        .collect::<Vec<_>>();

    UrgentEmailCandidatesContext {
        version: CONTEXT_CONTRACT_VERSION_V1.to_string(),
        candidate_count: candidates.len(),
        candidates,
    }
}

pub fn assemble_morning_brief_context(
    local_date: NaiveDate,
    morning_brief_local_time: &str,
    meetings: &[GoogleCalendarMeetingSource],
    urgent_email_candidates: &[GoogleEmailCandidateSource],
) -> MorningBriefContext {
    let meetings_today_context = assemble_meetings_today_context(local_date, meetings);
    let urgent_email_context = assemble_urgent_email_candidates_context(urgent_email_candidates);

    MorningBriefContext {
        version: CONTEXT_CONTRACT_VERSION_V1.to_string(),
        local_date: local_date.to_string(),
        morning_brief_local_time: normalize_local_time(morning_brief_local_time),
        meetings_today_count: meetings_today_context.meeting_count,
        urgent_email_candidate_count: urgent_email_context.candidate_count,
        meetings_today: meetings_today_context.meetings,
        urgent_email_candidates: urgent_email_context.candidates,
    }
}

#[derive(Debug)]
struct NormalizedMeeting {
    event_ref: Option<String>,
    title: String,
    start_at: DateTime<Utc>,
    end_at: Option<DateTime<Utc>>,
    attendee_count: usize,
}

fn normalize_meeting(
    calendar_day: NaiveDate,
    meeting: &GoogleCalendarMeetingSource,
) -> Option<NormalizedMeeting> {
    let start_at = meeting.start_at?;
    if start_at.date_naive() != calendar_day {
        return None;
    }

    let attendee_count = meeting
        .attendee_emails
        .iter()
        .filter_map(|email| normalize_identifier(Some(email.as_str()), MAX_REF_CHARS))
        .collect::<BTreeSet<_>>()
        .len()
        .min(MAX_ATTENDEE_COUNT);
    let end_at = meeting.end_at.filter(|end_at| *end_at > start_at);

    Some(NormalizedMeeting {
        event_ref: normalize_identifier(meeting.event_id.as_deref(), MAX_REF_CHARS),
        title: normalize_text(
            meeting.title.as_deref(),
            "Untitled meeting",
            MAX_TITLE_CHARS,
        ),
        start_at,
        end_at,
        attendee_count,
    })
}

#[derive(Debug)]
struct NormalizedEmailCandidate {
    message_ref: Option<String>,
    from: String,
    subject: String,
    snippet: String,
    received_at: Option<DateTime<Utc>>,
    labels: Vec<String>,
    has_attachments: bool,
}

fn normalize_email_candidate(candidate: &GoogleEmailCandidateSource) -> NormalizedEmailCandidate {
    let labels = candidate
        .label_ids
        .iter()
        .filter_map(|label| normalize_identifier(Some(label.as_str()), MAX_LABEL_CHARS))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .take(MAX_LABELS)
        .collect::<Vec<_>>();

    NormalizedEmailCandidate {
        message_ref: normalize_identifier(candidate.message_id.as_deref(), MAX_REF_CHARS),
        from: normalize_text(
            candidate.from.as_deref(),
            "unknown sender",
            MAX_SENDER_CHARS,
        ),
        subject: normalize_text(
            candidate.subject.as_deref(),
            "(no subject)",
            MAX_SUBJECT_CHARS,
        ),
        snippet: normalize_text(candidate.snippet.as_deref(), "", MAX_SNIPPET_CHARS),
        received_at: candidate.received_at,
        labels,
        has_attachments: candidate.has_attachments,
    }
}

fn positive_minutes(duration: chrono::Duration) -> Option<u32> {
    let minutes = duration.num_minutes();
    if minutes > 0 {
        return Some(minutes as u32);
    }

    None
}

fn compare_received_at_desc(left: Option<DateTime<Utc>>, right: Option<DateTime<Utc>>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => right.cmp(&left),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn normalize_local_time(value: &str) -> String {
    let compact = collapse_whitespace(value);
    if compact.is_empty() {
        return DEFAULT_MORNING_BRIEF_LOCAL_TIME.to_string();
    }

    truncate_chars(&compact, MAX_LOCAL_TIME_CHARS)
}

fn normalize_text(value: Option<&str>, fallback: &str, max_chars: usize) -> String {
    let compact = collapse_whitespace(value.unwrap_or(""));
    if compact.is_empty() {
        return fallback.to_string();
    }

    truncate_chars(&compact, max_chars)
}

fn normalize_identifier(value: Option<&str>, max_chars: usize) -> Option<String> {
    let compact = collapse_whitespace(value.unwrap_or(""));
    if compact.is_empty() {
        return None;
    }

    Some(truncate_chars(&compact, max_chars))
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn format_datetime(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Secs, true)
}
