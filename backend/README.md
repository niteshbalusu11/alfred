# Alfred Backend (Rust Workspace)

This workspace contains the Alfred iOS v1 backend services.

## Crates

1. `crates/api-server`: REST API aligned with `api/openapi.yaml` backed by Postgres + `sqlx`.
2. `crates/worker`: scheduled/proactive job execution (lease/retry/idempotency, push dispatch, privacy delete workflows).
3. `crates/enclave-runtime`: enclave runtime baseline process with health and attestation endpoints.
4. `crates/shared`: shared models, repositories, security runtime, and LLM gateway modules.

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
3. Worker execution includes durable processing primitives (lease ownership, retry classification, idempotency keys, and dead-letter handling).
4. Scalability boundary: DB queries live in `backend/crates/shared/src/repos`, and HTTP routing/handlers live under `backend/crates/api-server/src/http/*`.

## Security Runtime Environment

These vars control TEE/KMS-bound decrypt policy for connector refresh tokens:

1. `TEE_ATTESTATION_REQUIRED` (default: `true`)
2. `TEE_EXPECTED_RUNTIME` (default: `nitro`)
3. `TEE_ALLOWED_MEASUREMENTS` (CSV, default: `dev-local-enclave`)
4. `TEE_ATTESTATION_PUBLIC_KEY` (base64 Ed25519 public key used for challenge response signature verification)
5. `TEE_ATTESTATION_MAX_AGE_SECONDS` (default: `300`)
6. `TEE_ATTESTATION_CHALLENGE_TIMEOUT_MS` (default: `2000`)
7. `TEE_ALLOW_INSECURE_DEV_ATTESTATION` (default: `false`; only for local dev without signatures)
8. `KMS_KEY_ID` (default: `kms/local/alfred-refresh-token`)
9. `KMS_KEY_VERSION` (default: `1`)
10. `KMS_ALLOWED_MEASUREMENTS` (CSV; defaults to `TEE_ALLOWED_MEASUREMENTS`)
11. `TRUSTED_PROXY_IPS` (CSV of proxy/LB source IPs; only these peers are allowed to supply forwarded client IP headers for unauthenticated rate limiting)
12. `ALFRED_ENV` (`local`, `staging`, `production`; default: `local`)
13. `ENCLAVE_RUNTIME_MODE` (`dev-shim`, `remote`, `disabled`; non-local requires `remote`)
14. `ENCLAVE_RUNTIME_BASE_URL` (default: `http://127.0.0.1:8181`)
15. `ENCLAVE_RUNTIME_PROBE_TIMEOUT_MS` (default: `2000`)
16. `ENCLAVE_RUNTIME_BIND_ADDR` (enclave runtime process bind address; default: `127.0.0.1:8181`)
17. `ENCLAVE_RUNTIME_MEASUREMENT` (dev-shim measurement identifier; default: `dev-local-enclave`)
18. `TEE_ATTESTATION_SIGNING_PRIVATE_KEY` (base64 Ed25519 private key used by enclave runtime to sign challenge-bound attestation evidence)
19. `TEE_ATTESTATION_DOCUMENT_PATH` (remote-mode enclave runtime attestation identity source)
20. `TEE_ATTESTATION_DOCUMENT` (inline remote-mode attestation identity source for local smoke setups)

Connector token usage boundary:

1. API/worker handler modules do not call connector decrypt repository APIs directly.
2. Sensitive Google token refresh/revoke flows execute through the enclave RPC contract in `shared::enclave`.
3. Decrypt authorization fails closed when challenge-bound attestation verification/KMS policy checks fail or connector key metadata drifts.
4. API/worker startup now performs fail-closed connectivity checks against enclave runtime `GET /healthz`, `GET /v1/attestation/document`, and `POST /v1/attestation/challenge`.
5. Enclave decrypt flow re-reads connector key metadata from storage and does not trust host-provided key metadata in RPC requests.

Enclave runtime commands:

1. `just backend-enclave-runtime`
2. `just enclave-runtime`
3. `scripts/enclave-runtime/start-local.sh`
4. `scripts/enclave-runtime/smoke.sh`

## Push Delivery Environment (Worker)

Optional worker vars for APNs delivery abstraction:

1. `APNS_SANDBOX_ENDPOINT` (HTTP endpoint used for sandbox device deliveries)
2. `APNS_PRODUCTION_ENDPOINT` (HTTP endpoint used for production device deliveries)
3. `APNS_AUTH_TOKEN` (optional bearer token attached to push delivery requests)

If no endpoint is configured for a device environment, worker delivery is simulated and still logged/audited for local development.

## OpenRouter LLM Environment

These vars are validated at API and worker startup for the LLM backend path:

1. `OPENROUTER_API_KEY` (required)
2. `OPENROUTER_CHAT_COMPLETIONS_URL` (default: `https://openrouter.ai/api/v1/chat/completions`)
3. `OPENROUTER_HTTP_REFERER` (optional, sent as `HTTP-Referer` for OpenRouter app attribution)
4. `OPENROUTER_APP_TITLE` (optional, sent as `X-Title` for OpenRouter app attribution)
5. `OPENROUTER_TIMEOUT_MS` (default: `15000`)
6. `OPENROUTER_MAX_RETRIES` (default: `2`)
7. `OPENROUTER_RETRY_BASE_BACKOFF_MS` (default: `250`)
8. `OPENROUTER_MODEL_PRIMARY`
9. `OPENROUTER_MODEL_FALLBACK`

If model vars are omitted, backend falls back to built-in defaults:
`openai/gpt-4o-mini` (primary) and `anthropic/claude-3.5-haiku` (fallback).

## LLM Reliability Guardrails

These vars control runtime reliability protections for LLM requests:

1. `LLM_RATE_LIMIT_WINDOW_SECONDS` (default: `60`)
2. `LLM_RATE_LIMIT_GLOBAL_MAX_REQUESTS` (default: `120`)
3. `LLM_RATE_LIMIT_PER_USER_MAX_REQUESTS` (default: `30`)
4. `LLM_CIRCUIT_BREAKER_FAILURE_THRESHOLD` (default: `5`)
5. `LLM_CIRCUIT_BREAKER_COOLDOWN_SECONDS` (default: `60`)
6. `LLM_CACHE_TTL_SECONDS` (default: `20`)
7. `LLM_CACHE_MAX_ENTRIES` (default: `256`)
8. `LLM_BUDGET_WINDOW_SECONDS` (default: `3600`)
9. `LLM_BUDGET_MAX_ESTIMATED_COST_USD` (default: `1.0`)
10. `LLM_BUDGET_MODEL` (default: `openai/gpt-4o-mini`)

Behavior notes:

1. Reliability state is Redis-backed (via `REDIS_URL`) and shared across API/worker instances.
2. Requests are limited globally and per-user per window.
3. Repeated provider failures open a circuit breaker and fail closed until cooldown elapses.
4. Successful responses are cached in Redis for short-lived duplicate prompts, surviving process restarts.
5. When budget window spend reaches threshold, requests route to `LLM_BUDGET_MODEL` until the window resets.
6. API/worker startup fails fast if Redis reliability state cannot initialize.

## LLM Eval Harness

Deterministic eval/regression checks for assistant quality and safety are provided by
`llm-eval` (`backend/crates/llm-eval`).

Local commands from repo root:

1. `just backend-eval`
   1. Runs mocked deterministic evals with fixture-driven assertions and golden snapshot checks.
2. `just backend-eval-update`
   1. Intentionally rewrites mocked-mode goldens when prompt/schema/safety changes are expected.
3. `just backend-eval-live`
   1. Optional live OpenRouter smoke mode (requires `OPENROUTER_*` env vars).

Interpretation:

1. `schema_validity` failures indicate output contract/schema regressions.
2. `safe_output_source` failures indicate policy violations that triggered deterministic fallback.
3. `quality` failures indicate content quality regressions (for example empty summaries/actions).
4. `golden_snapshot` failures indicate deterministic prompt/output drift and require intentional review.

Fixture layout:

1. Case fixtures: `backend/crates/llm-eval/fixtures/cases`
2. Goldens: `backend/crates/llm-eval/fixtures/goldens`

CI behavior:

1. `.github/workflows/ci.yml` runs mocked mode as part of backend checks.
2. Live mode remains opt-in for local/provider smoke testing.
