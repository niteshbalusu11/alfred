# Logging Conventions

This project uses structured logs on the backend and a shared logger facade on iOS.

## Backend (API server)

Location:
- `backend/crates/api-server/src/main.rs`
- `backend/crates/api-server/src/http/observability.rs`

Rules:
- Emit JSON logs through `tracing_subscriber`.
- Include request correlation via `request_id`.
- For every request completion, include:
  - `method`
  - `route`
  - `path`
  - `status`
  - `latency_ms`
  - `outcome`
- Never log secrets, OAuth tokens, or raw sensitive payloads.
- Secret logging guard tests (`cargo test -p shared boundary_guards`) automatically scan API/worker/enclave-sensitive Rust modules and fail CI on forbidden tracing patterns.

Sensitive field policy:
- Forbidden in tracing fields and messages: `refresh_token`, `access_token`, `client_secret`, `apns_token`, `authorization_header`, `oauth_code`, `id_token`, `identity_token`, `bearer_token`.
- When mapping provider/enclave/database errors in sensitive token paths, use deterministic sanitized messages and error codes.
- Do not append upstream error text (`{err}`, `{error}`, `{message}`) to user-visible API errors, persisted worker failure reasons, or sensitive audit metadata.
- Audit metadata persistence redacts both sensitive keys and leaked token/authorization markers in values.

Backend examples:
- Do: `warn!(user_id = %user_id, error_code = "GOOGLE_REVOKE_FAILED", "google revoke failed")`
- Don’t: `warn!(refresh_token = %token, "revoke failed")`
- Don’t: `JobExecutionError::transient("GOOGLE_UNAVAILABLE", format!("provider failed: {message}"))`
- Do: `JobExecutionError::transient("GOOGLE_UNAVAILABLE", "provider request failed")`

## iOS app

Location:
- `alfred/alfred/Core/AppLogger.swift`

Usage:
- `AppLogger.debug(...)` for debug-only logs (`#if DEBUG` gated).
- `AppLogger.info(...)`, `AppLogger.warning(...)`, `AppLogger.error(...)` for logs that should also appear in Release builds.
- Select a category (`.app`, `.auth`, `.network`, `.oauth`) to keep log streams easy to filter.

Guidelines:
- Prefer event-style messages over verbose prose.
- Avoid logging user secrets/tokens, callback codes, or other sensitive values.
