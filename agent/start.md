# Alfred Agent Start Guide

## Purpose

This file is the operational and technical entry point for agents working in this repository.
Use it to understand the architecture quickly and run the system with consistent commands.

Read this first for product/business context:

`docs/product-context.md`

Read mandatory security/scalability rules:

`docs/engineering-standards.md`

Use this template for required AI review reporting before merge:

`docs/ai-review-template.md`

## Project Summary

Alfred is a privacy-first iOS assistant product with a hosted backend.
Current v1 scope:

1. Google Calendar meeting reminders.
2. Daily morning brief.
3. Urgent Gmail alerting.

The project intentionally avoids smart-home control in v1 to reduce reliability and liability risk.

## Repository Map

1. iOS app project:
   1. `alfred`
2. Local Swift package used by iOS client code:
   1. `alfred/Packages/AlfredAPIClient`
3. Backend Rust workspace:
   1. `backend`
4. API contract:
   1. `api/openapi.yaml`
5. DB migrations:
   1. `db/migrations`
6. Product/architecture RFC:
   1. `docs/rfc-0001-alfred-ios-v1.md`
7. Phase I master board:
   1. `docs/phase1-master-todo.md`
8. Product context (canonical):
   1. `docs/product-context.md`

## Runtime Components

1. iOS App (`SwiftUI`):
   1. UX surface for authentication, connector setup, settings, and notifications.
2. API Server (`Rust + axum`):
   1. Serves v1 REST endpoints aligned with OpenAPI.
3. Worker (`Rust + tokio`):
   1. Periodic processing loop for scheduled/proactive jobs.
4. Shared crate:
   1. Shared request/response models and basic runtime config.

## GitHub Issue-Driven Execution

Primary execution queue is GitHub issues in:

`https://github.com/niteshbalusu11/alfred/issues`

Phase I labels currently in use:

1. Scope: `phase-1`
2. Priority: `P0`, `P1`
3. Domain tags: `backend`, `ios`, `security`, `database`, `oauth`, `tee`, `worker`, `notifications`, `privacy`, `observability`, `sre`, `qa`

### Issue Selection Algorithm (Required)

1. Select open issues with `phase-1`.
2. Prioritize:
   1. `P0` before `P1`
   2. Lowest issue number first unless blocked by explicit dependency
3. If blocked:
   1. Comment blocker details on issue
   2. Move to next unblocked issue

### Issue Execution Protocol

1. Read issue acceptance criteria before coding.
2. Create branch with prefix `codex/`:
   1. Example: `codex/issue-4-worker-leasing`
3. Implement only acceptance-criteria scope.
4. Run validation commands from `Justfile`.
5. Update issue with:
   1. What changed
   2. Validation results
   3. Remaining follow-ups
6. Use standard comment formats from:
   1. `docs/issue-update-template.md`

### PR Lifecycle Protocol

1. Open PR targeting `master`.
2. Wait for CI checks:
   1. `.github/workflows/ci.yml` (`Backend Checks`, `iOS Build`)
3. Complete AI review report (security + bug check + scalability/code quality) before merge.
4. Merge only after checks pass and AI review is documented.
5. Once merged:
   1. run `just sync-master`
   2. pick next highest-priority unblocked issue

### Sync Rule: GitHub Issues vs Phase I Board

1. GitHub issue is the immediate execution source.
2. `docs/phase1-master-todo.md` is the planning/control board.
3. If either side changes, keep them aligned in the same work cycle.
4. Do not start untracked work; create/obtain an issue first.

## Security and Privacy Constraints

Agents must preserve these constraints while implementing features:

1. No plaintext long-term storage of connector data (tokens/data should be encrypted at rest).
2. Data minimization for v1:
   1. Persist only minimal state needed for scheduling, retries, and auditing.
3. API changes must remain aligned with:
   1. `api/openapi.yaml`
4. Schema changes must be migration-driven:
   1. Add/modify SQL files under `db/migrations`.

## Justfile Command Reference

All operational commands are centralized in:

`Justfile`

Run from repository root:

```bash
cd .
```

Primary commands:

1. `just check-tools`
   1. Verifies local toolchain (`xcodebuild`, `cargo`, `swift`).
2. `just check-infra-tools`
   1. Verifies local infra tooling (`docker`, `docker compose`).
3. `just infra-up`
   1. Starts local Postgres from `docker-compose.yml`.
4. `just infra-stop`
   1. Stops Postgres without deleting volumes.
5. `just infra-down`
   1. Stops Postgres and removes volumes.
6. `just backend-migrate`
   1. Applies SQL migrations from `db/migrations`.
7. `just backend-migrate-check`
   1. Prints migration status for configured `DATABASE_URL`.
8. `just ios-open`
   1. Opens the Xcode project.
9. `just ios-build`
   1. Builds iOS app for simulator.
10. `just ios-test`
   1. Runs iOS tests on default simulator destination.
11. `just ios-package-build`
   1. Builds the local Swift package (`AlfredAPIClient`).
12. `just backend-check`
   1. Runs Rust compile checks.
13. `just backend-build`
   1. Builds Rust backend workspace.
14. `just backend-test`
   1. Runs Rust tests.
15. `just backend-fmt`
   1. Formats Rust code.
16. `just backend-clippy`
    1. Runs lint checks with warnings denied.
17. `just backend-verify`
    1. Runs backend completion gate: fmt + clippy + tests + build.
18. `just backend-security-audit`
    1. Runs dependency vulnerability audit (`cargo audit`).
19. `just backend-bug-check`
    1. Runs backend tests and fails on placeholder/debug macros.
20. `just backend-architecture-check`
    1. Enforces DB/HTTP layer boundaries for scalability.
21. `just backend-deep-review`
    1. Runs backend verify + security audit + bug check + architecture checks.
22. `just backend-api`
    1. Runs REST API server.
23. `just backend-worker`
    1. Runs background worker.
24. `just dev`
    1. Runs API server + worker together.
25. `just docs`
    1. Prints key project documentation paths.
26. `just sync-master`
    1. Fetches remote, checks out `master`, and fast-forward pulls latest.

## Test and Quality Policy (Strict)

### Backend (Rust)

When backend code changes, unit/integration tests are required.

Required completion checks:

1. `just backend-fmt`
2. `just backend-clippy`
3. `just backend-test`
4. `just backend-build`

Important:

1. `cargo fmt` only formats code.
2. `cargo fmt` does **not** replace linting or testing.
3. Backend task is not done until linting, tests, and build all pass.
4. Preferred one-shot command:
   1. `just backend-verify`

### Frontend (iOS)

1. If changing core logic (state management, API client logic, data transforms), add/update Swift tests.
2. For UI-only changes in current phase, UI tests are optional and not required by default.
3. Minimum iOS completion check:
   1. `just ios-build`
4. Run `just ios-test` when logic tests were added/changed or when explicitly requested.

## Standard Agent Workflow

Use this sequence for most engineering tasks:

1. `just check-tools`
2. If backend work needs local DB:
   1. `just check-infra-tools`
   2. `just infra-up`
   3. `just backend-migrate`
3. `just backend-check`
4. `just ios-package-build`
5. `just ios-build`
6. Implement change.
7. Re-run:
   1. `just ios-build`
8. If backend behavior changed, also run:
   1. `just backend-deep-review`
9. If frontend core logic changed, also run:
   1. `just ios-test`
10. If API contract changed:
   1. Update `api/openapi.yaml`
   2. Ensure model updates in shared/server/client code.
11. If persistence changed:
   1. Add a new migration under `db/migrations`.
12. If issue state changed:
    1. Update GitHub issue comments/checklist
    2. Keep `docs/phase1-master-todo.md` status consistent where relevant
13. Before PR merge (mandatory for backend-impacting issues):
    1. Produce AI review report (security audit + bug check + scalability/cleanliness review)
    2. Use `docs/ai-review-template.md`
    3. Merge only after report is documented in issue/PR

## Mandatory Scalability Boundaries

1. All database/repository code must live in `backend/crates/shared/src/repos`.
2. All API HTTP handlers/middleware/routing must live in `backend/crates/api-server/src/http/*` modules.
3. Keep `main.rs` limited to startup wiring/config/bootstrap.

## Mandatory Code Decomposition Policy

Avoid monolithic files. New work should keep files focused and easy to review.

1. Prefer one responsibility per file/module.
2. For handwritten source files, keep a target size of `<= 300` lines.
3. When a file exceeds `500` lines, split it into submodules in the same issue unless blocked.
4. If touching a file already over `500` lines, do not add net-new complexity without extracting logic first.
5. Exceptions are limited to:
   1. generated files
   2. migration SQL
   3. test fixture data files
6. During issue updates and PR summaries:
   1. explicitly list newly extracted modules
   2. include follow-up issue references for any deferred decomposition

## Current Known State (Scaffold Stage)

1. API server is wired to Postgres via `sqlx` with migration bootstrapping and persisted state for v1 endpoint surfaces.
2. Worker loop is still placeholder execution and currently reports due-job counts only.
3. iOS app currently compiles; full backend integration into app screens is still pending.

## Immediate Next Engineering Targets

1. Implement real Google OAuth exchange + encrypted token persistence.
2. Add first end-to-end flow:
   1. connect Google
   2. persist connector
   3. schedule reminder job
   4. worker triggers notification event
3. Integrate `AlfredAPIClient` into iOS app screens.

## Agent Guardrails

1. Do not remove or bypass privacy controls for convenience.
2. Do not add wide OAuth scopes unless required for a documented feature.
3. Keep edits scoped and consistent with v1 goals.
4. Preserve compatibility between OpenAPI, backend models, and iOS client models.
5. Prefer finishing one issue cleanly over partial progress across many issues.
