# Alfred Backend (Rust Workspace)

This workspace contains the Alfred iOS v1 backend services.

## Crates

1. `crates/api-server`: REST API aligned with `api/openapi.yaml` backed by Postgres + `sqlx`.
2. `crates/worker`: scheduler/cron worker loop scaffold.
3. `crates/shared`: shared models and env config.

## Local Infrastructure

From repository root:

```bash
just infra-up
just backend-migrate
```

## Local Environment File (`.env`)

From repository root, create local runtime config:

```bash
cp .env.example .env
```

Security notes:

1. `.env` is ignored by git and must never contain production secrets.
2. `.env.example` contains only safe placeholders for local development.
3. Explicit shell environment variables override `.env` values.

## Run Services

From repository root:

```bash
just api
```

In a second terminal:

```bash
just worker
```

Optional combined startup:

```bash
just dev
```

## Notes

1. API handlers are backed by Postgres + `sqlx` for current v1 endpoints.
2. Migrations are stored under `db/migrations`.
3. Worker execution remains placeholder logic while durable job processing is implemented.
4. Scalability boundary: DB queries live in `backend/crates/shared/src/repos`, and HTTP code lives in `backend/crates/api-server/src/http.rs`.

## Security Runtime Environment

These vars control TEE/KMS-bound decrypt policy for connector refresh tokens:

1. `TEE_ATTESTATION_REQUIRED` (default: `true`)
2. `TEE_EXPECTED_RUNTIME` (default: `nitro`)
3. `TEE_ALLOWED_MEASUREMENTS` (CSV, default: `dev-local-enclave`)
4. `TEE_ATTESTATION_DOCUMENT_PATH` (path to attestation JSON refreshed by trusted runtime)
5. `TEE_ATTESTATION_DOCUMENT` (inline JSON fallback for local/dev only)
6. `TEE_ATTESTATION_PUBLIC_KEY` (base64 Ed25519 public key used for signature verification)
7. `TEE_ATTESTATION_MAX_AGE_SECONDS` (default: `300`)
8. `TEE_ALLOW_INSECURE_DEV_ATTESTATION` (default: `false`; only for local dev without signatures)
9. Secure mode (`TEE_ATTESTATION_REQUIRED=true` and insecure mode disabled) requires `TEE_ATTESTATION_DOCUMENT_PATH`.
10. `KMS_KEY_ID` (default: `kms/local/alfred-refresh-token`)
11. `KMS_KEY_VERSION` (default: `1`)
12. `KMS_ALLOWED_MEASUREMENTS` (CSV; defaults to `TEE_ALLOWED_MEASUREMENTS`)
13. `TRUSTED_PROXY_IPS` (CSV of proxy/LB source IPs; only these peers are allowed to supply forwarded client IP headers for unauthenticated rate limiting)

Connector token usage boundary:

1. Host API/worker crates do not decrypt connector refresh tokens directly.
2. Sensitive Google token refresh/revoke flows execute through the enclave RPC contract in `shared::enclave`.
3. Decrypt requests fail closed when attestation/KMS policy checks fail or connector key metadata drifts.

## Push Delivery Environment (Worker)

Optional worker vars for APNs delivery abstraction:

1. `APNS_SANDBOX_ENDPOINT` (HTTP endpoint used for sandbox device deliveries)
2. `APNS_PRODUCTION_ENDPOINT` (HTTP endpoint used for production device deliveries)
3. `APNS_AUTH_TOKEN` (optional bearer token attached to push delivery requests)

If no endpoint is configured for a device environment, worker delivery is simulated and still logged/audited for local development.
