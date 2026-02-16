use std::collections::HashMap;

use shared::repos::{AuditResult, Store};
use tracing::warn;

pub(super) async fn record_ai_audit_event(
    store: &Store,
    user_id: uuid::Uuid,
    event_type: &str,
    result: AuditResult,
    metadata: &HashMap<String, String>,
) {
    if let Err(err) = store
        .add_audit_event(user_id, event_type, None, result, metadata)
        .await
    {
        warn!(
            user_id = %user_id,
            event_type = event_type,
            "failed to persist AI worker audit event: {err}"
        );
    }
}
