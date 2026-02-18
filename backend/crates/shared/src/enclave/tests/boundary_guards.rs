use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn sensitive_host_paths_do_not_log_secret_token_fields() {
    for file in sensitive_tracing_guard_files() {
        let content = fs::read_to_string(&file)
            .expect("failed to read source file for secret logging guard test");
        assert_no_sensitive_tracing_args(file.display().to_string().as_str(), &content);
    }
}

#[test]
fn host_paths_do_not_call_store_decrypt_directly() {
    for file in decrypt_boundary_guard_files() {
        let content = fs::read_to_string(&file)
            .expect("failed to read source file for decrypt boundary guard test");
        assert!(
            !content.contains("decrypt_active_connector_refresh_token("),
            "{} must not call connector decrypt repository API directly",
            file.display()
        );
    }
}

#[test]
fn host_paths_do_not_perform_google_bearer_fetches_outside_enclave() {
    for file in bearer_fetch_guard_files() {
        let content = fs::read_to_string(&file)
            .expect("failed to read source file for enclave-only google fetch guard test");
        assert!(
            !content.contains(".bearer_auth("),
            "{} must not perform direct bearer-auth Google fetches in host runtime",
            file.display()
        );
    }
}

#[test]
fn host_paths_do_not_construct_plaintext_llm_context_for_migrated_flows() {
    for file in host_llm_orchestration_guard_files() {
        let content = fs::read_to_string(&file)
            .expect("failed to read source file for host llm orchestration guard test");
        assert!(
            !content.contains("LlmGatewayRequest::from_template("),
            "{} must not build LLM request templates in host runtime for migrated paths",
            file.display()
        );
        assert!(
            !content.contains("template_for_capability("),
            "{} must not select prompt templates in host runtime for migrated paths",
            file.display()
        );
        assert!(
            !content.contains("generate_with_telemetry("),
            "{} must not execute host-side LLM calls for migrated paths",
            file.display()
        );
        assert!(
            !content.contains("resolve_safe_output("),
            "{} must not resolve LLM safety output in host runtime for migrated paths",
            file.display()
        );
    }
}

#[test]
fn enclave_orchestrator_primary_route_does_not_use_keyword_router() {
    let shared_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = shared_root.join("../enclave-runtime/src/http/assistant/orchestrator/mod.rs");
    let content = fs::read_to_string(&path)
        .expect("failed to read enclave assistant orchestrator source for routing guard");

    assert!(
        !content.contains("detect_query_capability("),
        "{} must not use keyword detector in primary orchestrator route selection",
        path.display()
    );
    assert!(
        !content.contains("resolve_query_capability("),
        "{} must not use keyword resolver in primary orchestrator route selection",
        path.display()
    );
}

#[test]
fn sensitive_error_mapping_does_not_embed_upstream_messages() {
    for file in sensitive_error_message_guard_files() {
        let content = fs::read_to_string(&file)
            .expect("failed to read source file for error message guard test");
        assert_no_error_interpolation(file.display().to_string().as_str(), &content);
    }
}

#[test]
fn assistant_query_contracts_do_not_reintroduce_plaintext_query_fields() {
    let shared_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let openapi_path = shared_root.join("../../../api/openapi.yaml");
    let openapi = fs::read_to_string(&openapi_path)
        .expect("failed to read OpenAPI spec for assistant contract guard");
    let openapi_block = extract_named_yaml_block(
        &openapi,
        "AssistantQueryRequest:",
        "AssistantEncryptedRequestEnvelope:",
    )
    .expect("AssistantQueryRequest block must exist in OpenAPI");
    assert!(
        !openapi_block.contains("query:"),
        "OpenAPI AssistantQueryRequest must not include plaintext query field"
    );

    let rust_models_path = shared_root.join("src/models.rs");
    let rust_models = fs::read_to_string(&rust_models_path)
        .expect("failed to read shared models for plaintext guard");
    let rust_block = extract_rust_struct_block(&rust_models, "pub struct AssistantQueryRequest")
        .expect("AssistantQueryRequest struct must exist in shared models");
    assert!(
        !rust_block.contains("pub query:"),
        "shared AssistantQueryRequest must not include plaintext query field"
    );

    let swift_models_paths = [
        shared_root.join("../../../alfred/Packages/AlfredAPIClient/Sources/AssistantModels.swift"),
        shared_root.join("../../../alfred/Packages/AlfredAPIClient/Sources/Models.swift"),
    ];
    let swift_block = extract_swift_struct_block_from_files(
        &swift_models_paths,
        "public struct AssistantQueryRequest",
    )
    .expect("AssistantQueryRequest struct must exist in Swift models");
    assert!(
        !swift_block.contains("query: String"),
        "Swift AssistantQueryRequest must not include plaintext query field"
    );
}

#[test]
fn assistant_query_host_response_contracts_remain_envelope_only() {
    let shared_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let openapi_path = shared_root.join("../../../api/openapi.yaml");
    let openapi = fs::read_to_string(&openapi_path)
        .expect("failed to read OpenAPI spec for assistant response contract guard");
    let openapi_block = extract_named_yaml_block(
        &openapi,
        "AssistantQueryResponse:",
        "AssistantAttestedKeyRequest:",
    )
    .expect("AssistantQueryResponse block must exist in OpenAPI");
    assert!(
        !openapi_block.contains("display_text"),
        "OpenAPI AssistantQueryResponse must not expose plaintext display_text"
    );
    assert!(
        !openapi_block.contains("response_parts"),
        "OpenAPI AssistantQueryResponse must not expose plaintext response_parts"
    );
    assert!(
        !openapi_block.contains("payload"),
        "OpenAPI AssistantQueryResponse must not expose plaintext payload"
    );

    let rust_models_path = shared_root.join("src/models.rs");
    let rust_models = fs::read_to_string(&rust_models_path)
        .expect("failed to read shared models for assistant response guard");
    let rust_block = extract_rust_struct_block(&rust_models, "pub struct AssistantQueryResponse")
        .expect("AssistantQueryResponse struct must exist in shared models");
    assert!(
        !rust_block.contains("display_text"),
        "shared AssistantQueryResponse must not expose plaintext display_text"
    );
    assert!(
        !rust_block.contains("response_parts"),
        "shared AssistantQueryResponse must not expose plaintext response_parts"
    );
    assert!(
        !rust_block.contains("payload"),
        "shared AssistantQueryResponse must not expose plaintext payload"
    );

    let swift_models_path =
        shared_root.join("../../../alfred/Packages/AlfredAPIClient/Sources/AssistantModels.swift");
    let swift_models = fs::read_to_string(&swift_models_path)
        .expect("failed to read Swift assistant models for response guard");
    let swift_block =
        extract_swift_struct_block(&swift_models, "public struct AssistantQueryResponse")
            .expect("AssistantQueryResponse struct must exist in Swift models");
    assert!(
        !swift_block.contains("displayText"),
        "Swift AssistantQueryResponse must not expose plaintext displayText"
    );
    assert!(
        !swift_block.contains("responseParts"),
        "Swift AssistantQueryResponse must not expose plaintext responseParts"
    );
    assert!(
        !swift_block.contains("payload"),
        "Swift AssistantQueryResponse must not expose plaintext payload"
    );
}

#[test]
fn host_assistant_error_logs_do_not_record_upstream_message_bodies() {
    let shared_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = shared_root.join("../api-server/src/http/assistant/query.rs");
    let content =
        fs::read_to_string(&path).expect("failed to read assistant query host mapping source");

    assert!(
        !content.contains("message = %message"),
        "{} must not log upstream enclave message content in host assistant error mapping",
        path.display()
    );
    assert!(
        !content.contains("oauth_error = ?oauth_error"),
        "{} must not log upstream oauth_error details in host assistant error mapping",
        path.display()
    );
}

#[test]
fn host_connector_paths_do_not_exchange_oauth_codes_directly() {
    for file in oauth_exchange_guard_files() {
        let content = fs::read_to_string(&file)
            .expect("failed to read source file for oauth exchange boundary guard");
        assert!(
            !content.contains("grant_type\", \"authorization_code\""),
            "{} must not perform direct OAuth code exchange in host runtime",
            file.display()
        );
        assert!(
            !content.contains(".post(&oauth.token_url)"),
            "{} must not call Google token endpoint directly from host runtime",
            file.display()
        );
    }
}

#[test]
fn redis_reliability_state_does_not_store_plaintext_llm_response_payloads() {
    let shared_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = shared_root.join("src/llm/reliability/redis_state.rs");
    let content = fs::read_to_string(&path).expect("failed to read redis reliability state source");

    assert!(
        !content.contains("serde_json::to_string(response)"),
        "{} must not serialize plaintext LLM responses for Redis cache persistence",
        path.display()
    );
    assert!(
        !content.contains("serde_json::from_str::<LlmGatewayResponse>"),
        "{} must not deserialize plaintext LLM responses from Redis cache persistence",
        path.display()
    );
}

#[test]
fn callback_job_enqueue_does_not_write_trace_payload() {
    let shared_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = shared_root.join("../api-server/src/http/connectors/callback.rs");
    let content = fs::read_to_string(&path)
        .expect("failed to read connector callback source for payload guard");

    assert!(
        !content.contains("request_trace_payload("),
        "{} must not enqueue callback trace payload bytes into jobs table",
        path.display()
    );
}

fn assert_no_sensitive_tracing_args(path: &str, content: &str) {
    const TRACING_MACROS: [&str; 5] = ["trace!(", "debug!(", "info!(", "warn!(", "error!("];
    const SENSITIVE_TERMS: [&str; 9] = [
        "refresh_token",
        "access_token",
        "client_secret",
        "apns_token",
        "bearer_token",
        "authorization_header",
        "oauth_code",
        "identity_token",
        "id_token",
    ];

    for macro_call in TRACING_MACROS {
        let mut from = 0;
        while let Some(start_offset) = content[from..].find(macro_call) {
            let start = from + start_offset;
            let Some(end_offset) = content[start..].find(");") else {
                break;
            };
            let end = start + end_offset + 2;
            let snippet = content[start..end].to_ascii_lowercase();

            for term in SENSITIVE_TERMS {
                assert!(
                    !snippet.contains(term),
                    "{path} contains sensitive term `{term}` in tracing macro: {snippet}"
                );
            }

            from = end;
        }
    }
}

fn assert_no_error_interpolation(path: &str, content: &str) {
    const FORBIDDEN_INTERPOLATIONS: [&str; 3] = ["{err}", "{error}", "{message}"];

    let lowered = content.to_ascii_lowercase();
    for token in FORBIDDEN_INTERPOLATIONS {
        let mut from = 0;
        while let Some(found) = lowered[from..].find(token) {
            let token_start = from + found;
            let window_start = token_start.saturating_sub(220);
            let context = &lowered[window_start..token_start];

            assert!(
                !context.contains("format!("),
                "{path} contains upstream error interpolation token `{token}` in format! macro"
            );

            from = token_start + token.len();
        }
    }
}

fn collect_rust_guard_files(paths: &[&str]) -> Vec<PathBuf> {
    let shared_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut files = BTreeSet::new();

    for relative in paths {
        let absolute = shared_root.join(relative);
        collect_rust_files_recursive(&absolute, &mut files);
    }

    files.into_iter().collect()
}

fn extract_named_yaml_block<'a>(
    content: &'a str,
    marker: &str,
    next_marker: &str,
) -> Option<&'a str> {
    let start = content.find(marker)?;
    let remaining = &content[start..];
    let end = remaining
        .find(&format!("\n    {next_marker}"))
        .unwrap_or(remaining.len());
    Some(&remaining[..end])
}

fn extract_rust_struct_block<'a>(content: &'a str, marker: &str) -> Option<&'a str> {
    let start = content.find(marker)?;
    let remaining = &content[start..];
    let end = remaining.find("\n}\n\n").unwrap_or(remaining.len());
    Some(&remaining[..end])
}

fn extract_swift_struct_block<'a>(content: &'a str, marker: &str) -> Option<&'a str> {
    let start = content.find(marker)?;
    let remaining = &content[start..];
    let end = remaining.find("\n}\n\n").unwrap_or(remaining.len());
    Some(&remaining[..end])
}

fn extract_swift_struct_block_from_files(paths: &[PathBuf], marker: &str) -> Option<String> {
    for path in paths {
        let Ok(content) = fs::read_to_string(path) else {
            continue;
        };
        if let Some(block) = extract_swift_struct_block(&content, marker) {
            return Some(block.to_string());
        }
    }
    None
}

fn collect_rust_files_recursive(path: &Path, files: &mut BTreeSet<PathBuf>) {
    if path.is_file() {
        if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            files.insert(path.to_path_buf());
        }
        return;
    }

    if !path.is_dir() {
        return;
    }

    for entry in fs::read_dir(path).expect("failed to read source directory for guard test") {
        let entry = entry.expect("failed to read source directory entry for guard test");
        collect_rust_files_recursive(&entry.path(), files);
    }
}

fn sensitive_tracing_guard_files() -> Vec<PathBuf> {
    collect_rust_guard_files(&[
        "../api-server/src/http",
        "../worker/src",
        "../enclave-runtime/src",
    ])
}

fn decrypt_boundary_guard_files() -> Vec<PathBuf> {
    collect_rust_guard_files(&["../api-server/src", "../worker/src"])
}

fn bearer_fetch_guard_files() -> Vec<PathBuf> {
    collect_rust_guard_files(&[
        "../api-server/src/http/assistant",
        "../worker/src/job_actions/google",
    ])
}

fn sensitive_error_message_guard_files() -> Vec<PathBuf> {
    collect_rust_guard_files(&[
        "../api-server/src/http/assistant",
        "../api-server/src/http/connectors",
        "../worker/src/job_actions/google",
        "../worker/src/privacy_delete.rs",
        "../worker/src/privacy_delete_revoke.rs",
    ])
}

fn host_llm_orchestration_guard_files() -> Vec<PathBuf> {
    collect_rust_guard_files(&[
        "../api-server/src/http/assistant",
        "../worker/src/job_actions/google/morning_brief.rs",
        "../worker/src/job_actions/google/urgent_email.rs",
    ])
}

fn oauth_exchange_guard_files() -> Vec<PathBuf> {
    collect_rust_guard_files(&["../api-server/src/http/connectors/callback.rs"])
}
