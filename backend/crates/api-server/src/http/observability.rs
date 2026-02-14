use axum::extract::{MatchedPath, Request};
use axum::http::{HeaderValue, header};
use axum::middleware::Next;
use axum::response::Response;
use serde_json::{Map, Value, json};
use std::time::Instant;
use tracing::{info, warn};
use uuid::Uuid;

const REQUEST_ID_HEADER: &str = "x-request-id";
const MAX_REQUEST_ID_LEN: usize = 128;

#[derive(Clone, Debug)]
pub(super) struct RequestContext {
    pub(super) request_id: String,
}

pub(super) async fn request_observability_middleware(mut req: Request, next: Next) -> Response {
    let request_id = resolve_request_id(&req);
    req.extensions_mut().insert(RequestContext {
        request_id: request_id.clone(),
    });

    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let route = req
        .extensions()
        .get::<MatchedPath>()
        .map(|matched| matched.as_str().to_string())
        .unwrap_or(path);
    let started_at = Instant::now();

    let mut response = next.run(req).await;
    if let Ok(header_value) = HeaderValue::from_str(&request_id) {
        response.headers_mut().insert(
            header::HeaderName::from_static(REQUEST_ID_HEADER),
            header_value,
        );
    }

    let status = response.status().as_u16();
    let latency_ms = started_at.elapsed().as_millis() as u64;
    if status >= 500 {
        warn!(
            request_id = %request_id,
            method = %method,
            route = %route,
            status,
            latency_ms,
            metric_name = "api_http_request",
            "api request completed with server error"
        );
    } else {
        info!(
            request_id = %request_id,
            method = %method,
            route = %route,
            status,
            latency_ms,
            metric_name = "api_http_request",
            "api request metrics"
        );
    }

    response
}

pub(super) fn request_trace_payload(request_id: &str) -> Vec<u8> {
    attach_request_trace(Value::Object(Map::new()), request_id)
}

pub(super) fn attach_request_trace(payload: Value, request_id: &str) -> Vec<u8> {
    let mut root = match payload {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    root.insert("trace".to_string(), json!({ "request_id": request_id }));
    Value::Object(root).to_string().into_bytes()
}

fn resolve_request_id(req: &Request) -> String {
    req.headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(normalize_request_id)
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

fn normalize_request_id(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_REQUEST_ID_LEN {
        return None;
    }

    let valid = trimmed
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
    valid.then(|| trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{attach_request_trace, normalize_request_id, request_trace_payload};

    #[test]
    fn normalizes_valid_request_ids() {
        assert_eq!(
            normalize_request_id(" req-123._abc "),
            Some("req-123._abc".to_string())
        );
    }

    #[test]
    fn rejects_invalid_request_ids() {
        assert!(normalize_request_id("").is_none());
        assert!(normalize_request_id("abc$123").is_none());
        assert!(normalize_request_id(&"x".repeat(129)).is_none());
    }

    #[test]
    fn trace_payload_includes_request_id() {
        let raw = request_trace_payload("req-123");
        let value: Value = serde_json::from_slice(&raw).expect("valid payload");
        assert_eq!(value["trace"]["request_id"], "req-123");
    }

    #[test]
    fn attaches_request_trace_to_existing_payload() {
        let raw =
            attach_request_trace(serde_json::json!({"notification": {"title": "t"}}), "req-1");
        let value: Value = serde_json::from_slice(&raw).expect("valid payload");
        assert_eq!(value["trace"]["request_id"], "req-1");
        assert_eq!(value["notification"]["title"], "t");
    }
}
