use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Serialize;
use serde_json::{Value, json};
use shared::enclave_runtime::{AttestationChallengeRequest, AttestationChallengeResponse};

use crate::RuntimeState;

#[derive(Debug, Serialize)]
pub(crate) struct HealthResponse<'a> {
    status: &'a str,
    environment: &'a str,
    mode: &'a str,
}

pub(crate) async fn healthz(State(state): State<RuntimeState>) -> Json<HealthResponse<'static>> {
    Json(HealthResponse {
        status: "ok",
        environment: state.config.environment.as_str(),
        mode: state.config.mode.as_str(),
    })
}

pub(crate) async fn attestation_document(
    State(state): State<RuntimeState>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    state
        .config
        .attestation_document()
        .map(Json)
        .map_err(|err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "code": "attestation_document_unavailable",
                    "message": err,
                })),
            )
        })
}

pub(crate) async fn attestation_challenge(
    State(state): State<RuntimeState>,
    Json(challenge): Json<AttestationChallengeRequest>,
) -> Result<Json<AttestationChallengeResponse>, (StatusCode, Json<Value>)> {
    state
        .config
        .attestation_challenge_response(challenge)
        .map(Json)
        .map_err(|err| {
            let status = if err.starts_with("invalid challenge") {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            (
                status,
                Json(json!({
                    "code": "attestation_challenge_failed",
                    "message": err,
                })),
            )
        })
}
