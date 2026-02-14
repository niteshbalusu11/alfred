# Alfred Agent Start Guide

## Purpose

This file is the operational and technical entry point for agents working in this repository.
Use it to understand the architecture quickly and run the system with consistent commands.

Read this first for product/business context:

`/Users/niteshchowdharybalusu/Documents/alfred/docs/product-context.md`

## Project Summary

Alfred is a privacy-first iOS assistant product with a hosted backend.
Current v1 scope:

1. Google Calendar meeting reminders.
2. Daily morning brief.
3. Urgent Gmail alerting.

The project intentionally avoids smart-home control in v1 to reduce reliability and liability risk.

## Repository Map

1. iOS app project:
   1. `/Users/niteshchowdharybalusu/Documents/alfred/alfred`
2. Local Swift package used by iOS client code:
   1. `/Users/niteshchowdharybalusu/Documents/alfred/alfred/Packages/AlfredAPIClient`
3. Backend Rust workspace:
   1. `/Users/niteshchowdharybalusu/Documents/alfred/backend`
4. API contract:
   1. `/Users/niteshchowdharybalusu/Documents/alfred/api/openapi.yaml`
5. DB migration draft:
   1. `/Users/niteshchowdharybalusu/Documents/alfred/db/migrations/0001_init.sql`
6. Product/architecture RFC:
   1. `/Users/niteshchowdharybalusu/Documents/alfred/docs/rfc-0001-alfred-ios-v1.md`
7. Phase I master board:
   1. `/Users/niteshchowdharybalusu/Documents/alfred/docs/phase1-master-todo.md`
8. Product context (canonical):
   1. `/Users/niteshchowdharybalusu/Documents/alfred/docs/product-context.md`

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
   1. `/Users/niteshchowdharybalusu/Documents/alfred/docs/issue-update-template.md`

### PR Lifecycle Protocol

1. Open PR targeting `master`.
2. Wait for CI checks:
   1. `.github/workflows/ci.yml` (`Backend Checks`, `iOS Build`)
3. Merge only after checks pass.
4. Once merged:
   1. run `just sync-master`
   2. pick next highest-priority unblocked issue

### Sync Rule: GitHub Issues vs Phase I Board

1. GitHub issue is the immediate execution source.
2. `/docs/phase1-master-todo.md` is the planning/control board.
3. If either side changes, keep them aligned in the same work cycle.
4. Do not start untracked work; create/obtain an issue first.

## Security and Privacy Constraints

Agents must preserve these constraints while implementing features:

1. No plaintext long-term storage of connector data (tokens/data should be encrypted at rest).
2. Data minimization for v1:
   1. Persist only minimal state needed for scheduling, retries, and auditing.
3. API changes must remain aligned with:
   1. `/Users/niteshchowdharybalusu/Documents/alfred/api/openapi.yaml`
4. Schema changes must be migration-driven:
   1. Add/modify SQL files under `/Users/niteshchowdharybalusu/Documents/alfred/db/migrations`.

## Justfile Command Reference

All operational commands are centralized in:

`/Users/niteshchowdharybalusu/Documents/alfred/Justfile`

Run from repository root:

```bash
cd /Users/niteshchowdharybalusu/Documents/alfred
```

Primary commands:

1. `just check-tools`
   1. Verifies local toolchain (`xcodebuild`, `cargo`, `swift`).
2. `just ios-open`
   1. Opens the Xcode project.
3. `just ios-build`
   1. Builds iOS app for simulator.
4. `just ios-test`
   1. Runs iOS tests on default simulator destination.
5. `just ios-package-build`
   1. Builds the local Swift package (`AlfredAPIClient`).
6. `just backend-check`
   1. Runs Rust compile checks.
7. `just backend-build`
   1. Builds Rust backend workspace.
8. `just backend-test`
   1. Runs Rust tests.
9. `just backend-fmt`
   1. Formats Rust code.
10. `just backend-clippy`
   1. Runs lint checks with warnings denied.
11. `just backend-verify`
    1. Runs backend completion gate: fmt + clippy + tests + build.
12. `just backend-api`
    1. Runs REST API server.
13. `just backend-worker`
    1. Runs background worker.
14. `just dev`
    1. Runs API server + worker together.
15. `just docs`
    1. Prints key project documentation paths.
16. `just sync-master`
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
2. `just backend-check`
3. `just ios-package-build`
4. `just ios-build`
5. Implement change.
6. Re-run:
   1. `just ios-build`
7. If backend behavior changed, also run:
   1. `just backend-verify`
8. If frontend core logic changed, also run:
   1. `just ios-test`
9. If API contract changed:
   1. Update `/Users/niteshchowdharybalusu/Documents/alfred/api/openapi.yaml`
   2. Ensure model updates in shared/server/client code.
10. If persistence changed:
   1. Add a new migration under `/Users/niteshchowdharybalusu/Documents/alfred/db/migrations`.
11. If issue state changed:
    1. Update GitHub issue comments/checklist
    2. Keep `/docs/phase1-master-todo.md` status consistent where relevant

## Current Known State (Scaffold Stage)

1. API handlers are stubs and return synthetic data.
2. Worker loop is a placeholder and does not yet process real DB jobs.
3. iOS app currently compiles; full backend integration into app screens is still pending.

## Immediate Next Engineering Targets

1. Wire backend persistence with `sqlx` and PostgreSQL.
2. Implement real Google OAuth exchange + encrypted token persistence.
3. Add first end-to-end flow:
   1. connect Google
   2. persist connector
   3. schedule reminder job
   4. worker triggers notification event
4. Integrate `AlfredAPIClient` into iOS app screens.

## Agent Guardrails

1. Do not remove or bypass privacy controls for convenience.
2. Do not add wide OAuth scopes unless required for a documented feature.
3. Keep edits scoped and consistent with v1 goals.
4. Preserve compatibility between OpenAPI, backend models, and iOS client models.
5. Prefer finishing one issue cleanly over partial progress across many issues.
