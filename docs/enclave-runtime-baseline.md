# Enclave Runtime Baseline (SEC-002)

## Purpose

This runbook defines the baseline enclave runtime process for Phase I and the smoke checks required by SEC-002.

## Service Contract

The enclave runtime is a separate process (`backend/crates/enclave-runtime`) that exposes:

1. `GET /healthz`
2. `GET /v1/attestation/document`
3. `POST /v1/attestation/challenge`
4. `POST /v1/rpc/google/token/exchange`
5. `POST /v1/rpc/google/token/revoke`

API and worker startup now perform a fail-closed connectivity probe against these endpoints.

## Local Development (Dev Shim)

Prerequisites:

1. `.env` from `.env.example`
2. `ENCLAVE_RUNTIME_BASE_URL=http://127.0.0.1:8181`
3. `ALFRED_ENV=local`
4. `ENCLAVE_RUNTIME_MODE=dev-shim`
5. `TEE_ATTESTATION_REQUIRED=false`
6. `TEE_ALLOW_INSECURE_DEV_ATTESTATION=true`
7. (optional) `TEE_ATTESTATION_SIGNING_PRIVATE_KEY` for custom dev signing identity
8. `ENCLAVE_RPC_SHARED_SECRET` (for local, default fallback is `local-dev-enclave-rpc-secret`)
9. `ENCLAVE_RPC_AUTH_MAX_SKEW_SECONDS=30`

Start enclave runtime:

```bash
scripts/enclave-runtime/start-local.sh
```

Smoke check:

```bash
scripts/enclave-runtime/smoke.sh
```

Then run host services (which will verify enclave connectivity at startup):

```bash
just api
just worker
```

Or combined:

```bash
just dev
```

## Staging / Production-Like Mode

Required guardrails:

1. `ALFRED_ENV=staging` (or `production`)
2. `ENCLAVE_RUNTIME_MODE=remote`
3. `TEE_ATTESTATION_DOCUMENT_PATH` or `TEE_ATTESTATION_DOCUMENT` is set
4. `TEE_ATTESTATION_REQUIRED=true`
5. `TEE_ALLOW_INSECURE_DEV_ATTESTATION=false`
6. `TEE_ATTESTATION_SIGNING_PRIVATE_KEY` is configured

In staging/production environments, `ENCLAVE_RUNTIME_MODE=dev-shim` and `ENCLAVE_RUNTIME_MODE=disabled` are rejected by config validation.

## Packaging and Startup Path

Build enclave runtime artifact:

```bash
cd backend
cargo build -p enclave-runtime
```

Run enclave runtime artifact:

```bash
cd backend
cargo run -p enclave-runtime
```

## Verification Checklist

1. Enclave runtime process is running as a distinct binary/process.
2. `GET /healthz` returns `200`.
3. `GET /v1/attestation/document` returns `200` with JSON payload.
4. `POST /v1/attestation/challenge` returns `200` with signed challenge-bound evidence.
5. API starts successfully with enclave connectivity check enabled.
6. Worker starts successfully with enclave connectivity check enabled.
7. Enclave RPC endpoints reject malformed or unsigned host requests fail-closed.
