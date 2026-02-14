use axum::extract::{Request, State};
use axum::http::header;
use axum::middleware::Next;
use axum::response::Response;
use tracing::warn;
use uuid::Uuid;

use super::clerk_identity::{ClerkIdentityError, verify_identity_token};
use super::errors::{bad_gateway_response, store_error_response, unauthorized_response};
use super::{AppState, AuthUser};

const CLERK_SUBJECT_NAMESPACE: Uuid = Uuid::from_u128(0x10850be7d81f4f4ea2dc0bb96943a09e);

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

    let identity = match verify_identity_token(
        &state.http_client,
        &state.clerk_jwks_url,
        &state.clerk_secret_key,
        &state.clerk_issuer,
        &state.clerk_audience,
        token,
    )
    .await
    {
        Ok(identity) => identity,
        Err(ClerkIdentityError::InvalidToken { code, message }) => {
            warn!("clerk auth rejected: code={code}, message={message}");
            return unauthorized_response();
        }
        Err(ClerkIdentityError::UpstreamUnavailable { code, message }) => {
            warn!("clerk auth upstream unavailable: code={code}, message={message}");
            return bad_gateway_response(code, message);
        }
    };

    let user_id = user_id_for_clerk_subject(&state.clerk_issuer, &identity.subject);
    match state.store.ensure_user(user_id).await {
        Ok(()) => {}
        Err(err) => return store_error_response(err),
    }

    req.extensions_mut().insert(AuthUser { user_id });
    next.run(req).await
}

fn user_id_for_clerk_subject(issuer: &str, subject: &str) -> Uuid {
    let stable_subject = format!("{}:{subject}", issuer.trim_end_matches('/'));
    Uuid::new_v5(&CLERK_SUBJECT_NAMESPACE, stable_subject.as_bytes())
}
