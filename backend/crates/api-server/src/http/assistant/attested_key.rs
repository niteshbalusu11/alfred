use axum::Json;
use axum::extract::{Extension, State};
use axum::response::{IntoResponse, Response};
use shared::models::{
    AssistantAttestedKeyAttestation, AssistantAttestedKeyRequest, AssistantAttestedKeyResponse,
};

use super::super::errors::{bad_gateway_response, bad_request_response};
use super::super::{AppState, AuthUser};

pub(crate) async fn fetch_attested_key(
    State(state): State<AppState>,
    Extension(_user): Extension<AuthUser>,
    Json(request): Json<AssistantAttestedKeyRequest>,
) -> Response {
    if request.challenge_nonce.trim().is_empty() {
        return bad_request_response("invalid_challenge_nonce", "challenge_nonce is required");
    }
    if request.request_id.trim().is_empty() {
        return bad_request_response("invalid_request_id", "request_id is required");
    }
    if request.expires_at <= request.issued_at {
        return bad_request_response(
            "invalid_challenge_window",
            "expires_at must be greater than issued_at",
        );
    }

    let now = chrono::Utc::now().timestamp();
    if now > request.expires_at {
        return bad_request_response("challenge_expired", "challenge has expired");
    }

    let enclave_client = shared::enclave::EnclaveRpcClient::new(
        state.enclave_rpc.base_url.clone(),
        state.enclave_rpc.auth.clone(),
        state.http_client.clone(),
    );
    let response = match enclave_client
        .fetch_assistant_attested_key(
            request.challenge_nonce.clone(),
            request.issued_at,
            request.expires_at,
            request.request_id.clone(),
        )
        .await
    {
        Ok(response) => response,
        Err(_) => {
            return bad_gateway_response("enclave_rpc_failed", "Secure enclave RPC request failed");
        }
    };

    if response.challenge_nonce != request.challenge_nonce
        || response.request_id != request.request_id
    {
        return bad_gateway_response(
            "attestation_challenge_mismatch",
            "Attested key response did not match challenge",
        );
    }

    (
        axum::http::StatusCode::OK,
        Json(AssistantAttestedKeyResponse {
            key_id: response.key_id,
            algorithm: response.algorithm,
            public_key: response.public_key,
            key_expires_at: response.key_expires_at,
            attestation: AssistantAttestedKeyAttestation {
                runtime: response.runtime,
                measurement: response.measurement,
                challenge_nonce: response.challenge_nonce,
                issued_at: response.issued_at,
                expires_at: response.expires_at,
                request_id: response.request_id,
                evidence_issued_at: response.evidence_issued_at,
                signature: response.signature,
            },
        }),
    )
        .into_response()
}
