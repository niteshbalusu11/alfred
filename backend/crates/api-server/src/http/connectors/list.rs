use axum::Json;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use shared::models::{ConnectorStatus, ConnectorSummary, ListConnectorsResponse};
use shared::repos::StoreError;

use super::super::errors::store_error_response;
use super::super::{AppState, AuthUser};

pub(crate) async fn list_connectors(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> Response {
    let connectors = match state.store.list_connector_states(user.user_id).await {
        Ok(connectors) => connectors,
        Err(err) => return store_error_response(err),
    };

    let mut items = Vec::with_capacity(connectors.len());
    for connector in connectors {
        let status = match connector.status.as_str() {
            "ACTIVE" => ConnectorStatus::Active,
            "REVOKED" => ConnectorStatus::Revoked,
            value => {
                return store_error_response(StoreError::InvalidData(format!(
                    "unknown connector status persisted: {value}"
                )));
            }
        };

        items.push(ConnectorSummary {
            connector_id: connector.connector_id.to_string(),
            provider: connector.provider,
            status,
        });
    }

    (StatusCode::OK, Json(ListConnectorsResponse { items })).into_response()
}
