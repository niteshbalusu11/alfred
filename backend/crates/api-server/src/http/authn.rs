use axum::extract::{Request, State};
use axum::http::header;
use axum::middleware::Next;
use axum::response::Response;
use chrono::Utc;
use tracing::warn;

use super::errors::{store_error_response, unauthorized_response};
use super::tokens::hash_token;
use super::{AppState, AuthUser};

pub(super) async fn auth_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();

    let token = auth_header
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|token| !token.is_empty());

    let Some(token) = token else {
        warn!("missing or invalid authorization header");
        return unauthorized_response();
    };

    let token_hash = hash_token(token);

    let user_id = match state
        .store
        .resolve_session_user(&token_hash, Utc::now())
        .await
    {
        Ok(Some(user_id)) => user_id,
        Ok(None) => return unauthorized_response(),
        Err(err) => return store_error_response(err),
    };

    req.extensions_mut().insert(AuthUser { user_id });
    next.run(req).await
}
