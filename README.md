# Alfred

Alfred is a hosted, privacy-first AI life assistant.

Phase I is intentionally narrow and focused on iOS + Google:

1. Meeting reminders from Google Calendar
2. Daily morning brief
3. Urgent Gmail alerts

## Why Alfred

Alfred is not a generic chatbot. It is a proactive assistant designed for reliability, privacy, and user control.

Core principles:

1. Privacy before convenience
2. Reliability over feature breadth
3. Minimal permissions by default
4. Explainability and user control (audit, revoke, delete-all)

## Architecture At A Glance

1. iOS app (`SwiftUI`)
2. Rust API server (`axum`)
3. Rust worker (`tokio`) for scheduled/proactive jobs
4. Encrypted Postgres for operational state
5. TEE-backed sensitive decrypt/execution path
6. APNs notification delivery

## Repository Layout

1. iOS app: `alfred`
2. iOS API package: `alfred/Packages/AlfredAPIClient`
3. Backend workspace: `backend`
4. OpenAPI contract: `api/openapi.yaml`
5. DB migrations: `db/migrations`
6. Product context: `docs/product-context.md`
7. Engineering standards: `docs/engineering-standards.md`
8. Phase I board: `docs/phase1-master-todo.md`

## Prerequisites

Required:

1. macOS + Xcode (`xcodebuild`)
2. Rust toolchain (`cargo`)
3. Swift toolchain (`swift`)
4. `just` command runner

Optional (backend DB work):

1. Docker + Docker Compose

## Quick Start

Run all commands from repository root.

1. Validate toolchain:

```bash
just check-tools
```

2. Baseline build checks:

```bash
just backend-check
just ios-build
```

3. Start local Postgres when backend work needs DB access:

```bash
just check-infra-tools
just infra-up
export DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/alfred
just backend-migrate
```

4. Run backend services:

```bash
just backend-api
just backend-worker
```

Or run both together:

```bash
just dev
```

5. Open iOS project:

```bash
just ios-open
```

## Common Commands

1. `just ios-build` - Build iOS app
2. `just ios-test` - Run iOS tests
3. `just ios-package-build` - Build `AlfredAPIClient` package
4. `just backend-check` - Compile-check backend
5. `just backend-verify` - Backend completion gate (`fmt + clippy + tests + build`)
6. `just backend-deep-review` - Backend verify + security + bug + architecture checks
7. `just infra-up` / `just infra-stop` / `just infra-down` - Manage local Postgres
8. `just docs` - Print key project documentation paths

## Quality Gates

Backend changes are complete only when all pass:

1. `just backend-fmt`
2. `just backend-clippy`
3. `just backend-test`
4. `just backend-build`

Preferred one-shot command:

```bash
just backend-verify
```

iOS policy:

1. Always run `just ios-build`
2. Run `just ios-test` when iOS core logic changes

## Security And Scalability Guardrails

1. Do not store secrets/tokens in plaintext.
2. Do not log sensitive values.
3. Keep OAuth scopes minimal.
4. Use migrations for schema changes.
5. Keep API/server/client contracts aligned with `api/openapi.yaml`.

Required architecture boundaries:

1. DB repository code lives in `backend/crates/shared/src/repos`
2. HTTP routing/handlers/middleware live in `backend/crates/api-server/src/http.rs` (or future `/http/*` modules)
3. `main.rs` files stay bootstrap/startup focused

## Execution Workflow

Work is issue-driven.

1. Source of truth: GitHub issues (`phase-1`, prioritized `P0` then `P1`)
2. Branch naming prefix: `codex/` (example: `codex/issue-42-short-slug`)
3. Keep `docs/phase1-master-todo.md` aligned with issue status
4. Keep scope tightly aligned to issue acceptance criteria

## API Surface (Phase I)

The v1 OpenAPI contract is in `api/openapi.yaml` and includes:

1. iOS session auth
2. APNs device registration
3. Google connector start/callback/revoke
4. Preferences read/update
5. Audit event listing
6. Privacy delete-all request

## Current State

1. API server is wired to Postgres-backed v1 endpoint surfaces.
2. Worker currently has placeholder execution while durable job processing is completed.
3. iOS app builds; full production backend integration in UI is still in progress.

## Key Documents

1. Product context: `docs/product-context.md`
2. Agent start guide: `agent/start.md`
3. Engineering standards: `docs/engineering-standards.md`
4. RFC: `docs/rfc-0001-alfred-ios-v1.md`
5. Threat model: `docs/threat-model-phase1.md`
6. AI review template: `docs/ai-review-template.md`
7. Issue update template: `docs/issue-update-template.md`
