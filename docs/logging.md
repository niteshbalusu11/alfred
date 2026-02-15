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
