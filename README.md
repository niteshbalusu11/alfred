# Alfred

Alfred is a hosted, privacy-first AI life assistant for iOS.

It is designed for proactive help, not generic chat. Phase I focuses on Google-integrated workflows that save attention every day while keeping strict user control over data.

## What Alfred Is

Alfred combines:

1. An iOS app for onboarding, settings, and notifications.
2. A Rust API + worker backend for connectors, preferences, privacy, and automation.
3. LLM-backed assistant behavior (via OpenRouter) with deterministic safety fallback.
4. A TEE-sensitive execution path for protected decrypt/process flows.

Phase I user outcomes:

1. Google Calendar meeting reminders.
2. Daily morning brief.
3. Urgent Gmail alerts.
4. Natural-language assistant questions over connected Google context.

## Current Status (As Of 2026-02-18)

This repo is in active Phase I private-beta execution.

Completed migration lines:

1. LLM-first backend migration (`#91` through `#103`).
2. Semantic planner assistant routing migration (`#180`).
3. Content-blindness boundary migration (`#146` through `#149`).

Still in progress:

1. Clerk auth migration and legacy auth endpoint retirement (`#52`, `#53`, `#54`, `#56`).
2. Remaining Phase I board work across product, API, APNs, QA, and launch readiness.
3. External security assessment/remediation track before beta readiness.

Source-of-truth status lives in:

1. GitHub issues in `niteshbalusu11/alfred` (`phase-1`, then `P0` before `P1`).
2. `docs/phase1-master-todo.md` (planning/control board).

If the board and an issue conflict, GitHub issues are the immediate execution source.

## Architecture Snapshot

Core runtime components:

1. iOS app (`SwiftUI`): product UX.
2. API server (`Rust + axum`): auth, connectors, preferences, privacy, audit APIs.
3. Worker (`Rust + tokio`): scheduled/proactive processing.
4. Enclave runtime: sensitive processing boundary.
5. Postgres + Redis: operational persistence and reliability state.
6. Google APIs + APNs: external integrations.

## Privacy and Security Direction

Non-negotiable baseline:

1. Least-privilege OAuth scopes.
2. Encrypted secret/token storage.
3. No plaintext message-body persistence in server control-plane paths.
4. Enclave-only plaintext handling for protected assistant flows.
5. Redacted logs + auditability.
6. User revoke and delete-all controls.

See:

1. `docs/product-context.md`
2. `docs/engineering-standards.md`
3. `docs/content-blindness-invariants.md`
4. `docs/threat-model-phase1.md`

## Repository Map

1. iOS app: `alfred`
2. iOS API package: `alfred/Packages/AlfredAPIClient`
3. Backend workspace: `backend`
4. OpenAPI contract: `api/openapi.yaml`
5. DB migrations: `db/migrations`
6. Product context: `docs/product-context.md`
7. Agent/contributor start: `agent/start.md`
8. Phase I board: `docs/phase1-master-todo.md`

## Local Development Quick Start

Run from repo root.

1. Validate local tools:

```bash
just check-tools
just check-infra-tools
```

2. Create env file:

```bash
cp .env.example .env
```

3. Start local infra and apply migrations:

```bash
just infra-up
just backend-migrate
```

4. Start runtime services (recommended: separate terminals):

```bash
just enclave-runtime
just api
just worker
```

Optional (single terminal, includes `ngrok`):

```bash
just dev
```

5. Stop infra when done:

```bash
just infra-stop
```

Remove volumes too:

```bash
just infra-down
```

## Build, Test, and Quality Gates

Common commands:

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

Deep review gate for backend-impacting issues:

```bash
just backend-deep-review
```

## Contribution Workflow

Required workflow is issue-driven:

1. Select from GitHub issues with `phase-1`.
2. Priority order: `P0` first, then `P1`; lowest issue number first unless blocked.
3. Use `codex/` branch prefixes.
4. Keep scope aligned to issue acceptance criteria.
5. Update issue status/comments and keep board alignment.

Detailed process:

1. `AGENTS.md`
2. `agent/start.md`
3. `docs/issue-update-template.md`

## Reference Docs

1. Product context: `docs/product-context.md`
2. Engineering standards: `docs/engineering-standards.md`
3. Agent start guide: `agent/start.md`
4. UI source of truth: `docs/ui-spec.md`
5. OpenAPI contract: `api/openapi.yaml`
6. Cloud/local testing: `docs/cloud-deployment-local-testing.md`
