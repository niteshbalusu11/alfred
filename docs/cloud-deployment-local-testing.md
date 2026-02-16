# Cloud Deployment and Local Manual Testing Guide

Last updated: 2026-02-14

This guide is for first-time manual testing when you have not used the app/backend before.

Auth note:
1. Alfred backend auth is Clerk-only (epic `#52`).
2. Protected endpoints require a valid Clerk JWT bearer token.

## 1) Current Reality (What Is Already Implemented)

Backend:
1. API routes are implemented for devices/APNs registration, Google connector start/callback/revoke, preferences, audit events, and privacy delete-all.
2. Worker job engine, retries/idempotency, notification dispatch path, and privacy delete processing are implemented.

iOS:
1. There is working SwiftUI app code for sign-in, Google connect, preferences, privacy actions, and activity log.
2. There is an API client package and session/token storage code.
3. The app currently behaves more like an integration harness than final polished UX.

## 2) Local Manual Testing (Backend First, No iOS Required)

### Prerequisites

1. Docker + Docker Compose
2. Rust toolchain (`cargo`)
3. `sqlx-cli` (auto-installed by `just backend-migrate`)

### Step A: Start local DB and apply migrations

From repository root:

```bash
just check-infra-tools
just infra-up
DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/alfred just backend-migrate
```

### Step B: Start API and worker in two terminals

Terminal 1 (API on port 3000 so it matches current iOS default):

```bash
cd backend
export DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/alfred
export DATA_ENCRYPTION_KEY=dev-only-change-me
export CLERK_ISSUER=https://your-tenant.clerk.accounts.dev
export CLERK_AUDIENCE=alfred-api
export CLERK_SECRET_KEY=sk_test_replace_me
export GOOGLE_OAUTH_CLIENT_ID=dev-client-id
export GOOGLE_OAUTH_CLIENT_SECRET=dev-client-secret
export GOOGLE_OAUTH_REDIRECT_URI=http://localhost/oauth/callback
export API_BIND_ADDR=127.0.0.1:3000
export TEE_ATTESTATION_REQUIRED=false
export TEE_ALLOW_INSECURE_DEV_ATTESTATION=true
export TEE_ATTESTATION_DOCUMENT='{}'
cargo run -p api-server
```

Terminal 2 (worker):

```bash
cd backend
export DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/alfred
export DATA_ENCRYPTION_KEY=dev-only-change-me
export CLERK_ISSUER=https://your-tenant.clerk.accounts.dev
export CLERK_AUDIENCE=alfred-api
export CLERK_SECRET_KEY=sk_test_replace_me
export GOOGLE_OAUTH_CLIENT_ID=dev-client-id
export GOOGLE_OAUTH_CLIENT_SECRET=dev-client-secret
export TEE_ATTESTATION_REQUIRED=false
export TEE_ALLOW_INSECURE_DEV_ATTESTATION=true
export TEE_ATTESTATION_DOCUMENT='{}'
cargo run -p worker
```

### Step C: Verify health

```bash
curl -s http://127.0.0.1:3000/healthz
curl -s http://127.0.0.1:3000/readyz
```

Both should return `{"ok":true}`.

### Step D: Export a Clerk bearer token for manual API testing

Use a valid Clerk session token for the configured Clerk application:

```bash
export DEV_ACCESS_TOKEN="<paste-valid-clerk-jwt>"
```

Set helper env var:

```bash
export API=http://127.0.0.1:3000
export AUTH="Authorization: Bearer $DEV_ACCESS_TOKEN"
```

### Step E: Test core authenticated endpoints

Preferences read/write:

```bash
curl -s -H "$AUTH" "$API/v1/preferences"

curl -s -X PUT -H "$AUTH" -H "Content-Type: application/json" \
  "$API/v1/preferences" \
  -d '{
    "meeting_reminder_minutes": 20,
    "morning_brief_local_time": "08:00",
    "quiet_hours_start": "22:00",
    "quiet_hours_end": "07:00",
    "high_risk_requires_confirm": true
  }'
```

APNs device registration + test notification queue:

```bash
curl -s -X POST -H "$AUTH" -H "Content-Type: application/json" \
  "$API/v1/devices/apns" \
  -d '{
    "device_id": "local-device-1",
    "apns_token": "local-apns-token",
    "environment": "sandbox"
  }'

curl -s -X POST -H "$AUTH" -H "Content-Type: application/json" \
  "$API/v1/devices/apns/test" \
  -d '{
    "title": "Manual test",
    "body": "If worker is running, this should be processed."
  }'
```

Audit log and privacy delete:

```bash
curl -s -H "$AUTH" "$API/v1/audit-events"

DELETE_REQ_JSON="$(curl -s -X POST -H "$AUTH" "$API/v1/privacy/delete-all")"
echo "$DELETE_REQ_JSON"

REQUEST_ID="$(echo "$DELETE_REQ_JSON" | sed -n 's/.*"request_id":"\([^"]*\)".*/\1/p')"
curl -s -H "$AUTH" "$API/v1/privacy/delete-all/$REQUEST_ID"
```

### Optional: Google OAuth connector flow

Use this only after configuring real Google OAuth credentials:
1. Set real `GOOGLE_OAUTH_CLIENT_ID`, `GOOGLE_OAUTH_CLIENT_SECRET`, `GOOGLE_OAUTH_REDIRECT_URI`.
2. Call `POST /v1/connectors/google/start`.
3. Open `auth_url`, complete Google consent, and capture the returned `code`.
4. Call `POST /v1/connectors/google/callback` with `code` and `state`.

Without real Google credentials and real auth code, callback completion will fail (expected).

## 3) Local Manual Testing (iOS App)

### What exists today

These are implemented:
1. Sign-in screen (`alfred/alfred/Views/SignInView.swift`)
2. Dashboard for Google connect, preferences, privacy actions, and activity log (`alfred/alfred/Views/DashboardView.swift`)
3. Session manager and Keychain token store (`alfred/alfred/Core/SessionManager.swift`, `alfred/alfred/Core/KeychainSessionTokenStore.swift`)
4. API client package (`alfred/Packages/AlfredAPIClient`)

### Run steps

1. Keep backend API running on `127.0.0.1:3000` (or update `alfred/alfred/Core/AppConfiguration.swift`).
2. Build app:

```bash
just ios-build
```

3. Open and run in simulator:

```bash
just ios-open
```

### Important auth limitation

The backend validates bearer tokens against Clerk JWKS/issuer/audience.
For app sign-in and manual API calls, a valid Clerk token is required.

## 4) Cloud Deployment (Staging First)

There is currently no committed Dockerfile/Terraform/Kubernetes manifest in this repo. Use this minimum shape:

1. Managed Postgres instance
2. API service (`cargo run -p api-server` or compiled binary)
3. Worker service (`cargo run -p worker` or compiled binary)
4. Shared environment/secret management for both services

### Staging environment variables (minimum)

Shared:
1. `DATABASE_URL`
2. `DATA_ENCRYPTION_KEY`
3. `CLERK_ISSUER`
4. `CLERK_AUDIENCE`
5. `CLERK_SECRET_KEY`
6. `CLERK_BACKEND_API_URL` (optional default is `https://api.clerk.com/v1`)
7. `GOOGLE_OAUTH_CLIENT_ID`
8. `GOOGLE_OAUTH_CLIENT_SECRET`
9. `GOOGLE_OAUTH_REDIRECT_URI`
10. `GOOGLE_OAUTH_AUTH_URL` (optional default exists)
11. `GOOGLE_OAUTH_TOKEN_URL` (optional default exists)
12. `GOOGLE_OAUTH_REVOKE_URL` (optional default exists)

API:
1. `API_BIND_ADDR` (for cloud: usually `0.0.0.0:8080`)

Security/TEE:
1. For non-production smoke staging only: `TEE_ATTESTATION_REQUIRED=false`, `TEE_ALLOW_INSECURE_DEV_ATTESTATION=true`, `TEE_ATTESTATION_DOCUMENT={}`
2. For production-like secure mode: provide attestation document source, challenge signing key (`TEE_ATTESTATION_SIGNING_PRIVATE_KEY`), verifier public key (`TEE_ATTESTATION_PUBLIC_KEY`), and keep insecure mode disabled.
3. Set KMS policy inputs: `KMS_KEY_ID`, `KMS_KEY_VERSION`, and `KMS_ALLOWED_MEASUREMENTS`.
4. Production key policy must allow decrypt only for approved attested enclave measurements and deny direct host-role decrypt paths.
5. Use `docs/tee-kms-rotation-runbook.md` and `scripts/security/tee-kms-rotation.sh` for staged key/measurement rotation with evidence capture and rollback.

Worker/APNs (if testing push delivery):
1. `APNS_SANDBOX_ENDPOINT` and/or `APNS_PRODUCTION_ENDPOINT`
2. `APNS_AUTH_TOKEN` (if your APNs proxy endpoint requires bearer auth)

### Post-deploy smoke tests

1. `GET /healthz` and `GET /readyz`
2. Run manual authenticated flow from Section 2 Step E against staging URL
3. Confirm worker is running by observing queued test notification jobs being processed and corresponding audit events

## 5) What To Test First (Recommended Order)

1. Health/readiness
2. Preferences get/update
3. Device registration + test notification queue
4. Audit events pagination
5. Privacy delete request + status
6. Google OAuth start/callback/revoke (once real credentials are wired)

## 6) Known Gaps

1. No turnkey one-command cloud deploy manifest exists yet.
2. App sign-in/manual API calls require a valid Clerk token (no local token mint helper in this repo yet).
3. API default bind port (`8080`) differs from current iOS default base URL (`3000`), so local runs must align one side.
