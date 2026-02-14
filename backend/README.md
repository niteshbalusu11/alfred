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

Default local connection string:

```bash
export DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/alfred
```

## Run Services

From `backend`:

```bash
cargo run -p api-server
```

In a second terminal:

```bash
cargo run -p worker
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
