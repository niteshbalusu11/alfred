use std::collections::VecDeque;
use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header::AUTHORIZATION};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{Value, json};
use shared::llm::{
    AssistantCapability, LlmGateway, LlmGatewayError, LlmGatewayRequest, OpenRouterGateway,
    OpenRouterGatewayConfig, OpenRouterModelRoute, template_for_capability,
};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, oneshot};

#[derive(Debug, Clone)]
struct MockReply {
    status: StatusCode,
    body: Value,
}

#[derive(Debug, Clone)]
struct TestServerState {
    replies: Arc<Mutex<VecDeque<MockReply>>>,
    seen_models: Arc<Mutex<Vec<String>>>,
    seen_auth_headers: Arc<Mutex<Vec<String>>>,
}

impl TestServerState {
    fn with_replies(replies: Vec<MockReply>) -> Self {
        Self {
            replies: Arc::new(Mutex::new(VecDeque::from(replies))),
            seen_models: Arc::new(Mutex::new(Vec::new())),
            seen_auth_headers: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[tokio::test]
async fn uses_primary_model_and_parses_response() {
    let state = TestServerState::with_replies(vec![MockReply {
        status: StatusCode::OK,
        body: success_response_body("provider-model", valid_output_json_string()),
    }]);
    let (url, shutdown_tx, server_task) = spawn_test_server(state.clone()).await;

    let gateway = OpenRouterGateway::new(config_for(url, 1, 0)).expect("gateway should build");
    let response = gateway
        .generate(meetings_summary_request())
        .await
        .expect("primary response should succeed");

    shutdown_tx.send(()).expect("shutdown signal should send");
    server_task.await.expect("server task should join");

    assert_eq!(response.model, "provider-model");
    assert_eq!(response.provider_request_id.as_deref(), Some("req-success"));
    assert_eq!(response.output["version"], "2026-02-15");

    let seen_models = state.seen_models.lock().await.clone();
    assert_eq!(seen_models, vec!["primary-model".to_string()]);

    let seen_auth_headers = state.seen_auth_headers.lock().await.clone();
    assert_eq!(
        seen_auth_headers,
        vec!["Bearer test-openrouter-key".to_string()]
    );
}

#[tokio::test]
async fn retries_transient_failures_before_succeeding() {
    let state = TestServerState::with_replies(vec![
        provider_error_reply(StatusCode::SERVICE_UNAVAILABLE, "overloaded"),
        provider_error_reply(StatusCode::BAD_GATEWAY, "upstream_gateway"),
        MockReply {
            status: StatusCode::OK,
            body: success_response_body("provider-model", valid_output_json_string()),
        },
    ]);
    let (url, shutdown_tx, server_task) = spawn_test_server(state.clone()).await;

    let gateway = OpenRouterGateway::new(config_for(url, 2, 0)).expect("gateway should build");
    let response = gateway
        .generate(meetings_summary_request())
        .await
        .expect("request should succeed after retries");

    shutdown_tx.send(()).expect("shutdown signal should send");
    server_task.await.expect("server task should join");

    assert_eq!(response.output["output"]["title"], "Daily meetings");
    let seen_models = state.seen_models.lock().await.clone();
    assert_eq!(
        seen_models,
        vec![
            "primary-model".to_string(),
            "primary-model".to_string(),
            "primary-model".to_string()
        ]
    );
}

#[tokio::test]
async fn falls_back_to_secondary_model_after_primary_retries_exhausted() {
    let state = TestServerState::with_replies(vec![
        provider_error_reply(StatusCode::SERVICE_UNAVAILABLE, "capacity"),
        provider_error_reply(StatusCode::SERVICE_UNAVAILABLE, "capacity"),
        MockReply {
            status: StatusCode::OK,
            body: success_response_body("fallback-provider-model", valid_output_json_string()),
        },
    ]);
    let (url, shutdown_tx, server_task) = spawn_test_server(state.clone()).await;

    let gateway = OpenRouterGateway::new(config_for(url, 1, 0)).expect("gateway should build");
    let response = gateway
        .generate(meetings_summary_request())
        .await
        .expect("fallback should recover request");

    shutdown_tx.send(()).expect("shutdown signal should send");
    server_task.await.expect("server task should join");

    assert_eq!(response.model, "fallback-provider-model");
    let seen_models = state.seen_models.lock().await.clone();
    assert_eq!(
        seen_models,
        vec![
            "primary-model".to_string(),
            "primary-model".to_string(),
            "fallback-model".to_string()
        ]
    );
}

#[tokio::test]
async fn does_not_fallback_on_unauthorized_provider_error() {
    let state = TestServerState::with_replies(vec![provider_error_reply(
        StatusCode::UNAUTHORIZED,
        "invalid_api_key",
    )]);
    let (url, shutdown_tx, server_task) = spawn_test_server(state.clone()).await;

    let gateway = OpenRouterGateway::new(config_for(url, 1, 0)).expect("gateway should build");
    let err = gateway
        .generate(meetings_summary_request())
        .await
        .expect_err("unauthorized errors should fail immediately");

    shutdown_tx.send(()).expect("shutdown signal should send");
    server_task.await.expect("server task should join");

    assert!(
        matches!(err, LlmGatewayError::ProviderFailure(ref message) if message.contains("status=401")),
        "expected structured unauthorized provider error, got {err:?}"
    );

    let seen_models = state.seen_models.lock().await.clone();
    assert_eq!(seen_models, vec!["primary-model".to_string()]);
}

#[tokio::test]
async fn falls_back_when_primary_returns_invalid_json_payload() {
    let state = TestServerState::with_replies(vec![
        MockReply {
            status: StatusCode::OK,
            body: success_response_body("primary-model", Value::String("not-json".to_string())),
        },
        MockReply {
            status: StatusCode::OK,
            body: success_response_body("fallback-model", valid_output_json_string()),
        },
    ]);
    let (url, shutdown_tx, server_task) = spawn_test_server(state.clone()).await;

    let gateway = OpenRouterGateway::new(config_for(url, 0, 0)).expect("gateway should build");
    let response = gateway
        .generate(meetings_summary_request())
        .await
        .expect("fallback should recover invalid primary payload");

    shutdown_tx.send(()).expect("shutdown signal should send");
    server_task.await.expect("server task should join");

    assert_eq!(response.model, "fallback-model");
    let seen_models = state.seen_models.lock().await.clone();
    assert_eq!(
        seen_models,
        vec!["primary-model".to_string(), "fallback-model".to_string()]
    );
}

fn meetings_summary_request() -> LlmGatewayRequest {
    LlmGatewayRequest::from_template(
        template_for_capability(AssistantCapability::MeetingsSummary),
        json!({
            "calendar_day": "2026-02-15",
            "meetings": [
                {
                    "title": "Team sync",
                    "start_at": "2026-02-15T09:00:00Z"
                }
            ]
        }),
    )
}

fn config_for(
    chat_completions_url: String,
    max_retries: u32,
    retry_base_backoff_ms: u64,
) -> OpenRouterGatewayConfig {
    OpenRouterGatewayConfig {
        chat_completions_url,
        api_key: "test-openrouter-key".to_string(),
        timeout_ms: 5_000,
        max_retries,
        retry_base_backoff_ms,
        model_route: OpenRouterModelRoute {
            primary_model: "primary-model".to_string(),
            fallback_model: Some("fallback-model".to_string()),
        },
    }
}

fn valid_output_json_string() -> Value {
    Value::String(
        json!({
            "version": "2026-02-15",
            "output": {
                "title": "Daily meetings",
                "summary": "You have one meeting this morning.",
                "key_points": ["Team sync at 9:00 AM"],
                "follow_ups": ["Share release blockers before noon"]
            }
        })
        .to_string(),
    )
}

fn success_response_body(model: &str, content: Value) -> Value {
    json!({
        "id": "req-success",
        "model": model,
        "choices": [
            {
                "message": {
                    "content": content
                }
            }
        ],
        "usage": {
            "prompt_tokens": 12,
            "completion_tokens": 8,
            "total_tokens": 20
        }
    })
}

fn provider_error_reply(status: StatusCode, code: &str) -> MockReply {
    MockReply {
        status,
        body: json!({
            "error": {
                "code": code
            }
        }),
    }
}

async fn spawn_test_server(
    state: TestServerState,
) -> (String, oneshot::Sender<()>, tokio::task::JoinHandle<()>) {
    let app = Router::new()
        .route("/chat/completions", post(test_chat_completions_handler))
        .with_state(state);

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let local_addr = listener
        .local_addr()
        .expect("listener address should resolve");
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    let server_task = tokio::spawn(async move {
        let server = axum::serve(listener, app).with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        });

        server.await.expect("test server should run");
    });

    (
        format!("http://{local_addr}/chat/completions"),
        shutdown_tx,
        server_task,
    )
}

async fn test_chat_completions_handler(
    State(state): State<TestServerState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> (StatusCode, Json<Value>) {
    if let Some(model) = payload.get("model").and_then(Value::as_str) {
        state.seen_models.lock().await.push(model.to_string());
    }

    if let Some(value) = headers
        .get(AUTHORIZATION)
        .and_then(|header| header.to_str().ok())
    {
        state.seen_auth_headers.lock().await.push(value.to_string());
    }

    let reply = state.replies.lock().await.pop_front().unwrap_or(MockReply {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        body: json!({
            "error": {
                "code": "exhausted_test_replies"
            }
        }),
    });

    (reply.status, Json(reply.body))
}
