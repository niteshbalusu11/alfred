use axum::Json;
use axum::extract::{Extension, State};
use axum::response::{IntoResponse, Response};
use base64::Engine as _;
use chrono::Utc;
use shared::assistant_crypto::{
    ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305, ASSISTANT_ENVELOPE_VERSION_V1,
};
use shared::enclave::EnclaveRpcError;
use shared::models::{AssistantQueryRequest, AssistantQueryResponse};
use tracing::warn;
use uuid::Uuid;

use super::super::errors::{bad_gateway_response, bad_request_response, store_error_response};
use super::super::{AppState, AuthUser};

pub(crate) async fn query_assistant(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Json(request): Json<AssistantQueryRequest>,
) -> Response {
    if let Some(response) = validate_envelope_shape(&request) {
        return response;
    }

    let now = Utc::now();
    let prior_session_state = match request.session_id {
        Some(session_id) => {
            match state
                .store
                .load_assistant_encrypted_session(user.user_id, session_id, now)
                .await
            {
                Ok(record) => record.map(|record| record.state),
                Err(err) => return store_error_response(err),
            }
        }
        None => None,
    };

    let enclave_client = shared::enclave::EnclaveRpcClient::new(
        state.enclave_rpc.base_url.clone(),
        state.enclave_rpc.auth.clone(),
        state.http_client.clone(),
    );
    let assistant_request_id = request.envelope.request_id.clone();
    let response = match enclave_client
        .process_assistant_query(user.user_id, request, prior_session_state)
        .await
    {
        Ok(response) => response,
        Err(err) => return map_assistant_enclave_error(err, user.user_id, &assistant_request_id),
    };

    if let Some(session_state) = &response.session_state {
        let ttl_seconds = (session_state.expires_at - now).num_seconds();
        if ttl_seconds <= 0 {
            return bad_gateway_response(
                "invalid_enclave_session_state",
                "Secure enclave session state has expired",
            );
        }

        if let Err(err) = state
            .store
            .upsert_assistant_encrypted_session(
                user.user_id,
                response.session_id,
                session_state,
                now,
                ttl_seconds,
            )
            .await
        {
            return store_error_response(err);
        }
    }

    (
        axum::http::StatusCode::OK,
        Json(AssistantQueryResponse {
            session_id: response.session_id,
            envelope: response.envelope,
        }),
    )
        .into_response()
}

fn validate_envelope_shape(request: &AssistantQueryRequest) -> Option<Response> {
    let envelope = &request.envelope;
    if envelope.version != ASSISTANT_ENVELOPE_VERSION_V1 {
        return Some(bad_request_response(
            "invalid_envelope_version",
            "assistant envelope version is not supported",
        ));
    }

    if envelope.algorithm != ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305 {
        return Some(bad_request_response(
            "invalid_envelope_algorithm",
            "assistant envelope algorithm is not supported",
        ));
    }

    if envelope.key_id.trim().is_empty() {
        return Some(bad_request_response("invalid_key_id", "key_id is required"));
    }

    if envelope.request_id.trim().is_empty() {
        return Some(bad_request_response(
            "invalid_request_id",
            "request_id is required",
        ));
    }

    let client_public_key = match base64::engine::general_purpose::STANDARD
        .decode(envelope.client_ephemeral_public_key.as_bytes())
    {
        Ok(bytes) => bytes,
        Err(_) => {
            return Some(bad_request_response(
                "invalid_client_public_key",
                "client_ephemeral_public_key must be valid base64",
            ));
        }
    };
    if client_public_key.len() != 32 {
        return Some(bad_request_response(
            "invalid_client_public_key",
            "client_ephemeral_public_key must decode to 32 bytes",
        ));
    }

    let nonce = match base64::engine::general_purpose::STANDARD.decode(envelope.nonce.as_bytes()) {
        Ok(bytes) => bytes,
        Err(_) => {
            return Some(bad_request_response(
                "invalid_nonce",
                "nonce must be valid base64",
            ));
        }
    };
    if nonce.len() != 12 {
        return Some(bad_request_response(
            "invalid_nonce",
            "nonce must decode to 12 bytes",
        ));
    }

    if base64::engine::general_purpose::STANDARD
        .decode(envelope.ciphertext.as_bytes())
        .is_err()
    {
        return Some(bad_request_response(
            "invalid_ciphertext",
            "ciphertext must be valid base64",
        ));
    }

    None
}

fn map_assistant_enclave_error(
    err: EnclaveRpcError,
    user_id: Uuid,
    assistant_request_id: &str,
) -> Response {
    match err {
        EnclaveRpcError::RpcContractRejected { code } => {
            warn!(
                %user_id,
                assistant_request_id,
                code = %code,
                "assistant query rejected by enclave contract"
            );
            bad_request_response(
                "invalid_enclave_request",
                "Encrypted assistant request rejected",
            )
        }
        EnclaveRpcError::RpcUnauthorized { code } => {
            warn!(
                %user_id,
                assistant_request_id,
                code = %code,
                "assistant query unauthorized by enclave RPC"
            );
            bad_gateway_response("enclave_rpc_failed", "Secure enclave RPC request failed")
        }
        EnclaveRpcError::RpcTransportUnavailable { message } => {
            warn!(
                %user_id,
                assistant_request_id,
                message = %message,
                "assistant query enclave RPC transport unavailable"
            );
            bad_gateway_response("enclave_rpc_failed", "Secure enclave RPC request failed")
        }
        EnclaveRpcError::RpcResponseInvalid { message } => {
            warn!(
                %user_id,
                assistant_request_id,
                message = %message,
                "assistant query enclave RPC response invalid"
            );
            bad_gateway_response("enclave_rpc_failed", "Secure enclave RPC request failed")
        }
        EnclaveRpcError::DecryptNotAuthorized { message } => {
            warn!(
                %user_id,
                assistant_request_id,
                message = %message,
                "assistant query token decrypt not authorized"
            );
            bad_gateway_response("enclave_rpc_failed", "Secure enclave RPC request failed")
        }
        EnclaveRpcError::ConnectorTokenDecryptFailed { message } => {
            warn!(
                %user_id,
                assistant_request_id,
                message = %message,
                "assistant query connector token decrypt failed"
            );
            bad_gateway_response("enclave_rpc_failed", "Secure enclave RPC request failed")
        }
        EnclaveRpcError::ConnectorTokenUnavailable => {
            warn!(
                %user_id,
                assistant_request_id,
                "assistant query connector token unavailable"
            );
            bad_request_response(
                "connector_token_unavailable",
                "Google connector is not active for this account; reconnect Google and retry",
            )
        }
        EnclaveRpcError::ProviderRequestUnavailable { operation, message } => {
            warn!(
                %user_id,
                assistant_request_id,
                operation = %operation,
                message = %message,
                "assistant query provider request unavailable"
            );
            bad_gateway_response("enclave_rpc_failed", "Secure enclave RPC request failed")
        }
        EnclaveRpcError::ProviderRequestFailed {
            operation,
            status,
            oauth_error,
        } => {
            warn!(
                %user_id,
                assistant_request_id,
                operation = %operation,
                status,
                oauth_error = ?oauth_error,
                "assistant query provider request failed"
            );
            bad_gateway_response("enclave_rpc_failed", "Secure enclave RPC request failed")
        }
        EnclaveRpcError::ProviderResponseInvalid { operation, message } => {
            warn!(
                %user_id,
                assistant_request_id,
                operation = %operation,
                message = %message,
                "assistant query provider response invalid"
            );
            bad_gateway_response("enclave_rpc_failed", "Secure enclave RPC request failed")
        }
    }
}
