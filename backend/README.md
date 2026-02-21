# Alfred Backend (Rust Workspace)

This workspace contains the Alfred iOS v1 backend services.

## Crates

1. `crates/api-server`: REST API aligned with `api/openapi.yaml` backed by Postgres + `sqlx`.
2. `crates/worker`: scheduled/proactive job execution (lease/retry/idempotency, push dispatch, privacy delete workflows), now migrating to generic Automation v2 scheduling/execution.
3. `crates/enclave-runtime`: enclave runtime baseline process with health and attestation endpoints.
4. `crates/shared`: shared models, repositories, security runtime, and LLM gateway modules.

## Active Migration: Automation v2 (Breaking)

Source-of-truth tracker: GitHub issue `#208` with execution issues `#209` through `#214`.

Target backend changes:

1. Replace hardcoded proactive job actions (`MeetingReminder`, `MorningBrief`, `UrgentEmailCheck`) with generic `AUTOMATION_RUN`.
2. Persist automation schedule metadata with sealed prompt ciphertext only.
3. Materialize due runs with deterministic idempotency keying and lease-safe multi-worker behavior.
4. Execute automation prompt workflows through enclave RPC so host never receives plaintext prompt/output.
5. Emit encrypted APNs payloads for iOS Notification Service Extension decrypt/render.

Migration policy:

1. No feature flags.
2. No backward compatibility layer for legacy proactive runtime behavior.
3. Preserve reliability invariants: lease ownership, deterministic retries, idempotency, dead-letter behavior.
4. Preserve privacy boundary: metadata-only host logging/audit; no automation plaintext persistence.

## Local Infrastructure

From repository root:

```bash
just check-tools
just check-infra-tools
just infra-up
just backend-migrate
```

One-shot local backend test workflow:

```bash
just backend-tests
```

This command runs infra checks, starts Postgres/Redis, applies migrations,
runs backend tests + mocked evals, then stops infra.
It uses an isolated `alfred_test` Postgres database by default so test resets
do not wipe local app-development data in `alfred`.

## Local Environment File (`.env`)

From repository root, create local runtime config:

```bash
cp .env.example .env
```

Security notes:

1. `.env` is ignored by git and must never contain production secrets.
2. `.env.example` contains only safe placeholders for local development.
3. Explicit shell environment variables override `.env` values.
4. `API_HTTP_TIMEOUT_MS` controls API upstream request timeout (including enclave RPC); default is `60000`.

## Run Services (Local Quick Start)

From repository root:

1. Start enclave runtime in terminal A:

```bash
just enclave-runtime
```

2. Start API server in terminal B:

```bash
just api
```

3. Start worker in terminal C:

```bash
just worker
```

The three commands above now run through `bacon` with restart-on-change enabled.

4. Verify health:

```bash
curl -s http://127.0.0.1:8080/healthz
curl -s http://127.0.0.1:8080/readyz
curl -s http://127.0.0.1:8181/healthz
```

Expected API responses include `{"ok":true}`.

5. Stop infrastructure when done:

```bash
just infra-stop
```

If you want to remove local volumes too:

```bash
just infra-down
```

Optional combined startup (API + worker + enclave + ngrok in one terminal):

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
21. `ENCLAVE_RPC_SHARED_SECRET` (shared secret for signed host↔enclave RPC request authentication; required outside local)
22. `ENCLAVE_RPC_AUTH_MAX_SKEW_SECONDS` (default: `30`; max allowed timestamp skew for signed RPC requests)
23. `ASSISTANT_INGRESS_ACTIVE_KEY_ID` (default: `assistant-ingress-v1`; key id advertised to clients for assistant ingress encryption)
24. `ASSISTANT_INGRESS_ACTIVE_PRIVATE_KEY` (base64 X25519 private key for active assistant ingress decryption key; required outside local)
25. `ASSISTANT_INGRESS_PREVIOUS_KEY_ID` (optional previous key id accepted for decrypt during key rotation grace windows)
26. `ASSISTANT_INGRESS_PREVIOUS_PRIVATE_KEY` (optional previous base64 X25519 private key paired with previous key id)
27. `ASSISTANT_INGRESS_PREVIOUS_KEY_EXPIRES_AT` (unix timestamp for previous key expiry; required outside local when previous key is configured)
28. `ASSISTANT_INGRESS_KEY_TTL_SECONDS` (default: `900`; rolling attested-key expiry horizon returned to clients for the active ingress key)
29. `ASSISTANT_INGRESS_SESSION_TTL_SECONDS` (default: `5184000`; encrypted assistant session-state persistence TTL, 60 days)

Non-local (`ALFRED_ENV=staging|production`) security guards:

1. `ENCLAVE_RUNTIME_MODE` must be `remote`.
2. `TEE_ATTESTATION_REQUIRED=true` and `TEE_ALLOW_INSECURE_DEV_ATTESTATION=false`.
3. `TEE_ALLOWED_MEASUREMENTS` and `KMS_ALLOWED_MEASUREMENTS` must not contain `dev-local-enclave`.
4. `ENCLAVE_RUNTIME_BASE_URL` must use `https`, or loopback `http` only (`127.0.0.1`, `localhost`, `[::1]`).
5. Enclave runtime rejects inline `TEE_ATTESTATION_DOCUMENT` outside local.

Connector token usage boundary:

1. API/worker handler modules do not call connector decrypt repository APIs directly.
2. Host runtimes use signed enclave RPC requests (`POST /v1/rpc/google/token/exchange` and `POST /v1/rpc/google/token/revoke`) with nonce + timestamp replay protections.
3. Sensitive Google token refresh/revoke flows execute only through enclave runtime handlers.
4. Decrypt authorization fails closed when challenge-bound attestation verification/KMS policy checks fail or connector key metadata drifts.
5. API/worker startup performs fail-closed connectivity checks against enclave runtime `GET /healthz`, `GET /v1/attestation/document`, and `POST /v1/attestation/challenge`.
6. Enclave decrypt flow re-reads connector key metadata from storage and does not trust host-provided key metadata in RPC requests.

Enclave runtime commands:

1. `just backend-enclave-runtime`
2. `just enclave-runtime`
3. `scripts/enclave-runtime/start-local.sh`
4. `scripts/enclave-runtime/smoke.sh`

## Push Delivery Environment (Worker)

Required worker vars for direct APNs delivery:

1. `APNS_KEY_ID` (Apple APNs key identifier)
2. `APNS_TEAM_ID` (Apple Developer Team identifier)
3. `APNS_TOPIC` (bundle identifier used as APNs topic, e.g. `com.prodata.alfred`)
4. One APNs private-key source:
   1. `APNS_AUTH_KEY_P8` (inline PEM; supports `\\n` escaped newlines), or
   2. `APNS_AUTH_KEY_P8_BASE64` (base64-encoded full `.p8` file), or
   3. `APNS_AUTH_KEY_P8_PATH` (absolute path to `.p8` file)
5. `WORKER_ASSISTANT_SESSION_PURGE_BATCH_SIZE` (default: `200`; bounded expired assistant-session rows purged per worker tick)

Worker sends directly to Apple APNs:

1. sandbox devices -> `https://api.sandbox.push.apple.com/3/device/{token}`
2. production devices -> `https://api.push.apple.com/3/device/{token}`

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
