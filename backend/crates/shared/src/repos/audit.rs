use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::Row;
use uuid::Uuid;

use crate::models::AuditEvent;

use super::{AuditResult, Store, StoreError};

impl Store {
    pub async fn add_audit_event(
        &self,
        user_id: Uuid,
        event_type: &str,
        connector: Option<&str>,
        result: AuditResult,
        metadata: &HashMap<String, String>,
    ) -> Result<(), StoreError> {
        self.ensure_user(user_id).await?;

        let redacted_metadata = redact_sensitive_metadata(metadata);

        sqlx::query(
            "INSERT INTO audit_events (user_id, event_type, connector, result, redacted_metadata)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(user_id)
        .bind(event_type)
        .bind(connector)
        .bind(result.as_str())
        .bind(redacted_metadata)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn list_audit_events(
        &self,
        user_id: Uuid,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<(Vec<AuditEvent>, Option<String>), StoreError> {
        let cursor = parse_cursor(cursor)?;

        let rows = sqlx::query(
            "SELECT id, created_at, event_type, connector, result, redacted_metadata
             FROM audit_events
             WHERE user_id = $1
               AND (
                 $2::timestamptz IS NULL
                 OR created_at < $2
                 OR (created_at = $2 AND id < $3)
               )
             ORDER BY created_at DESC, id DESC
             LIMIT $4",
        )
        .bind(user_id)
        .bind(cursor.as_ref().map(|(ts, _)| *ts))
        .bind(cursor.as_ref().map(|(_, id)| *id))
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        let mut items = Vec::with_capacity(rows.len());
        let mut last_key: Option<(DateTime<Utc>, Uuid)> = None;

        for row in rows {
            let id: Uuid = row.try_get("id")?;
            let created_at: DateTime<Utc> = row.try_get("created_at")?;
            let event_type: String = row.try_get("event_type")?;
            let connector: Option<String> = row.try_get("connector")?;
            let result: String = row.try_get("result")?;
            let metadata_value: Value = row.try_get("redacted_metadata")?;

            last_key = Some((created_at, id));

            items.push(AuditEvent {
                id: id.to_string(),
                timestamp: created_at,
                event_type,
                connector,
                result,
                metadata: json_value_to_string_map(metadata_value),
            });
        }

        let next_cursor = if items.len() == limit {
            last_key.map(|(ts, id)| encode_cursor(ts, id))
        } else {
            None
        };

        Ok((items, next_cursor))
    }
}

fn parse_cursor(cursor: Option<&str>) -> Result<Option<(DateTime<Utc>, Uuid)>, StoreError> {
    let Some(cursor) = cursor else {
        return Ok(None);
    };

    let (timestamp_micros, id) = cursor.split_once('|').ok_or(StoreError::InvalidCursor)?;
    let timestamp_micros = timestamp_micros
        .parse::<i64>()
        .map_err(|_| StoreError::InvalidCursor)?;
    let timestamp =
        DateTime::from_timestamp_micros(timestamp_micros).ok_or(StoreError::InvalidCursor)?;
    let id = Uuid::parse_str(id).map_err(|_| StoreError::InvalidCursor)?;

    Ok(Some((timestamp, id)))
}

fn encode_cursor(timestamp: DateTime<Utc>, id: Uuid) -> String {
    format!("{}|{}", timestamp.timestamp_micros(), id)
}

fn json_value_to_string_map(value: Value) -> HashMap<String, String> {
    match value {
        Value::Object(map) => map
            .into_iter()
            .map(|(key, value)| {
                let stringified = match value {
                    Value::String(string) => string,
                    other => other.to_string(),
                };
                (key, stringified)
            })
            .collect(),
        _ => HashMap::new(),
    }
}

fn is_sensitive_metadata_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.contains("token")
        || key.contains("secret")
        || key.contains("password")
        || key.contains("authorization")
        || key.contains("code")
}

fn redact_sensitive_metadata(metadata: &HashMap<String, String>) -> Value {
    Value::Object(
        metadata
            .iter()
            .map(|(key, value)| {
                if is_sensitive_metadata_key(key) {
                    (key.clone(), Value::String("[REDACTED]".to_string()))
                } else {
                    (key.clone(), Value::String(value.clone()))
                }
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::Value;

    use super::{is_sensitive_metadata_key, redact_sensitive_metadata};

    #[test]
    fn sensitive_metadata_keys_are_case_insensitive() {
        assert!(is_sensitive_metadata_key("refresh_token"));
        assert!(is_sensitive_metadata_key("Authorization"));
        assert!(is_sensitive_metadata_key("OAUTH_CODE"));
        assert!(is_sensitive_metadata_key("apiSecret"));
        assert!(!is_sensitive_metadata_key("request_id"));
    }

    #[test]
    fn redaction_masks_sensitive_fields_and_preserves_non_sensitive_fields() {
        let mut metadata = HashMap::new();
        metadata.insert("refresh_token".to_string(), "rt-123".to_string());
        metadata.insert("Authorization".to_string(), "Bearer abc".to_string());
        metadata.insert("request_id".to_string(), "req-1".to_string());
        metadata.insert("status".to_string(), "ok".to_string());

        let redacted = redact_sensitive_metadata(&metadata);
        assert!(redacted.is_object());
        let object = redacted
            .as_object()
            .expect("redacted metadata should always be a JSON object");

        assert_eq!(
            object.get("refresh_token"),
            Some(&Value::String("[REDACTED]".to_string()))
        );
        assert_eq!(
            object.get("Authorization"),
            Some(&Value::String("[REDACTED]".to_string()))
        );
        assert_eq!(
            object.get("request_id"),
            Some(&Value::String("req-1".to_string()))
        );
        assert_eq!(object.get("status"), Some(&Value::String("ok".to_string())));
    }
}
