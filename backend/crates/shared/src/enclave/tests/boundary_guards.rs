use std::fs;
use std::path::PathBuf;

#[test]
fn sensitive_worker_api_paths_do_not_log_secret_token_fields() {
    let shared_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let files = [
        shared_root.join("../api-server/src/http/assistant/session.rs"),
        shared_root.join("../api-server/src/http/connectors.rs"),
        shared_root.join("../api-server/src/http/connectors/revoke.rs"),
        shared_root.join("../worker/src/job_actions/google/session.rs"),
        shared_root.join("../worker/src/privacy_delete_revoke.rs"),
    ];

    for file in files {
        let content = fs::read_to_string(&file)
            .expect("failed to read source file for secret logging guard test");
        assert_no_sensitive_tracing_args(file.display().to_string().as_str(), &content);
    }
}

#[test]
fn host_paths_do_not_call_store_decrypt_directly() {
    let shared_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let files = [
        shared_root.join("../api-server/src/http/assistant/session.rs"),
        shared_root.join("../api-server/src/http/connectors/revoke.rs"),
        shared_root.join("../worker/src/job_actions/google/session.rs"),
        shared_root.join("../worker/src/privacy_delete_revoke.rs"),
    ];

    for file in files {
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
    let shared_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let files = [
        shared_root.join("../api-server/src/http/assistant/query.rs"),
        shared_root.join("../api-server/src/http/assistant/fetch.rs"),
        shared_root.join("../worker/src/job_actions/google/mod.rs"),
        shared_root.join("../worker/src/job_actions/google/morning_brief.rs"),
        shared_root.join("../worker/src/job_actions/google/urgent_email.rs"),
        shared_root.join("../worker/src/job_actions/google/fetch.rs"),
    ];

    for file in files {
        let content = fs::read_to_string(&file)
            .expect("failed to read source file for enclave-only google fetch guard test");
        assert!(
            !content.contains(".bearer_auth("),
            "{} must not perform direct bearer-auth Google fetches in host runtime",
            file.display()
        );
    }
}

fn assert_no_sensitive_tracing_args(path: &str, content: &str) {
    const TRACING_MACROS: [&str; 5] = ["trace!(", "debug!(", "info!(", "warn!(", "error!("];
    const SENSITIVE_TERMS: [&str; 4] = [
        "refresh_token",
        "access_token",
        "client_secret",
        "apns_token",
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
