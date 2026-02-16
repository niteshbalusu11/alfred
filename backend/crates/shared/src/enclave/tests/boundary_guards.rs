use std::fs;
use std::path::PathBuf;

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
fn sensitive_error_mapping_does_not_embed_upstream_messages() {
    const FORBIDDEN_PATTERNS: [&str; 3] = ["{message})", "{err})", "{error})"];

    for file in sensitive_error_message_guard_files() {
        let content = fs::read_to_string(&file)
            .expect("failed to read source file for error message guard test");
        for pattern in FORBIDDEN_PATTERNS {
            assert!(
                !content.contains(pattern),
                "{} contains upstream error interpolation pattern `{pattern}` in sensitive error mapping",
                file.display()
            );
        }
    }
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

fn guard_paths(files: &[&str]) -> Vec<PathBuf> {
    let shared_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    files.iter().map(|path| shared_root.join(path)).collect()
}

fn sensitive_tracing_guard_files() -> Vec<PathBuf> {
    guard_paths(&[
        "../api-server/src/http/authn.rs",
        "../api-server/src/http/assistant/session.rs",
        "../api-server/src/http/assistant/query.rs",
        "../api-server/src/http/connectors/revoke.rs",
        "../api-server/src/http/connectors/helpers.rs",
        "../worker/src/job_actions/google/session.rs",
        "../worker/src/job_actions/google/fetch.rs",
        "../worker/src/job_actions/google/morning_brief.rs",
        "../worker/src/job_actions/google/urgent_email.rs",
        "../worker/src/privacy_delete_revoke.rs",
        "../worker/src/privacy_delete.rs",
    ])
}

fn decrypt_boundary_guard_files() -> Vec<PathBuf> {
    guard_paths(&[
        "../api-server/src/http/assistant/session.rs",
        "../api-server/src/http/connectors/revoke.rs",
        "../worker/src/job_actions/google/session.rs",
        "../worker/src/privacy_delete_revoke.rs",
    ])
}

fn bearer_fetch_guard_files() -> Vec<PathBuf> {
    guard_paths(&[
        "../api-server/src/http/assistant/query.rs",
        "../api-server/src/http/assistant/fetch.rs",
        "../worker/src/job_actions/google/mod.rs",
        "../worker/src/job_actions/google/morning_brief.rs",
        "../worker/src/job_actions/google/urgent_email.rs",
        "../worker/src/job_actions/google/fetch.rs",
    ])
}

fn sensitive_error_message_guard_files() -> Vec<PathBuf> {
    guard_paths(&[
        "../worker/src/job_actions/google/fetch.rs",
        "../worker/src/privacy_delete_revoke.rs",
    ])
}
