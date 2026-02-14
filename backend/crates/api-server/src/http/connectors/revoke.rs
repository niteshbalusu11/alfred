use std::collections::HashMap;

use axum::Json;
use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use shared::enclave::ConnectorSecretRequest;
use shared::models::{ConnectorStatus, ErrorBody, ErrorResponse, RevokeConnectorResponse};
use shared::repos::{AuditResult, LEGACY_CONNECTOR_TOKEN_KEY_ID};
use uuid::Uuid;

use super::super::errors::{bad_request_response, store_error_response};
use super::super::{AppState, AuthUser};
use super::helpers::{build_enclave_client, map_revoke_enclave_error};

pub(crate) async fn revoke_connector(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(connector_id): Path<String>,
) -> Response {
    let connector_id = match Uuid::parse_str(&connector_id) {
        Ok(connector_id) => connector_id,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: ErrorBody {
                        code: "not_found".to_string(),
                        message: "Connector not found".to_string(),
                    },
                }),
            )
                .into_response();
        }
    };

    let mut connector_metadata = match state
        .store
        .get_active_connector_key_metadata(user.user_id, connector_id)
        .await
    {
        Ok(Some(connector_metadata)) => connector_metadata,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: ErrorBody {
                        code: "not_found".to_string(),
                        message: "Connector not found".to_string(),
                    },
                }),
            )
                .into_response();
        }
        Err(err) => return store_error_response(err),
    };

    if connector_metadata.provider != "google" {
        return bad_request_response(
            "unsupported_provider",
            "Connector provider is not supported",
        );
    }

    if connector_metadata.token_key_id == LEGACY_CONNECTOR_TOKEN_KEY_ID {
        if let Err(err) = state
            .store
            .adopt_legacy_connector_token_key_id(
                user.user_id,
                connector_id,
                state.secret_runtime.kms_key_id(),
                state.secret_runtime.kms_key_version(),
            )
            .await
        {
            return store_error_response(err);
        }

        connector_metadata = match state
            .store
            .get_active_connector_key_metadata(user.user_id, connector_id)
            .await
        {
            Ok(Some(connector_metadata)) => connector_metadata,
            Ok(None) => {
                return bad_request_response(
                    "connector_token_unavailable",
                    "Connector token metadata changed; retry the request",
                );
            }
            Err(err) => return store_error_response(err),
        };
    }

    let enclave_client = build_enclave_client(&state);
    let enclave_response = match enclave_client
        .revoke_google_connector_token(ConnectorSecretRequest {
            user_id: user.user_id,
            connector_id,
            token_key_id: connector_metadata.token_key_id.clone(),
            token_version: connector_metadata.token_version,
        })
        .await
    {
        Ok(response) => response,
        Err(err) => return map_revoke_enclave_error(err),
    };

    match state
        .store
        .revoke_connector(user.user_id, connector_id)
        .await
    {
        Ok(true) => {
            let mut metadata = HashMap::new();
            metadata.insert("connector_id".to_string(), connector_id.to_string());
            metadata.insert(
                "attested_measurement".to_string(),
                enclave_response.attested_identity.measurement,
            );

            if let Err(err) = state
                .store
                .add_audit_event(
                    user.user_id,
                    "CONNECTOR_REVOKED",
                    Some("google"),
                    AuditResult::Success,
                    &metadata,
                )
                .await
            {
                return store_error_response(err);
            }

            (
                StatusCode::OK,
                Json(RevokeConnectorResponse {
                    status: ConnectorStatus::Revoked,
                }),
            )
                .into_response()
        }
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: ErrorBody {
                    code: "not_found".to_string(),
                    message: "Connector not found".to_string(),
                },
            }),
        )
            .into_response(),
        Err(err) => store_error_response(err),
    }
}
