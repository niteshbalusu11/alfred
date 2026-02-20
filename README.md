# Alfred

> Batman's loyal butler.

Alfred is a privacy-first AI assistant for iOS with a hosted Rust backend.

The product goal is proactive help over Google data (calendar + email) with strong privacy boundaries, not a generic chat app.

## Active Migration: Automation v2 (Breaking)

The current execution epic is `#208`: client-defined periodic prompt automations that replace hardcoded proactive worker jobs.

Scope tracked in `#209` through `#214`:

1. Automation rule/run schema and repository APIs.
2. Automation CRUD API endpoints with encrypted prompt envelope input.
3. Worker scheduler + generic `AUTOMATION_RUN` execution path.
4. Enclave automation execution with encrypted notification artifact output.
5. Encrypted APNs payload contract for automation notifications.
6. iOS Notification Service Extension decrypt/render path.

Migration rules:

1. No feature flags.
2. No backwards compatibility for legacy hardcoded proactive job behavior.
3. No host plaintext prompt/output persistence or logging.

## What Exists In Code Today

### Backend (`backend/`)

1. Clerk-authenticated API server (`axum`) with routes for:
   1. Assistant: `/v1/assistant/attested-key`, `/v1/assistant/query`
   2. Connectors: Google OAuth start/callback/list/revoke
   3. Devices: APNs registration and test notification enqueue
   4. Preferences, audit events, and privacy delete-all
2. Encrypted assistant query flow:
   1. Attested key challenge/response
   2. Encrypted request/response envelopes
   3. Encrypted session continuity state
3. Enclave runtime orchestration for assistant and connector-sensitive operations.
4. Worker pipeline with leased/idempotent jobs for:
   1. Active v1 runtime includes meeting reminders, morning brief generation, and urgent email alerts.
   2. These hardcoded paths are being replaced by Automation v2 generic scheduling/execution (`#208`).
5. LLM-backed assistant routing/orchestration with deterministic fallback and safety validation.

### Assistant Capabilities

The assistant supports these lanes in code:

1. `calendar_lookup`
2. `email_lookup`
3. `mixed`
4. `general_chat`
5. `meetings_today`

Routing is planner-driven (semantic plan + policy), with:

1. Clarification flow for low-confidence tool requests
2. English-first language policy
3. Deterministic fallback when planner/model output is unavailable or invalid

### iOS App (`alfred/`)

Current tab shell and primary UX:

1. `Home`: voice transcription + assistant conversation
2. `Activity`: audit/event timeline
3. `Connectors`: Google connect/disconnect flow

Authentication and API access are Clerk-based. The app uses encrypted assistant query APIs and renders structured assistant response parts (chat text + tool summaries).

Note:
Backend preferences and privacy delete-all APIs are implemented; current iOS navigation is focused on Home/Activity/Connectors.

## Repository Map

1. iOS app: `alfred`
2. iOS API package: `alfred/Packages/AlfredAPIClient`
3. Backend workspace: `backend`
4. OpenAPI contract: `api/openapi.yaml`
5. DB migrations: `db/migrations`
6. Product context: `docs/product-context.md`
7. Agent/contributor workflow: `AGENTS.md`, `agent/start.md`

## Local Development

Run from repo root:

```bash
just check-tools
just check-infra-tools
cp .env.example .env
just infra-up
just backend-migrate
```

Start services (recommended in separate terminals):

```bash
just enclave-runtime
just api
just worker
```

Optional combined dev mode (also starts `ngrok`):

```bash
just dev
```

Stop infra:

```bash
just infra-stop
```

Destroy infra volumes:

```bash
just infra-down
```

## Build and Verification

```bash
just backend-check
just backend-tests
just backend-verify
just ios-build
just ios-test
```

Backend completion gate for backend-impacting changes:

1. `just backend-fmt`
2. `just backend-clippy`
3. `just backend-tests`
4. `just backend-build`

Deep review gate:

```bash
just backend-deep-review
```

## Workflow and Source Of Truth

Execution is issue-driven (`phase-1`, prioritize `P0` before `P1`) with `codex/` branches.

Primary references:

1. `AGENTS.md`
2. `agent/start.md`
3. `docs/engineering-standards.md`
4. `docs/phase1-master-todo.md`
5. `docs/ui-spec.md`
