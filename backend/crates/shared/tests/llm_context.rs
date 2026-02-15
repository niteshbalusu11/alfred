use chrono::{DateTime, NaiveDate, Utc};
use serde_json::Value;
use shared::llm::{
    GoogleCalendarMeetingSource, GoogleEmailCandidateSource, assemble_meetings_today_context,
    assemble_morning_brief_context, assemble_urgent_email_candidates_context,
};

#[test]
fn meetings_today_context_is_deterministic_and_matches_fixture() {
    let calendar_day = date("2026-02-15");
    let meetings = sample_meetings_unsorted();
    let mut reversed = meetings.clone();
    reversed.reverse();

    let context = assemble_meetings_today_context(calendar_day, &meetings);
    let reversed_context = assemble_meetings_today_context(calendar_day, &reversed);

    assert_eq!(context, reversed_context);
    assert_eq!(
        serde_json::to_value(context).expect("context should serialize"),
        meetings_fixture()
    );
}

#[test]
fn urgent_email_context_matches_fixture_and_excludes_sensitive_source_fields() {
    let candidates = sample_email_candidates_unsorted();
    let context = assemble_urgent_email_candidates_context(&candidates);
    let encoded = serde_json::to_string(&context).expect("context should encode");

    assert!(!encoded.contains("access_token"));
    assert!(!encoded.contains("raw_headers"));
    assert_eq!(
        serde_json::to_value(context).expect("context should serialize"),
        urgent_email_fixture()
    );
}

#[test]
fn morning_brief_context_matches_fixture() {
    let local_date = date("2026-02-15");
    let meetings = sample_meetings_unsorted();
    let candidates = sample_email_candidates_unsorted();

    let context = assemble_morning_brief_context(local_date, " 08:30 ", &meetings, &candidates);

    assert_eq!(
        serde_json::to_value(context).expect("context should serialize"),
        morning_brief_fixture()
    );
}

#[test]
fn assembly_handles_empty_and_noisy_inputs_gracefully() {
    let local_date = date("2026-02-15");
    let noisy_meetings = vec![GoogleCalendarMeetingSource {
        event_id: Some("".to_string()),
        title: Some("   ".to_string()),
        start_at: None,
        end_at: None,
        attendee_emails: vec![" ".to_string()],
    }];
    let noisy_candidates = vec![GoogleEmailCandidateSource {
        message_id: None,
        from: Some("   ".to_string()),
        subject: Some("   ".to_string()),
        snippet: Some("   ".to_string()),
        received_at: None,
        label_ids: vec![" ".to_string()],
        has_attachments: false,
    }];

    let context =
        assemble_morning_brief_context(local_date, "   ", &noisy_meetings, &noisy_candidates);
    let encoded = serde_json::to_string(&context).expect("context should encode");

    assert_eq!(context.morning_brief_local_time, "08:00");
    assert_eq!(context.meetings_today_count, 0);
    assert_eq!(context.urgent_email_candidate_count, 1);
    assert_eq!(context.urgent_email_candidates[0].message_ref, "email-001");
    assert_eq!(context.urgent_email_candidates[0].from, "unknown sender");
    assert_eq!(context.urgent_email_candidates[0].subject, "(no subject)");
    assert_eq!(context.urgent_email_candidates[0].snippet, "");
    assert!(context.urgent_email_candidates[0].received_at.is_none());
    assert!(!encoded.contains("access_token"));
    assert!(!encoded.contains("raw_headers"));
}

fn meetings_fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/meetings_today_context.json"))
        .expect("fixture must be valid JSON")
}

fn urgent_email_fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/urgent_email_context.json"))
        .expect("fixture must be valid JSON")
}

fn morning_brief_fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/morning_brief_context.json"))
        .expect("fixture must be valid JSON")
}

fn sample_meetings_unsorted() -> Vec<GoogleCalendarMeetingSource> {
    vec![
        GoogleCalendarMeetingSource {
            event_id: Some("evt-planning".to_string()),
            title: Some(" Platform planning review ".to_string()),
            start_at: Some(ts("2026-02-15T14:00:00Z")),
            end_at: Some(ts("2026-02-15T15:15:00Z")),
            attendee_emails: vec![
                "alice@example.com".to_string(),
                "bob@example.com".to_string(),
                "alice@example.com".to_string(),
                " ".to_string(),
            ],
        },
        GoogleCalendarMeetingSource {
            event_id: Some("evt-missing-start".to_string()),
            title: Some("No start time".to_string()),
            start_at: None,
            end_at: None,
            attendee_emails: vec![],
        },
        GoogleCalendarMeetingSource {
            event_id: None,
            title: Some(" Team sync ".to_string()),
            start_at: Some(ts("2026-02-15T09:30:00Z")),
            end_at: Some(ts("2026-02-15T10:00:00Z")),
            attendee_emails: vec![],
        },
        GoogleCalendarMeetingSource {
            event_id: None,
            title: Some("Wrong day".to_string()),
            start_at: Some(ts("2026-02-16T08:00:00Z")),
            end_at: Some(ts("2026-02-16T08:15:00Z")),
            attendee_emails: vec![],
        },
    ]
}

fn sample_email_candidates_unsorted() -> Vec<GoogleEmailCandidateSource> {
    vec![
        GoogleEmailCandidateSource {
            message_id: Some("msg-2".to_string()),
            from: Some(" CFO <cfo@example.com> ".to_string()),
            subject: Some(" Budget variance follow-up ".to_string()),
            snippet: Some(" Need approval today for vendor invoice. ".to_string()),
            received_at: Some(ts("2026-02-15T18:00:00Z")),
            label_ids: vec!["IMPORTANT".to_string(), "INBOX".to_string()],
            has_attachments: true,
        },
        GoogleEmailCandidateSource {
            message_id: None,
            from: None,
            subject: None,
            snippet: Some(" ".to_string()),
            received_at: None,
            label_ids: vec![" ".to_string()],
            has_attachments: false,
        },
        GoogleEmailCandidateSource {
            message_id: Some("msg-1".to_string()),
            from: Some("Ops".to_string()),
            subject: Some("Server alert".to_string()),
            snippet: Some("Latency high in us-east-1".to_string()),
            received_at: Some(ts("2026-02-15T19:00:00Z")),
            label_ids: vec!["INBOX".to_string(), "ALERT".to_string()],
            has_attachments: false,
        },
    ]
}

fn date(value: &str) -> NaiveDate {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").expect("date must parse")
}

fn ts(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .expect("timestamp must parse")
        .with_timezone(&Utc)
}
