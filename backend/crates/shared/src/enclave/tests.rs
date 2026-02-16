use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use axum::extract::Json;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Router, response::IntoResponse};
use tokio::time::sleep;
use uuid::Uuid;

use super::{
    AttestedIdentityPayload, ConnectorSecretRequest, ENCLAVE_RPC_CONTRACT_VERSION,
    ENCLAVE_RPC_PATH_EXCHANGE_GOOGLE_TOKEN, EnclaveRpcAuthConfig, EnclaveRpcClient,
    EnclaveRpcError, EnclaveRpcErrorEnvelope, EnclaveRpcExchangeGoogleTokenRequest,
    EnclaveRpcExchangeGoogleTokenResponse, EnclaveRpcRevokeGoogleTokenRequest,
    EnclaveRpcRevokeGoogleTokenResponse,
};

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

#[tokio::test]
async fn rpc_client_maps_timeout_to_transport_unavailable() {
    let app = Router::new().route(
        ENCLAVE_RPC_PATH_EXCHANGE_GOOGLE_TOKEN,
        post(
            |Json(req): Json<EnclaveRpcExchangeGoogleTokenRequest>| async move {
                sleep(Duration::from_millis(200)).await;
                Json(EnclaveRpcExchangeGoogleTokenResponse {
                    contract_version: ENCLAVE_RPC_CONTRACT_VERSION.to_string(),
                    request_id: req.request_id,
                    access_token: "access-token".to_string(),
                    attested_identity: AttestedIdentityPayload {
                        runtime: "nitro".to_string(),
                        measurement: "mr_enclave_1".to_string(),
                    },
                })
            },
        ),
    );
    let (base_url, _server) = start_test_server(app).await;

    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_millis(50))
        .build()
        .expect("timeout client should build");
    let client = EnclaveRpcClient::new(
        base_url,
        EnclaveRpcAuthConfig {
            shared_secret: "local-secret".to_string(),
            max_clock_skew_seconds: 30,
        },
        http_client,
    );

    let err = client
        .exchange_google_access_token(ConnectorSecretRequest {
            user_id: Uuid::new_v4(),
            connector_id: Uuid::new_v4(),
        })
        .await
        .expect_err("timeout should map to transport unavailable");

    assert!(matches!(
        err,
        EnclaveRpcError::RpcTransportUnavailable { .. }
    ));
}

#[tokio::test]
async fn rpc_client_maps_provider_failure_error_contract() {
    let app = Router::new().route(
        ENCLAVE_RPC_PATH_EXCHANGE_GOOGLE_TOKEN,
        post(
            |Json(req): Json<EnclaveRpcExchangeGoogleTokenRequest>| async move {
                (
                    StatusCode::BAD_GATEWAY,
                    Json(EnclaveRpcErrorEnvelope::with_provider_failure(
                        Some(req.request_id),
                        401,
                        Some("invalid_grant".to_string()),
                    )),
                )
                    .into_response()
            },
        ),
    );
    let (base_url, _server) = start_test_server(app).await;

    let client = EnclaveRpcClient::new(
        base_url,
        EnclaveRpcAuthConfig {
            shared_secret: "local-secret".to_string(),
            max_clock_skew_seconds: 30,
        },
        reqwest::Client::new(),
    );

    let err = client
        .exchange_google_access_token(ConnectorSecretRequest {
            user_id: Uuid::new_v4(),
            connector_id: Uuid::new_v4(),
        })
        .await
        .expect_err("provider failure should map to provider error variant");

    assert!(matches!(
        err,
        EnclaveRpcError::ProviderRequestFailed {
            status: 401,
            oauth_error: Some(_),
            ..
        }
    ));
}

#[tokio::test]
async fn rpc_client_maps_transport_auth_rejection() {
    let app = Router::new().route(
        ENCLAVE_RPC_PATH_EXCHANGE_GOOGLE_TOKEN,
        post(
            |Json(req): Json<EnclaveRpcExchangeGoogleTokenRequest>| async move {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(EnclaveRpcErrorEnvelope::new(
                        Some(req.request_id),
                        "invalid_request_signature",
                        "request signature is invalid",
                        false,
                    )),
                )
                    .into_response()
            },
        ),
    );
    let (base_url, _server) = start_test_server(app).await;

    let client = EnclaveRpcClient::new(
        base_url,
        EnclaveRpcAuthConfig {
            shared_secret: "local-secret".to_string(),
            max_clock_skew_seconds: 30,
        },
        reqwest::Client::new(),
    );

    let err = client
        .exchange_google_access_token(ConnectorSecretRequest {
            user_id: Uuid::new_v4(),
            connector_id: Uuid::new_v4(),
        })
        .await
        .expect_err("auth rejection should map to unauthorized variant");

    assert!(matches!(err, EnclaveRpcError::RpcUnauthorized { .. }));
}

#[tokio::test]
async fn rpc_client_rejects_contract_version_drift() {
    let app = Router::new().route(
        ENCLAVE_RPC_PATH_EXCHANGE_GOOGLE_TOKEN,
        post(
            |Json(req): Json<EnclaveRpcExchangeGoogleTokenRequest>| async move {
                Json(EnclaveRpcExchangeGoogleTokenResponse {
                    contract_version: "v2".to_string(),
                    request_id: req.request_id,
                    access_token: "access-token".to_string(),
                    attested_identity: AttestedIdentityPayload {
                        runtime: "nitro".to_string(),
                        measurement: "mr_enclave_1".to_string(),
                    },
                })
            },
        ),
    );
    let (base_url, _server) = start_test_server(app).await;

    let client = EnclaveRpcClient::new(
        base_url,
        EnclaveRpcAuthConfig {
            shared_secret: "local-secret".to_string(),
            max_clock_skew_seconds: 30,
        },
        reqwest::Client::new(),
    );

    let err = client
        .exchange_google_access_token(ConnectorSecretRequest {
            user_id: Uuid::new_v4(),
            connector_id: Uuid::new_v4(),
        })
        .await
        .expect_err("contract drift must fail closed");

    assert!(matches!(err, EnclaveRpcError::RpcResponseInvalid { .. }));
}

#[tokio::test]
async fn rpc_client_rejects_request_id_mismatch_in_exchange_response() {
    let app = Router::new().route(
        ENCLAVE_RPC_PATH_EXCHANGE_GOOGLE_TOKEN,
        post(
            |_req: Json<EnclaveRpcExchangeGoogleTokenRequest>| async move {
                Json(EnclaveRpcExchangeGoogleTokenResponse {
                    contract_version: ENCLAVE_RPC_CONTRACT_VERSION.to_string(),
                    request_id: "mismatched-request-id".to_string(),
                    access_token: "access-token".to_string(),
                    attested_identity: AttestedIdentityPayload {
                        runtime: "nitro".to_string(),
                        measurement: "mr_enclave_1".to_string(),
                    },
                })
            },
        ),
    );
    let (base_url, _server) = start_test_server(app).await;

    let client = EnclaveRpcClient::new(
        base_url,
        EnclaveRpcAuthConfig {
            shared_secret: "local-secret".to_string(),
            max_clock_skew_seconds: 30,
        },
        reqwest::Client::new(),
    );

    let err = client
        .exchange_google_access_token(ConnectorSecretRequest {
            user_id: Uuid::new_v4(),
            connector_id: Uuid::new_v4(),
        })
        .await
        .expect_err("request_id mismatch must fail closed");

    assert!(matches!(err, EnclaveRpcError::RpcResponseInvalid { .. }));
}

#[tokio::test]
async fn rpc_client_revoke_maps_transport_auth_rejection() {
    let app = Router::new().route(
        super::ENCLAVE_RPC_PATH_REVOKE_GOOGLE_TOKEN,
        post(
            |Json(req): Json<EnclaveRpcRevokeGoogleTokenRequest>| async move {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(EnclaveRpcErrorEnvelope::new(
                        Some(req.request_id),
                        "missing_request_header",
                        "missing auth header",
                        false,
                    )),
                )
                    .into_response()
            },
        ),
    );
    let (base_url, _server) = start_test_server(app).await;

    let client = EnclaveRpcClient::new(
        base_url,
        EnclaveRpcAuthConfig {
            shared_secret: "local-secret".to_string(),
            max_clock_skew_seconds: 30,
        },
        reqwest::Client::new(),
    );

    let err = client
        .revoke_google_connector_token(ConnectorSecretRequest {
            user_id: Uuid::new_v4(),
            connector_id: Uuid::new_v4(),
        })
        .await
        .expect_err("auth rejection should map to unauthorized variant");

    assert!(matches!(err, EnclaveRpcError::RpcUnauthorized { .. }));
}

#[tokio::test]
async fn rpc_client_rejects_request_id_mismatch_in_revoke_response() {
    let app = Router::new().route(
        super::ENCLAVE_RPC_PATH_REVOKE_GOOGLE_TOKEN,
        post(
            |_req: Json<EnclaveRpcRevokeGoogleTokenRequest>| async move {
                Json(EnclaveRpcRevokeGoogleTokenResponse {
                    contract_version: ENCLAVE_RPC_CONTRACT_VERSION.to_string(),
                    request_id: "mismatched-request-id".to_string(),
                    attested_identity: AttestedIdentityPayload {
                        runtime: "nitro".to_string(),
                        measurement: "mr_enclave_1".to_string(),
                    },
                })
            },
        ),
    );
    let (base_url, _server) = start_test_server(app).await;

    let client = EnclaveRpcClient::new(
        base_url,
        EnclaveRpcAuthConfig {
            shared_secret: "local-secret".to_string(),
            max_clock_skew_seconds: 30,
        },
        reqwest::Client::new(),
    );

    let err = client
        .revoke_google_connector_token(ConnectorSecretRequest {
            user_id: Uuid::new_v4(),
            connector_id: Uuid::new_v4(),
        })
        .await
        .expect_err("request_id mismatch must fail closed");

    assert!(matches!(err, EnclaveRpcError::RpcResponseInvalid { .. }));
}

async fn start_test_server(app: Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("test listener should bind");
    let local_addr = listener
        .local_addr()
        .expect("listener should expose local address");

    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("test server should run");
    });

    (format!("http://{}", local_addr), server)
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
